use crate::scanner::{format_reg_value, open_hive, resolve_dynamic_subkey};
use crate::types::{CapturedOpState, HardwareProfile, SnapshotRecord, TweakDef, TweakOp};
use chrono::Local;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use winreg::enums::*;
use winreg::RegKey;

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn capture_op_live_state(op_index: usize, op: &TweakOp, hw: &HardwareProfile) -> CapturedOpState {
    let read64 = KEY_READ | KEY_WOW64_64KEY;
    match op.kind.as_str() {
        "registry" => {
            let subkey = resolve_dynamic_subkey(&op.subkey, hw);
            let root = open_hive(&op.hive);
            if op.value_type == "KEY_ONLY" {
                let existed = root.open_subkey_with_flags(&subkey, read64).is_ok();
                return CapturedOpState {
                    op_index,
                    kind: "registry".to_string(),
                    resolved_hive: op.hive.clone(),
                    resolved_subkey: subkey,
                    name: String::new(),
                    existed,
                    captured_type: "KEY_ONLY".to_string(),
                    captured_value: if existed {
                        "KEY_PRESENT".to_string()
                    } else {
                        "ABSENT".to_string()
                    },
                    service_name: String::new(),
                    task_path: String::new(),
                    subgroup_guid: String::new(),
                    setting_guid: String::new(),
                    revert_cmd: String::new(),
                };
            }
            match root.open_subkey_with_flags(&subkey, read64) {
                Ok(k) => match k.get_raw_value(&op.name) {
                    Ok(rv) => {
                        let (vt, val) = format_reg_value(&rv);
                        CapturedOpState {
                            op_index,
                            kind: "registry".to_string(),
                            resolved_hive: op.hive.clone(),
                            resolved_subkey: subkey,
                            name: op.name.clone(),
                            existed: true,
                            captured_type: vt,
                            captured_value: val,
                            service_name: String::new(),
                            task_path: String::new(),
                            subgroup_guid: String::new(),
                            setting_guid: String::new(),
                            revert_cmd: String::new(),
                        }
                    }
                    Err(_) => CapturedOpState {
                        op_index,
                        kind: "registry".to_string(),
                        resolved_hive: op.hive.clone(),
                        resolved_subkey: subkey,
                        name: op.name.clone(),
                        existed: false,
                        captured_type: op.value_type.clone(),
                        captured_value: "ABSENT".to_string(),
                        service_name: String::new(),
                        task_path: String::new(),
                        subgroup_guid: String::new(),
                        setting_guid: String::new(),
                        revert_cmd: String::new(),
                    },
                },
                Err(_) => CapturedOpState {
                    op_index,
                    kind: "registry".to_string(),
                    resolved_hive: op.hive.clone(),
                    resolved_subkey: subkey,
                    name: op.name.clone(),
                    existed: false,
                    captured_type: op.value_type.clone(),
                    captured_value: "ABSENT".to_string(),
                    service_name: String::new(),
                    task_path: String::new(),
                    subgroup_guid: String::new(),
                    setting_guid: String::new(),
                    revert_cmd: String::new(),
                },
            }
        }
        "service" => {
            let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
            let sub = format!(r"SYSTEM\CurrentControlSet\Services\{}", op.service_name);
            let start_opt = hklm
                .open_subkey_with_flags(&sub, read64)
                .and_then(|k| k.get_value::<u32, _>("Start"))
                .ok();
            CapturedOpState {
                op_index,
                kind: "service".to_string(),
                resolved_hive: "HKLM".to_string(),
                resolved_subkey: sub,
                name: "Start".to_string(),
                existed: start_opt.is_some(),
                captured_type: "DWORD".to_string(),
                captured_value: start_opt
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| op.default_value.clone()),
                service_name: op.service_name.clone(),
                task_path: String::new(),
                subgroup_guid: String::new(),
                setting_guid: String::new(),
                revert_cmd: String::new(),
            }
        }
        "schtask" => CapturedOpState {
            op_index,
            kind: "schtask".to_string(),
            resolved_hive: String::new(),
            resolved_subkey: String::new(),
            name: String::new(),
            existed: true,
            captured_type: "SCHTASK".to_string(),
            captured_value: op.default_value.clone(),
            service_name: String::new(),
            task_path: op.task_path.clone(),
            subgroup_guid: String::new(),
            setting_guid: String::new(),
            revert_cmd: String::new(),
        },
        "powercfg" => {
            let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
            let user_path = format!(
                r"SYSTEM\CurrentControlSet\Control\Power\User\PowerSchemes\{}\{}\{}",
                hw.active_power_scheme, op.subgroup_guid, op.setting_guid
            );
            let ac_opt = hklm
                .open_subkey_with_flags(&user_path, read64)
                .and_then(|k| k.get_value::<u32, _>("ACSettingIndex"))
                .ok();
            CapturedOpState {
                op_index,
                kind: "powercfg".to_string(),
                resolved_hive: String::new(),
                resolved_subkey: String::new(),
                name: String::new(),
                existed: ac_opt.is_some(),
                captured_type: "POWERCFG".to_string(),
                captured_value: ac_opt
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| op.default_value.clone()),
                service_name: String::new(),
                task_path: String::new(),
                subgroup_guid: op.subgroup_guid.clone(),
                setting_guid: op.setting_guid.clone(),
                revert_cmd: op.revert_cmd.clone(),
            }
        }
        _ => CapturedOpState {
            op_index,
            kind: op.kind.clone(),
            resolved_hive: String::new(),
            resolved_subkey: String::new(),
            name: op.param_key.clone(),
            existed: true,
            captured_type: op.kind.clone(),
            captured_value: op.default_value.clone(),
            service_name: String::new(),
            task_path: String::new(),
            subgroup_guid: String::new(),
            setting_guid: String::new(),
            revert_cmd: op.revert_cmd.clone(),
        },
    }
}

