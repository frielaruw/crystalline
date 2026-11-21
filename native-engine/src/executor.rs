use crate::safety::{evaluate_eligibility, validate_op_safety_firewall};
use crate::scanner::{
    format_reg_value, open_hive, resolve_dynamic_subkey, resolve_dynamic_value,
};
use crate::snapshot::{
    capture_op_live_state, create_snapshot_before_apply, load_baseline_map, save_baseline_map,
};
use crate::types::{
    CapturedOpState, DiffEntry, ExecutionReport, HardwareProfile, SnapshotRecord, TweakDef,
    TweakOp,
};
use sha2::{Digest, Sha256};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::time::Instant;
use winreg::enums::*;
use winreg::{RegKey, RegValue};

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn generate_dry_run_diff(
    tweaks: &[&TweakDef],
    hw: &HardwareProfile,
    mode: &str,
    backup_root: &Path,
) -> Vec<DiffEntry> {
    let baseline = load_baseline_map(backup_root);
    let mut entries = Vec::new();

    for t in tweaks {
        let (eligible, mut warnings) = evaluate_eligibility(t, hw);
        let mut blocked = false;
        let mut block_reason = String::new();

        if t.requires_admin && !hw.is_elevated {
            blocked = true;
            block_reason = "Requires Administrator elevation (relaunch elevated to apply HKLM/Service changes).".to_string();
        } else if !eligible && mode == "apply" {
            blocked = true;
            block_reason = warnings
                .first()
                .cloned()
                .unwrap_or_else(|| "Hardware/OS requirement not met.".to_string());
        }

        for (idx, op) in t.ops.iter().enumerate() {
            if let Err(fw_err) = validate_op_safety_firewall(op) {
                blocked = true;
                block_reason = fw_err.clone();
                warnings.push(fw_err);
            }
            let live = capture_op_live_state(idx, op, hw);
            let target = match op.kind.as_str() {
                "registry" => {
                    if live.name.is_empty() {
                        format!("{}\\{}\\(Default)", live.resolved_hive, live.resolved_subkey)
                    } else {
                        format!("{}\\{}\\{}", live.resolved_hive, live.resolved_subkey, live.name)
                    }
                }
                "service" => format!("Service: {}", op.service_name),
                "schtask" => format!("Scheduled Task: {}", op.task_path),
                "powercfg" => format!("PowerCfg: {}\\{}", op.subgroup_guid, op.setting_guid),
                _ => format!("{}: {}", op.kind, op.param_key),
            };

            let after = if mode == "revert" {
                baseline
                    .get(&t.id)
                    .and_then(|v| v.get(idx))
                    .map(|b| format!("{} (Snapshot Restore)", b.captured_value))
                    .unwrap_or_else(|| format!("{} (Windows Default)", op.default_value))
            } else {
                resolve_dynamic_value(&op.apply_value, hw)
            };

            entries.push(DiffEntry {
                tweak_id: t.id.clone(),
                tweak_title: t.title.clone(),
                risk: t.risk.clone(),
                requires_admin: t.requires_admin,
                target,
                before: live.captured_value,
                after,
                warnings: warnings.clone(),
                blocked,
                block_reason: block_reason.clone(),
            });
        }
    }
    entries
}

pub fn apply_tweaks(
    tweaks: &[&TweakDef],
    hw: &HardwareProfile,
    backup_root: &Path,
    allow_near_override: bool,
) -> ExecutionReport {
    let t0 = Instant::now();
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();
    let mut ucpd_warnings = Vec::new();
    let mut reboot_needed = false;
    let mut explorer_restart_needed = false;

    // Filter eligible + permission-checked tweaks
    let mut executable: Vec<&TweakDef> = Vec::new();
    for t in tweaks {
        if t.requires_admin && !hw.is_elevated {
            failed.push((
                t.id.clone(),
                "Requires Administrator privileges — click 'Relaunch as Admin' in header.".to_string(),
            ));
            continue;
        }
        let (eligible, warnings) = evaluate_eligibility(t, hw);
        if !eligible && !allow_near_override {
            failed.push((
                t.id.clone(),
                warnings
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| "Ineligible hardware/OS.".to_string()),
            ));
            continue;
        }
        if let Some(err) = t.ops.iter().find_map(|op| validate_op_safety_firewall(op).err()) {
            failed.push((t.id.clone(), err));
            continue;
        }
        executable.push(t);
    }

    if executable.is_empty() {
        return ExecutionReport {
            ok: false,
            action: "apply".to_string(),
            duration_ms: t0.elapsed().as_secs_f64() * 1000.0,
            snapshot_id: None,
            emergency_reg_path: None,
            succeeded,
            failed,
            ucpd_warnings,
            reboot_needed: false,
            explorer_restart_needed: false,
        };
    }

    // 1. Mandatory pre-mutation snapshot + offline WinRE .reg/.cmd backup
    let label = if executable.len() == 1 {
        format!("Apply {}", executable[0].title)
    } else {
        format!("Batch Apply ({} tweaks)", executable.len())
    };
    let snap = match create_snapshot_before_apply(&executable, hw, &label, backup_root) {
        Ok(s) => s,
        Err(e) => {
            failed.push((
                "snapshot_engine".to_string(),
                format!("Aborted for safety: could not save pre-tweak snapshot: {}", e),
            ));
            return ExecutionReport {
                ok: false,
                action: "apply".to_string(),
                duration_ms: t0.elapsed().as_secs_f64() * 1000.0,
                snapshot_id: None,
                emergency_reg_path: None,
                succeeded,
                failed,
                ucpd_warnings,
                reboot_needed: false,
                explorer_restart_needed: false,
            };
        }
    };

    // 2. Transactional per-tweak execution with automatic atomic rollback on partial failure
    for t in executable {
        let mut applied_indices = Vec::new();
        let mut op_error: Option<String> = None;

        for (idx, op) in t.ops.iter().enumerate() {
            match execute_single_apply(op, hw) {
                Ok(maybe_ucpd_warn) => {
                    applied_indices.push(idx);
                    if let Some(w) = maybe_ucpd_warn {
                        ucpd_warnings.push(format!("{}: {}", t.title, w));
                    }
                }
                Err(err) => {
                    op_error = Some(err);
                    break;
                }
            }
        }

        if let Some(err) = op_error {
            // Atomic rollback of already-applied ops for this tweak!
            if let Some(saved_ops) = snap.states.get(&t.id) {
                for &done_idx in applied_indices.iter().rev() {
                    if let Some(st) = saved_ops.get(done_idx) {
                        let _ = restore_captured_op(st, hw);
                    }
                }
            }
            failed.push((t.id.clone(), format!("Rolled back cleanly after error: {}", err)));
        } else {
            succeeded.push(t.id.clone());
            if t.reboot_required {
                reboot_needed = true;
            }
            if t.explorer_restart {
                explorer_restart_needed = true;
            }
        }
    }

    ExecutionReport {
        ok: failed.is_empty(),
        action: "apply".to_string(),
        duration_ms: t0.elapsed().as_secs_f64() * 1000.0,
        snapshot_id: Some(snap.snapshot_id),
        emergency_reg_path: Some(snap.emergency_reg_path),
        succeeded,
        failed,
        ucpd_warnings,
        reboot_needed,
        explorer_restart_needed,
    }
}