fn full_hive_name(short: &str) -> &'static str {
    match short.to_uppercase().as_str() {
        "HKCU" | "HKEY_CURRENT_USER" => "HKEY_CURRENT_USER",
        "HKCR" | "HKEY_CLASSES_ROOT" => "HKEY_CLASSES_ROOT",
        "HKU" | "HKEY_USERS" => "HKEY_USERS",
        _ => "HKEY_LOCAL_MACHINE",
    }
}

fn build_emergency_files(
    snapshot_id: &str,
    states: &HashMap<String, Vec<CapturedOpState>>,
    emergency_dir: &Path,
) -> (PathBuf, PathBuf) {
    let _ = fs::create_dir_all(emergency_dir);
    let reg_path = emergency_dir.join(format!("undo_{}.reg", snapshot_id));
    let cmd_path = emergency_dir.join(format!("undo_{}.cmd", snapshot_id));

    let mut reg = String::from("Windows Registry Editor Version 5.00\r\n; Crystalline Offline WinRE Emergency Undo File\r\n\r\n");
    let mut cmd = String::from("@echo off\r\n:: Crystalline Offline Emergency Rollback Script\r\necho Restoring pre-tweak Windows state...\r\nreg import \"%~dp0undo_");
    cmd.push_str(snapshot_id);
    cmd.push_str(".reg\"\r\n");

    for (tweak_id, ops) in states {
        reg.push_str(&format!("; --- Tweak: {} ---\r\n", tweak_id));
        for st in ops {
            match st.kind.as_str() {
                "registry" => {
                    let hive = full_hive_name(&st.resolved_hive);
                    if st.captured_type == "KEY_ONLY" && !st.existed {
                        reg.push_str(&format!("[-{}\\{}]\r\n\r\n", hive, st.resolved_subkey));
                    } else if !st.existed {
                        reg.push_str(&format!("[{}\\{}]\r\n", hive, st.resolved_subkey));
                        if st.name.is_empty() {
                            reg.push_str("@=-\r\n\r\n");
                        } else {
                            reg.push_str(&format!("\"{}\"=-\r\n\r\n", st.name.replace('"', "\\\"")));
                        }
                    } else {
                        reg.push_str(&format!("[{}\\{}]\r\n", hive, st.resolved_subkey));
                        let key_lhs = if st.name.is_empty() {
                            "@".to_string()
                        } else {
                            format!("\"{}\"", st.name.replace('"', "\\\""))
                        };
                        match st.captured_type.as_str() {
                            "DWORD" => {
                                let v = st.captured_value.parse::<u32>().unwrap_or(0);
                                reg.push_str(&format!("{}=dword:{:08x}\r\n\r\n", key_lhs, v));
                            }
                            "QWORD" => {
                                let v = st.captured_value.parse::<u64>().unwrap_or(0);
                                let hex_bytes: Vec<String> = v
                                    .to_le_bytes()
                                    .iter()
                                    .map(|b| format!("{:02x}", b))
                                    .collect();
                                reg.push_str(&format!("{}=hex(b):{}\r\n\r\n", key_lhs, hex_bytes.join(",")));
                            }
                            "SZ" => {
                                let esc = st.captured_value.replace('\\', "\\\\").replace('"', "\\\"");
                                reg.push_str(&format!("{}=\"{}\"\r\n\r\n", key_lhs, esc));
                            }
                            "EXPAND_SZ" => {
                                let mut u16s: Vec<u16> = st.captured_value.encode_utf16().collect();
                                u16s.push(0);
                                let hex_bytes: Vec<String> = u16s
                                    .into_iter()
                                    .flat_map(|c| c.to_le_bytes())
                                    .map(|b| format!("{:02x}", b))
                                    .collect();
                                reg.push_str(&format!("{}=hex(2):{}\r\n\r\n", key_lhs, hex_bytes.join(",")));
                            }
                            "BINARY" | "MULTI_SZ" => {
                                let tag = if st.captured_type == "MULTI_SZ" { "hex(7)" } else { "hex" };
                                let bytes: Vec<String> = st
                                    .captured_value
                                    .as_bytes()
                                    .chunks(2)
                                    .filter_map(|c| std::str::from_utf8(c).ok())
                                    .filter(|s| s.len() == 2 && s.chars().all(|c| c.is_ascii_hexdigit()))
                                    .map(|s| s.to_lowercase())
                                    .collect();
                                if !bytes.is_empty() {
                                    reg.push_str(&format!("{}={}:{}\r\n\r\n", key_lhs, tag, bytes.join(",")));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "service" => {
                    let mode = match st.captured_value.as_str() {
                        "2" => "auto",
                        "4" => "disabled",
                        _ => "demand",
                    };
                    cmd.push_str(&format!("sc config \"{}\" start= {}\r\n", st.service_name, mode));
                }
                "schtask" => {
                    let flag = if st.captured_value.eq_ignore_ascii_case("Disabled") {
                        "/Disable"
                    } else {
                        "/Enable"
                    };
                    cmd.push_str(&format!("schtasks /Change /TN \"{}\" {}\r\n", st.task_path, flag));
                }
                "powercfg" => {
                    if !st.captured_value.is_empty() {
                        cmd.push_str(&format!(
                            "powercfg /setacvalueindex SCHEME_CURRENT {} {} {}\r\n",
                            st.subgroup_guid, st.setting_guid, st.captured_value
                        ));
                    }
                }
                _ => {
                    if !st.revert_cmd.is_empty() {
                        cmd.push_str(&format!("{}\r\n", st.revert_cmd));
                    }
                }
            }
        }
    }
    cmd.push_str("powercfg /setactive SCHEME_CURRENT >nul 2>&1\r\necho Rollback completed.\r\n");

    let _ = fs::write(&reg_path, reg);
    let _ = fs::write(&cmd_path, cmd);
    (reg_path, cmd_path)
}

pub fn load_baseline_map(backup_root: &Path) -> HashMap<String, Vec<CapturedOpState>> {
    let p = backup_root.join("original_baseline.json");
    if let Ok(raw) = fs::read_to_string(p) {
        if let Ok(map) = serde_json::from_str::<HashMap<String, Vec<CapturedOpState>>>(&raw) {
            return map;
        }
    }
    HashMap::new()
}

pub fn save_baseline_map(backup_root: &Path, map: &HashMap<String, Vec<CapturedOpState>>) {
    let _ = fs::create_dir_all(backup_root);
    let p = backup_root.join("original_baseline.json");
    if let Ok(json) = serde_json::to_string_pretty(map) {
        let _ = fs::write(p, json);
    }
}

pub fn get_snapshot_backed_ids(backup_root: &Path) -> HashSet<String> {
    load_baseline_map(backup_root).keys().cloned().collect()
}

pub fn create_snapshot_before_apply(
    tweaks: &[&TweakDef],
    hw: &HardwareProfile,
    label: &str,
    backup_root: &Path,
) -> Result<SnapshotRecord, String> {
    let snapshots_dir = backup_root.join("snapshots");
    let emergency_dir = backup_root.join("emergency_reg");
    fs::create_dir_all(&snapshots_dir).map_err(|e| e.to_string())?;

    let now = Local::now();
    let snapshot_id = format!("snap_{}", now.format("%Y%m%d_%H%M%S_%3f"));
    let created_at = now.to_rfc3339();

    let mut states: HashMap<String, Vec<CapturedOpState>> = HashMap::new();
    let mut baseline = load_baseline_map(backup_root);

    for t in tweaks {
        let captured: Vec<CapturedOpState> = t
            .ops
            .iter()
            .enumerate()
            .map(|(idx, op)| capture_op_live_state(idx, op, hw))
            .collect();
        // Only record into original_baseline if not already recorded (preserves untouched pre-tweak OS state!)
        baseline.entry(t.id.clone()).or_insert_with(|| captured.clone());
        states.insert(t.id.clone(), captured);
    }

    save_baseline_map(backup_root, &baseline);

    let (reg_path, cmd_path) = build_emergency_files(&snapshot_id, &states, &emergency_dir);
    let serialized_states = serde_json::to_string(&states).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    hasher.update(serialized_states.as_bytes());
    let sha256 = format!("{:x}", hasher.finalize());

    let record = SnapshotRecord {
        snapshot_id: snapshot_id.clone(),
        created_at,
        label: label.to_string(),
        tweak_ids: tweaks.iter().map(|t| t.id.clone()).collect(),
        sha256,
        emergency_reg_path: reg_path.to_string_lossy().to_string(),
        emergency_cmd_path: cmd_path.to_string_lossy().to_string(),
        states,
    };

    let snap_file = snapshots_dir.join(format!("{}.json", snapshot_id));
    let json = serde_json::to_string_pretty(&record).map_err(|e| e.to_string())?;
    fs::write(&snap_file, json).map_err(|e| e.to_string())?;

    Ok(record)
}

pub fn list_snapshots(backup_root: &Path) -> Vec<SnapshotRecord> {
    let snapshots_dir = backup_root.join("snapshots");
    let mut list = Vec::new();
    if let Ok(entries) = fs::read_dir(snapshots_dir) {
        for ent in entries.flatten() {
            if ent.path().extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(raw) = fs::read_to_string(ent.path()) {
                    if let Ok(rec) = serde_json::from_str::<SnapshotRecord>(&raw) {
                        list.push(rec);
                    }
                }
            }
        }
    }
    list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    list
}

pub fn create_windows_restore_point(label: &str, is_elevated: bool) -> Result<String, String> {
    if !is_elevated {
        return Err("Administrator elevation is required to create a Windows System Restore Point.".to_string());
    }
    let safe_label: String = label
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .take(48)
        .collect();
    // Self-heal first: Checkpoint-Computer fails (ServiceDisabled) when System
    // Restore is turned off, the SR scheduled task is disabled, or the shadow-copy
    // provider services are stopped — which prior "optimizer" tools commonly do.
    // The script enables SR on the system drive, clears policy/registry kill
    // switches, un-disables the SR task, starts the provider services, then
    // creates the point.
    let ps_script = format!(
        "$srPath='HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\SystemRestore'; \
         $polPath='HKLM:\\SOFTWARE\\Policies\\Microsoft\\Windows NT\\SystemRestore'; \
         foreach ($p in @($srPath, $polPath)) {{ \
           if (Test-Path $p) {{ \
             Set-ItemProperty -Path $p -Name DisableSR -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue; \
             Set-ItemProperty -Path $p -Name DisableConfig -Value 0 -Type DWord -Force -ErrorAction SilentlyContinue }} }}; \
         Set-ItemProperty -Path $srPath -Name RPSessionInterval -Value 1 -Type DWord -Force -ErrorAction SilentlyContinue; \
         try {{ Enable-ScheduledTask -TaskName 'SR' -TaskPath '\\Microsoft\\Windows\\SystemRestore\\' -ErrorAction Stop | Out-Null }} catch {{}}; \
         foreach ($svc in @('srservice','swprv','VSS')) {{ \
           try {{ Set-Service -Name $svc -StartupType Manual -ErrorAction Stop }} catch {{}} }}; \
         try {{ Enable-ComputerRestore -Drive \"$env:SystemDrive\\\" -ErrorAction SilentlyContinue }} catch {{}}; \
         foreach ($svc in @('VSS','swprv','srservice')) {{ \
           try {{ $s = Get-Service -Name $svc -ErrorAction Stop; \
                 if ($s.Status -ne 'Running') {{ Start-Service -Name $svc -ErrorAction Stop }} }} catch {{}} }}; \
         $prev=(Get-ItemProperty -Path $srPath -Name SystemRestorePointCreationFrequency -ErrorAction SilentlyContinue).SystemRestorePointCreationFrequency; \
         Set-ItemProperty -Path $srPath -Name SystemRestorePointCreationFrequency -Value 0 -Type DWord -Force; \
         try {{ Checkpoint-Computer -Description 'Crystalline: {}' -RestorePointType 'MODIFY_SETTINGS' }} \
         finally {{ \
           if ($null -ne $prev) {{ Set-ItemProperty -Path $srPath -Name SystemRestorePointCreationFrequency -Value $prev -Type DWord -Force }} \
           else {{ Remove-ItemProperty -Path $srPath -Name SystemRestorePointCreationFrequency -ErrorAction SilentlyContinue }} \
         }}",
        safe_label
    );
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to invoke System Restore: {}", e))?;
    if out.status.success() {
        Ok(format!("Restore point 'Crystalline: {}' created successfully.", safe_label))
    } else {
        let raw = String::from_utf8_lossy(&out.stderr);
        // Collapse PowerShell's multi-line error block into the actionable line.
        let detail = raw
            .lines()
            .map(|l| l.trim())
            .find(|l| !l.is_empty() && l.contains(':'))
            .unwrap_or("")
            .split(':')
            .nth(1)
            .unwrap_or("")
            .trim()
            .to_string();
        let detail = if detail.is_empty() {
            raw.lines().find(|l| !l.trim().is_empty()).unwrap_or("unknown error").trim().to_string()
        } else {
            detail
        };
        let truncated: String = detail.chars().take(240).collect();
        Err(format!(
            "System Restore could not create a checkpoint: {}. VSS or System Restore may still be blocked by policy.",
            truncated
        ))
    }
}