pub fn revert_tweaks(
    tweaks: &[&TweakDef],
    hw: &HardwareProfile,
    backup_root: &Path,
) -> ExecutionReport {
    let t0 = Instant::now();
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();
    let mut reboot_needed = false;
    let mut explorer_restart_needed = false;
    let mut baseline = load_baseline_map(backup_root);
    let mut baseline_modified = false;

    for t in tweaks {
        if t.requires_admin && !hw.is_elevated {
            failed.push((
                t.id.clone(),
                "Requires Administrator privileges to revert machine-level setting.".to_string(),
            ));
            continue;
        }

        let mut err_msg: Option<String> = None;
        if let Some(saved_ops) = baseline.get(&t.id).cloned() {
            for st in &saved_ops {
                if let Err(e) = restore_captured_op(st, hw) {
                    err_msg = Some(e);
                    break;
                }
            }
            if err_msg.is_none() {
                baseline.remove(&t.id);
                baseline_modified = true;
            }
        } else {
            // Fallback to verified Windows Factory Default if tweak was applied outside Crystalline
            for op in &t.ops {
                if let Err(e) = revert_op_to_factory_default(op, hw) {
                    err_msg = Some(e);
                    break;
                }
            }
        }

        if let Some(e) = err_msg {
            failed.push((t.id.clone(), e));
        } else {
            succeeded.push(t.id.clone());
            if t.reboot_required {
                reboot_needed = true;
            }
            if t.explorer_restart {
                explorer_restart_needed = true;
            }
        }
    }

    if baseline_modified {
        save_baseline_map(backup_root, &baseline);
    }

    ExecutionReport {
        ok: failed.is_empty(),
        action: "revert".to_string(),
        duration_ms: t0.elapsed().as_secs_f64() * 1000.0,
        snapshot_id: None,
        emergency_reg_path: None,
        succeeded,
        failed,
        ucpd_warnings: Vec::new(),
        reboot_needed,
        explorer_restart_needed,
    }
}

pub fn restore_snapshot_by_id(
    snapshot_id: &str,
    hw: &HardwareProfile,
    backup_root: &Path,
) -> ExecutionReport {
    let t0 = Instant::now();
    let snap_path = backup_root
        .join("snapshots")
        .join(format!("{}.json", snapshot_id));
    let raw = match std::fs::read_to_string(&snap_path) {
        Ok(r) => r,
        Err(e) => {
            return ExecutionReport {
                ok: false,
                action: "restore_snapshot".to_string(),
                duration_ms: 0.0,
                snapshot_id: Some(snapshot_id.to_string()),
                emergency_reg_path: None,
                succeeded: vec![],
                failed: vec![(snapshot_id.to_string(), format!("Snapshot file missing: {}", e))],
                ucpd_warnings: vec![],
                reboot_needed: false,
                explorer_restart_needed: false,
            };
        }
    };
    let rec: SnapshotRecord = match serde_json::from_str(&raw) {
        Ok(r) => r,
        Err(e) => {
            return ExecutionReport {
                ok: false,
                action: "restore_snapshot".to_string(),
                duration_ms: 0.0,
                snapshot_id: Some(snapshot_id.to_string()),
                emergency_reg_path: None,
                succeeded: vec![],
                failed: vec![(snapshot_id.to_string(), format!("Corrupt snapshot JSON: {}", e))],
                ucpd_warnings: vec![],
                reboot_needed: false,
                explorer_restart_needed: false,
            };
        }
    };

    // Verify SHA-256 integrity before touching registry!
    if let Ok(serialized_states) = serde_json::to_string(&rec.states) {
        let mut hasher = Sha256::new();
        hasher.update(serialized_states.as_bytes());
        let check = format!("{:x}", hasher.finalize());
        if check != rec.sha256 {
            return ExecutionReport {
                ok: false,
                action: "restore_snapshot".to_string(),
                duration_ms: 0.0,
                snapshot_id: Some(snapshot_id.to_string()),
                emergency_reg_path: None,
                succeeded: vec![],
                failed: vec![(
                    snapshot_id.to_string(),
                    "SHA-256 checksum mismatch! Refusing to apply tampered snapshot.".to_string(),
                )],
                ucpd_warnings: vec![],
                reboot_needed: false,
                explorer_restart_needed: false,
            };
        }
    }

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();

    for (tweak_id, ops) in &rec.states {
        let mut err_opt = None;
        for st in ops {
            if st.resolved_hive == "HKLM" && !hw.is_elevated {
                err_opt = Some("Requires Administrator elevation for HKLM/Service restore.".to_string());
                break;
            }
            if let Err(e) = restore_captured_op(st, hw) {
                err_opt = Some(e);
                break;
            }
        }
        if let Some(e) = err_opt {
            failed.push((tweak_id.clone(), e));
        } else {
            succeeded.push(tweak_id.clone());
        }
    }

    ExecutionReport {
        ok: failed.is_empty(),
        action: "restore_snapshot".to_string(),
        duration_ms: t0.elapsed().as_secs_f64() * 1000.0,
        snapshot_id: Some(snapshot_id.to_string()),
        emergency_reg_path: Some(rec.emergency_reg_path),
        succeeded,
        failed,
        ucpd_warnings: vec![],
        reboot_needed: false,
        explorer_restart_needed: false,
    }
}

fn execute_single_apply(op: &TweakOp, hw: &HardwareProfile) -> Result<Option<String>, String> {
    let rw64 = KEY_READ | KEY_WRITE | KEY_WOW64_64KEY;
    match op.kind.as_str() {
        "registry" => {
            let subkey = resolve_dynamic_subkey(&op.subkey, hw);
            let val_str = resolve_dynamic_value(&op.apply_value, hw);
            let root = open_hive(&op.hive);

            if op.value_type == "KEY_ONLY" {
                let (k, _) = root
                    .create_subkey_with_flags(&subkey, rw64)
                    .map_err(|e| format!("Failed creating key {}\\{}: {}", op.hive, subkey, e))?;
                let _ = k.set_value("", &"");
                return Ok(None);
            }
            if op.value_type == "DELETE_VALUE" {
                if let Ok(k) = root.open_subkey_with_flags(&subkey, rw64) {
                    let _ = k.delete_value(&op.name);
                }
                return Ok(None);
            }

            let (k, _) = root
                .create_subkey_with_flags(&subkey, rw64)
                .map_err(|e| format!("Access denied on {}\\{}: {}", op.hive, subkey, e))?;

            write_typed_reg_value(&k, &op.name, &op.value_type, &val_str)?;

            // Post-write Read-Back Verification (catches silent UCPD.sys / filter driver blocks!)
            if let Ok(readback) = k.get_raw_value(&op.name) {
                let (_, rb_val) = format_reg_value(&readback);
                if !rb_val.eq_ignore_ascii_case(&val_str) {
                    return Ok(Some(format!(
                        "Kernel driver (likely UCPD.sys or GPO) intercepted write to {}\\{} (stayed '{}').",
                        subkey, op.name, rb_val
                    )));
                }
            }
            Ok(None)
        }
        "service" => {
            let mode = match op.apply_value.as_str() {
                "2" => "auto",
                "4" => "disabled",
                _ => "demand",
            };
            let out = Command::new("sc")
                .args(["config", &op.service_name, &format!("start= {}", mode)])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                // Fallback direct registry Start write if SCM locked
                let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
                let sub = format!(r"SYSTEM\CurrentControlSet\Services\{}", op.service_name);
                let k = hklm
                    .open_subkey_with_flags(&sub, rw64)
                    .map_err(|e| format!("Service '{}' cannot be modified: {}", op.service_name, e))?;
                let v = op.apply_value.parse::<u32>().unwrap_or(3);
                k.set_value("Start", &v)
                    .map_err(|e| format!("Service '{}' protected (TrustedInstaller/PPL): {}", op.service_name, e))?;
            }
            Ok(None)
        }
        "schtask" => {
            let flag = if op.apply_value.eq_ignore_ascii_case("Disabled") {
                "/Disable"
            } else {
                "/Enable"
            };
            let out = Command::new("schtasks")
                .args(["/Change", "/TN", &op.task_path, flag])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(format!(
                    "schtasks failed for {}: {}",
                    op.task_path,
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            Ok(None)
        }
        "powercfg" => {
            if !op.apply_cmd.is_empty() {
                run_shell_cmd(&op.apply_cmd)?;
            } else {
                let _ = Command::new("powercfg")
                    .args([
                        "/setacvalueindex",
                        "SCHEME_CURRENT",
                        &op.subgroup_guid,
                        &op.setting_guid,
                        &op.apply_value,
                    ])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();
                let _ = Command::new("powercfg")
                    .args([
                        "/setdcvalueindex",
                        "SCHEME_CURRENT",
                        &op.subgroup_guid,
                        &op.setting_guid,
                        &op.apply_value,
                    ])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();
                let _ = Command::new("powercfg")
                    .args(["/setactive", "SCHEME_CURRENT"])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();
            }
            Ok(None)
        }
        _ => {
            if !op.apply_cmd.is_empty() {
                run_shell_cmd(&op.apply_cmd)?;
            }
            Ok(None)
        }
    }
}

fn restore_captured_op(st: &CapturedOpState, _hw: &HardwareProfile) -> Result<(), String> {
    let rw64 = KEY_READ | KEY_WRITE | KEY_WOW64_64KEY;
    match st.kind.as_str() {
        "registry" => {
            let root = open_hive(&st.resolved_hive);
            if st.captured_type == "KEY_ONLY" {
                if !st.existed {
                    // Delete parent shell override key cleanly
                    let _ = root.delete_subkey_all(&st.resolved_subkey);
                    if let Some((parent, _leaf)) = st.resolved_subkey.rsplit_once('\\') {
                        if parent.ends_with('}') {
                            let _ = root.delete_subkey_all(parent);
                        }
                    }
                }
                return Ok(());
            }
            if !st.existed || st.captured_value == "ABSENT" {
                if let Ok(k) = root.open_subkey_with_flags(&st.resolved_subkey, rw64) {
                    let _ = k.delete_value(&st.name);
                }
                return Ok(());
            }
            let (k, _) = root
                .create_subkey_with_flags(&st.resolved_subkey, rw64)
                .map_err(|e| e.to_string())?;
            write_typed_reg_value(&k, &st.name, &st.captured_type, &st.captured_value)
        }
        "service" => {
            let mode = match st.captured_value.as_str() {
                "2" => "auto",
                "4" => "disabled",
                _ => "demand",
            };
            let out = Command::new("sc")
                .args(["config", &st.service_name, &format!("start= {}", mode)])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
                if let Ok(k) = hklm.open_subkey_with_flags(&st.resolved_subkey, rw64) {
                    let v = st.captured_value.parse::<u32>().unwrap_or(3);
                    let _ = k.set_value("Start", &v);
                }
            }
            Ok(())
        }
        "schtask" => {
            let flag = if st.captured_value.eq_ignore_ascii_case("Disabled") {
                "/Disable"
            } else {
                "/Enable"
            };
            let _ = Command::new("schtasks")
                .args(["/Change", "/TN", &st.task_path, flag])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
            Ok(())
        }
        "powercfg" => {
            if !st.revert_cmd.is_empty() {
                let _ = run_shell_cmd(&st.revert_cmd);
            } else if !st.captured_value.is_empty() {
                let _ = Command::new("powercfg")
                    .args([
                        "/setacvalueindex",
                        "SCHEME_CURRENT",
                        &st.subgroup_guid,
                        &st.setting_guid,
                        &st.captured_value,
                    ])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();
                let _ = Command::new("powercfg")
                    .args(["/setactive", "SCHEME_CURRENT"])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();
            }
            Ok(())
        }
        _ => {
            if !st.revert_cmd.is_empty() {
                run_shell_cmd(&st.revert_cmd)?;
            }
            Ok(())
        }
    }
}

fn revert_op_to_factory_default(op: &TweakOp, hw: &HardwareProfile) -> Result<(), String> {
    let rw64 = KEY_READ | KEY_WRITE | KEY_WOW64_64KEY;
    match op.kind.as_str() {
        "registry" => {
            let subkey = resolve_dynamic_subkey(&op.subkey, hw);
            let root = open_hive(&op.hive);
            if op.value_type == "KEY_ONLY" {
                let _ = root.delete_subkey_all(&subkey);
                if let Some((parent, _)) = subkey.rsplit_once('\\') {
                    if parent.ends_with('}') {
                        let _ = root.delete_subkey_all(parent);
                    }
                }
                return Ok(());
            }
            if op.default_value.eq_ignore_ascii_case("ABSENT") || op.default_value.is_empty() {
                if let Ok(k) = root.open_subkey_with_flags(&subkey, rw64) {
                    let _ = k.delete_value(&op.name);
                }
                return Ok(());
            }
            let (k, _) = root
                .create_subkey_with_flags(&subkey, rw64)
                .map_err(|e| e.to_string())?;
            write_typed_reg_value(&k, &op.name, &op.value_type, &op.default_value)
        }
        "service" => {
            let mode = match op.default_value.as_str() {
                "2" => "auto",
                "4" => "disabled",
                _ => "demand",
            };
            let _ = Command::new("sc")
                .args(["config", &op.service_name, &format!("start= {}", mode)])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
            Ok(())
        }
        "schtask" => {
            let flag = if op.default_value.eq_ignore_ascii_case("Disabled") {
                "/Disable"
            } else {
                "/Enable"
            };
            let _ = Command::new("schtasks")
                .args(["/Change", "/TN", &op.task_path, flag])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
            Ok(())
        }
        "powercfg" => {
            if !op.revert_cmd.is_empty() {
                let _ = run_shell_cmd(&op.revert_cmd);
            } else if !op.default_value.is_empty() {
                let _ = Command::new("powercfg")
                    .args([
                        "/setacvalueindex",
                        "SCHEME_CURRENT",
                        &op.subgroup_guid,
                        &op.setting_guid,
                        &op.default_value,
                    ])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();
                let _ = Command::new("powercfg")
                    .args(["/setactive", "SCHEME_CURRENT"])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();
            }
            Ok(())
        }
        _ => {
            if !op.revert_cmd.is_empty() {
                run_shell_cmd(&op.revert_cmd)?;
            }
            Ok(())
        }
    }
}

fn write_typed_reg_value(k: &RegKey, name: &str, vtype: &str, val_str: &str) -> Result<(), String> {
    match vtype {
        "DWORD" => {
            let v: u32 = if let Some(hex) = val_str.strip_prefix("0x") {
                u32::from_str_radix(hex, 16).map_err(|e| e.to_string())?
            } else {
                val_str.parse::<u32>().map_err(|e| e.to_string())?
            };
            k.set_value(name, &v).map_err(|e| e.to_string())
        }
        "QWORD" => {
            let v: u64 = val_str.parse::<u64>().map_err(|e| e.to_string())?;
            k.set_value(name, &v).map_err(|e| e.to_string())
        }
        "EXPAND_SZ" => {
            let mut u16s: Vec<u16> = val_str.encode_utf16().collect();
            u16s.push(0);
            let bytes: Vec<u8> = u16s.into_iter().flat_map(|c| c.to_le_bytes()).collect();
            let rv = RegValue {
                bytes,
                vtype: REG_EXPAND_SZ,
            };
            k.set_raw_value(name, &rv).map_err(|e| e.to_string())
        }
        "BINARY" | "MULTI_SZ" => {
            let clean: String = val_str.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            let mut bytes = Vec::with_capacity(clean.len() / 2);
            for i in (0..clean.len()).step_by(2) {
                if i + 2 <= clean.len() {
                    if let Ok(b) = u8::from_str_radix(&clean[i..i + 2], 16) {
                        bytes.push(b);
                    }
                }
            }
            let vt = if vtype == "MULTI_SZ" { REG_MULTI_SZ } else { REG_BINARY };
            let rv = RegValue { bytes, vtype: vt };
            k.set_raw_value(name, &rv).map_err(|e| e.to_string())
        }
        _ => k.set_value(name, &val_str).map_err(|e| e.to_string()),
    }
}

fn run_shell_cmd(cmd_line: &str) -> Result<(), String> {
    let out = Command::new("cmd")
        .args(["/C", cmd_line])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

pub fn heal_bad_tweak(finding_id: &str, hw: &HardwareProfile) -> Result<String, String> {
    let rw64 = KEY_READ | KEY_WRITE | KEY_WOW64_64KEY;
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    match finding_id {
        "heal_menu_show_delay_zero" => {
            let (k, _) = hkcu
                .create_subkey_with_flags(r"Control Panel\Desktop", rw64)
                .map_err(|e| e.to_string())?;
            k.set_value("MenuShowDelay", &"20").map_err(|e| e.to_string())?;
            Ok("Restored MenuShowDelay to safe 20ms.".to_string())
        }
        _ => {
            if !hw.is_elevated {
                return Err("Administrator elevation required to repair machine-level kernel/BCD/network setting.".to_string());
            }
            match finding_id {
                "heal_wait_to_kill_service" => {
                    let (k, _) = hklm
                        .create_subkey_with_flags(r"SYSTEM\CurrentControlSet\Control", rw64)
                        .map_err(|e| e.to_string())?;
                    k.set_value("WaitToKillServiceTimeout", &"5000")
                        .map_err(|e| e.to_string())?;
                    Ok("Restored WaitToKillServiceTimeout to 5000ms.".to_string())
                }
                "heal_svchost_split" => {
                    let (k, _) = hklm
                        .create_subkey_with_flags(r"SYSTEM\CurrentControlSet\Control", rw64)
                        .map_err(|e| e.to_string())?;
                    k.set_value("SvcHostSplitThresholdInKB", &3670016u32)
                        .map_err(|e| e.to_string())?;
                    Ok("Restored SvcHostSplitThresholdInKB to Windows client default (0x380000).".to_string())
                }
                "heal_mouse_queue" => {
                    let (k, _) = hklm
                        .create_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services\mouclass\Parameters", rw64)
                        .map_err(|e| e.to_string())?;
                    k.set_value("MouseDataQueueSize", &100u32)
                        .map_err(|e| e.to_string())?;
                    Ok("Restored MouseDataQueueSize to 100 packets.".to_string())
                }
                "heal_kbd_queue" => {
                    let (k, _) = hklm
                        .create_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services\kbdclass\Parameters", rw64)
                        .map_err(|e| e.to_string())?;
                    k.set_value("KeyboardDataQueueSize", &100u32)
                        .map_err(|e| e.to_string())?;
                    Ok("Restored KeyboardDataQueueSize to 100 packets.".to_string())
                }
                "heal_cpu_mitigations" => {
                    if let Ok(k) = hklm.open_subkey_with_flags(
                        r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management",
                        rw64,
                    ) {
                        let _ = k.delete_value("FeatureSettingsOverride");
                        let _ = k.delete_value("FeatureSettingsOverrideMask");
                    }
                    Ok("Removed unsafe CPU mitigation overrides.".to_string())
                }
                "heal_ssd_trim" => {
                    run_shell_cmd("fsutil behavior set DisableDeleteNotification 0")?;
                    Ok("Re-enabled NVMe/SSD TRIM (DisableDeleteNotification = 0).".to_string())
                }
                "heal_tcp_autotuning" => {
                    run_shell_cmd("netsh int tcp set global autotuninglevel=normal")?;
                    Ok("Restored TCP Receive Window Auto-Tuning to 'normal'.".to_string())
                }
                "heal_bcd_useplatformclock" => {
                    let _ = run_shell_cmd("bcdedit /deletevalue useplatformclock");
                    Ok("Removed forced HPET platform clock override from BCD.".to_string())
                }
                other => Err(format!("Unknown repair ID: {}", other)),
            }
        }
    }
}
