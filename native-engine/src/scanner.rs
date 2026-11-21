use crate::safety::{evaluate_eligibility, scan_for_bad_tweaks};
use crate::types::{
    Catalog, FullScanReport, HardwareProfile, NearMatchRule, OpLiveDetail, TweakDef, TweakOp,
    TweakScanResult,
};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::os::windows::process::CommandExt;
use std::time::Instant;
use winreg::enums::*;
use winreg::{RegKey, RegValue};

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct SystemProbeCaches {
    pub schtasks_disabled: HashSet<String>,
    pub schtasks_present: HashSet<String>,
    pub netsh_tcp: HashMap<String, String>,
    pub fsutil_ntfs: HashMap<String, String>,
    pub bcd_current: HashMap<String, String>,
}

impl SystemProbeCaches {
    pub fn collect(hw: &HardwareProfile) -> Self {
        let mut schtasks_disabled = HashSet::new();
        let mut schtasks_present = HashSet::new();

        // Read Windows TaskCache Registry directly in <2ms instead of spawning schtasks.exe!
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let read64 = KEY_READ | KEY_WOW64_64KEY;
        if let Ok(tree_root) = hklm.open_subkey_with_flags(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Schedule\TaskCache\Tree",
            read64,
        ) {
            if let Ok(tasks_root) = hklm.open_subkey_with_flags(
                r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Schedule\TaskCache\Tasks",
                read64,
            ) {
                walk_task_tree(
                    &tree_root,
                    &tasks_root,
                    "",
                    &mut schtasks_present,
                    &mut schtasks_disabled,
                );
            }
        }

        // Parallel lightweight CLI probes for netsh, fsutil, bcdedit (with CREATE_NO_WINDOW)
        let (netsh_tcp, (fsutil_ntfs, bcd_current)) = rayon::join(
            || probe_netsh_tcp(),
            || {
                rayon::join(
                    || probe_fsutil(hw.is_elevated),
                    || {
                        if hw.is_elevated {
                            probe_bcdedit()
                        } else {
                            HashMap::new()
                        }
                    },
                )
            },
        );

        Self {
            schtasks_disabled,
            schtasks_present,
            netsh_tcp,
            fsutil_ntfs,
            bcd_current,
        }
    }
}

fn walk_task_tree(
    node: &RegKey,
    tasks_root: &RegKey,
    prefix: &str,
    present: &mut HashSet<String>,
    disabled: &mut HashSet<String>,
) {
    let read64 = KEY_READ | KEY_WOW64_64KEY;
    if let Ok(guid) = node.get_value::<String, _>("Id") {
        let norm_path = prefix.to_lowercase();
        present.insert(norm_path.clone());
        if let Ok(tk) = tasks_root.open_subkey_with_flags(&guid, read64) {
            // In TaskCache\Tasks\<GUID>, Triggers binary header byte[0x00..0x10] or Enabled status:
            // Also check SD/Triggers: bit in Triggers byte 12 or Absence of enabled flag
            if let Ok(raw) = tk.get_raw_value("Triggers") {
                // Windows TaskCache Triggers blob: header version + flags; offset 12 is enabled bool in Vista+ format when > 12 bytes
                if raw.bytes.len() > 12 && raw.bytes[12] == 0 {
                    disabled.insert(norm_path);
                }
            }
        }
    }
    for sub_name in node.enum_keys().flatten() {
        if let Ok(sub_key) = node.open_subkey_with_flags(&sub_name, read64) {
            let next_prefix = format!("{}\\{}", prefix, sub_name);
            walk_task_tree(&sub_key, tasks_root, &next_prefix, present, disabled);
        }
    }
}

fn probe_netsh_tcp() -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(out) = Command::new("netsh")
        .args(["int", "tcp", "show", "global"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        let txt = String::from_utf8_lossy(&out.stdout);
        for line in txt.lines() {
            if let Some((k, v)) = line.split_once(':') {
                map.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    map
}

fn probe_fsutil(_is_elevated: bool) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let read64 = KEY_READ | KEY_WOW64_64KEY;
    // Read NTFS filesystem control parameters directly from HKLM\SYSTEM\CurrentControlSet\Control\FileSystem!
    if let Ok(fs) = hklm.open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Control\FileSystem", read64) {
        for key in [
            "NtfsDisable8dot3NameCreation",
            "NtfsDisableLastAccessUpdate",
            "NtfsMemoryUsage",
            "NtfsDisableEncryption",
            "DisableDeleteNotification",
            "LongPathsEnabled",
        ] {
            if let Ok(v) = fs.get_value::<u32, _>(key) {
                map.insert(key.to_string(), v.to_string());
            }
        }
    }
    map
}

fn probe_bcdedit() -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(out) = Command::new("bcdedit")
        .args(["/enum", "{current}"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
    {
        let txt = String::from_utf8_lossy(&out.stdout);
        for line in txt.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                map.insert(parts[0].trim().to_lowercase(), parts[1..].join(" "));
            }
        }
    }
    map
}

pub fn resolve_dynamic_subkey(subkey: &str, hw: &HardwareProfile) -> String {
    if subkey.contains("{{ACTIVE_NIC_GUID}}") {
        let nic = hw
            .active_nic_guids
            .first()
            .cloned()
            .unwrap_or_else(|| "{00000000-0000-0000-0000-000000000000}".to_string());
        subkey.replace("{{ACTIVE_NIC_GUID}}", &nic)
    } else {
        subkey.to_string()
    }
}

pub fn resolve_dynamic_value(val: &str, hw: &HardwareProfile) -> String {
    if val == "{{TOTAL_RAM_KB_PLUS_MARGIN}}" {
        let kb = (hw.total_ram_mb * 1024) + 524_288;
        kb.min(u32::MAX as u64).to_string()
    } else {
        val.to_string()
    }
}

pub fn open_hive(hive: &str) -> RegKey {
    match hive.to_uppercase().as_str() {
        "HKCU" | "HKEY_CURRENT_USER" => RegKey::predef(HKEY_CURRENT_USER),
        "HKCR" | "HKEY_CLASSES_ROOT" => RegKey::predef(HKEY_CLASSES_ROOT),
        "HKU" | "HKEY_USERS" => RegKey::predef(HKEY_USERS),
        _ => RegKey::predef(HKEY_LOCAL_MACHINE),
    }
}

pub fn format_reg_value(rv: &RegValue) -> (String, String) {
    match rv.vtype {
        REG_DWORD => {
            if rv.bytes.len() >= 4 {
                let n = u32::from_le_bytes([rv.bytes[0], rv.bytes[1], rv.bytes[2], rv.bytes[3]]);
                ("DWORD".to_string(), n.to_string())
            } else {
                ("DWORD".to_string(), "0".to_string())
            }
        }
        REG_QWORD => {
            if rv.bytes.len() >= 8 {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&rv.bytes[..8]);
                ("QWORD".to_string(), u64::from_le_bytes(arr).to_string())
            } else {
                ("QWORD".to_string(), "0".to_string())
            }
        }
        REG_SZ | REG_EXPAND_SZ => {
            let u16s: Vec<u16> = rv
                .bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .take_while(|&ch| ch != 0)
                .collect();
            let s = String::from_utf16_lossy(&u16s);
            let t = if rv.vtype == REG_EXPAND_SZ {
                "EXPAND_SZ"
            } else {
                "SZ"
            };
            (t.to_string(), s)
        }
        REG_MULTI_SZ => {
            let hex: String = rv.bytes.iter().map(|b| format!("{:02x}", b)).collect();
            ("MULTI_SZ".to_string(), hex)
        }
        _ => {
            let hex: String = rv.bytes.iter().map(|b| format!("{:02x}", b)).collect();
            ("BINARY".to_string(), hex)
        }
    }
}

fn evaluate_single_op(
    op: &TweakOp,
    near_rule: Option<&NearMatchRule>,
    hw: &HardwareProfile,
    caches: &SystemProbeCaches,
) -> OpLiveDetail {
    let read64 = KEY_READ | KEY_WOW64_64KEY;
    match op.kind.as_str() {
        "registry" => {
            let subkey = resolve_dynamic_subkey(&op.subkey, hw);
            let apply_val = resolve_dynamic_value(&op.apply_value, hw);
            let label = if op.name.is_empty() {
                format!("{}\\{}\\(Default)", op.hive, subkey)
            } else {
                format!("{}\\{}\\{}", op.hive, subkey, op.name)
            };
            let root = open_hive(&op.hive);
            if op.value_type == "KEY_ONLY" {
                let key_exists = root.open_subkey_with_flags(&subkey, read64).is_ok();
                return OpLiveDetail {
                    target_label: label,
                    current_value: if key_exists {
                        "KEY_PRESENT".to_string()
                    } else {
                        "ABSENT".to_string()
                    },
                    expected_apply: "KEY_PRESENT".to_string(),
                    expected_default: "ABSENT".to_string(),
                    status: if key_exists {
                        "APPLIED".to_string()
                    } else {
                        "DEFAULT".to_string()
                    },
                };
            }
            if op.value_type == "DELETE_VALUE" {
                let val_exists = root
                    .open_subkey_with_flags(&subkey, read64)
                    .and_then(|k| k.get_raw_value(&op.name))
                    .is_ok();
                return OpLiveDetail {
                    target_label: label,
                    current_value: if val_exists {
                        "PRESENT".to_string()
                    } else {
                        "ABSENT".to_string()
                    },
                    expected_apply: "ABSENT".to_string(),
                    expected_default: op.default_value.clone(),
                    status: if !val_exists {
                        "APPLIED".to_string()
                    } else {
                        "DEFAULT".to_string()
                    },
                };
            }

            match root.open_subkey_with_flags(&subkey, read64) {
                Ok(k) => match k.get_raw_value(&op.name) {
                    Ok(rv) => {
                        let (_vtype, cur) = format_reg_value(&rv);
                        let status = classify_value(&cur, &apply_val, &op.default_value, near_rule);
                        OpLiveDetail {
                            target_label: label,
                            current_value: cur,
                            expected_apply: apply_val,
                            expected_default: op.default_value.clone(),
                            status,
                        }
                    }
                    Err(_) => {
                        let status = if apply_val.eq_ignore_ascii_case("ABSENT") {
                            "APPLIED"
                        } else {
                            "DEFAULT"
                        };
                        OpLiveDetail {
                            target_label: label,
                            current_value: "ABSENT (OS Default)".to_string(),
                            expected_apply: apply_val,
                            expected_default: op.default_value.clone(),
                            status: status.to_string(),
                        }
                    }
                },
                Err(_) => OpLiveDetail {
                    target_label: label,
                    current_value: "ABSENT (OS Default)".to_string(),
                    expected_apply: apply_val,
                    expected_default: op.default_value.clone(),
                    status: "DEFAULT".to_string(),
                },
            }
        }
        "service" => {
            let sub = format!(r"SYSTEM\CurrentControlSet\Services\{}", op.service_name);
            let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
            let label = format!("Service: {}", op.service_name);
            match hklm.open_subkey_with_flags(&sub, read64) {
                Ok(k) => match k.get_value::<u32, _>("Start") {
                    Ok(start) => {
                        let cur_s = start.to_string();
                        let status = if cur_s == op.apply_value {
                            "APPLIED".to_string()
                        } else if cur_s == op.default_value {
                            "DEFAULT".to_string()
                        } else if (op.apply_value == "4" && cur_s == "3" && op.default_value == "2")
                            || (op.apply_value == "3" && cur_s == "4")
                        {
                            "NEAR_MATCH".to_string()
                        } else {
                            "DEFAULT".to_string()
                        };
                        OpLiveDetail {
                            target_label: label,
                            current_value: service_mode_label(&cur_s),
                            expected_apply: service_mode_label(&op.apply_value),
                            expected_default: service_mode_label(&op.default_value),
                            status,
                        }
                    }
                    Err(_) => OpLiveDetail {
                        target_label: label,
                        current_value: "NOT_FOUND".to_string(),
                        expected_apply: service_mode_label(&op.apply_value),
                        expected_default: service_mode_label(&op.default_value),
                        status: "NOT_FOUND".to_string(),
                    },
                },
                Err(_) => OpLiveDetail {
                    target_label: label,
                    current_value: "NOT_INSTALLED".to_string(),
                    expected_apply: service_mode_label(&op.apply_value),
                    expected_default: service_mode_label(&op.default_value),
                    status: "NOT_FOUND".to_string(),
                },
            }
        }
        "schtask" => {
            let norm = op.task_path.to_lowercase();
            let label = format!("Task: {}", op.task_path);
            if !caches.schtasks_present.contains(&norm) {
                return OpLiveDetail {
                    target_label: label,
                    current_value: "NOT_INSTALLED".to_string(),
                    expected_apply: "Disabled".to_string(),
                    expected_default: "Enabled".to_string(),
                    status: "NOT_FOUND".to_string(),
                };
            }
            let is_disabled = caches.schtasks_disabled.contains(&norm);
            let want_disabled = op.apply_value.eq_ignore_ascii_case("Disabled");
            OpLiveDetail {
                target_label: label,
                current_value: if is_disabled {
                    "Disabled".to_string()
                } else {
                    "Enabled".to_string()
                },
                expected_apply: op.apply_value.clone(),
                expected_default: op.default_value.clone(),
                status: if is_disabled == want_disabled {
                    "APPLIED".to_string()
                } else {
                    "DEFAULT".to_string()
                },
            }
        }
        "powercfg" => {
            let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
            let user_path = format!(
                r"SYSTEM\CurrentControlSet\Control\Power\User\PowerSchemes\{}\{}\{}",
                hw.active_power_scheme, op.subgroup_guid, op.setting_guid
            );
            let default_path = format!(
                r"SYSTEM\CurrentControlSet\Control\Power\PowerSettings\{}\{}\DefaultPowerSchemeValues\{}",
                op.subgroup_guid, op.setting_guid, hw.active_power_scheme
            );
            let ac_val = hklm
                .open_subkey_with_flags(&user_path, read64)
                .and_then(|k| k.get_value::<u32, _>("ACSettingIndex"))
                .or_else(|_| {
                    hklm.open_subkey_with_flags(&default_path, read64)
                        .and_then(|k| k.get_value::<u32, _>("ACPowerSettingIndex"))
                });
            let label = format!("PowerCfg: {}\\{}", &op.subgroup_guid[..8.min(op.subgroup_guid.len())], &op.setting_guid[..8.min(op.setting_guid.len())]);
            match ac_val {
                Ok(v) => {
                    let cur = v.to_string();
                    let status = classify_value(&cur, &op.apply_value, &op.default_value, near_rule);
                    OpLiveDetail {
                        target_label: label,
                        current_value: cur,
                        expected_apply: op.apply_value.clone(),
                        expected_default: op.default_value.clone(),
                        status,
                    }
                }
                Err(_) => OpLiveDetail {
                    target_label: label,
                    current_value: "OS Managed".to_string(),
                    expected_apply: op.apply_value.clone(),
                    expected_default: op.default_value.clone(),
                    status: "DEFAULT".to_string(),
                },
            }
        }
        "netsh_tcp" => {
            let cur = caches
                .netsh_tcp
                .get(&op.param_key)
                .cloned()
                .unwrap_or_else(|| "default".to_string());
            let matched = cur.eq_ignore_ascii_case(&op.apply_value);
            OpLiveDetail {
                target_label: format!("Netsh TCP: {}", op.param_key),
                current_value: cur.clone(),
                expected_apply: op.apply_value.clone(),
                expected_default: op.default_value.clone(),
                status: if matched {
                    "APPLIED".to_string()
                } else if !cur.eq_ignore_ascii_case(&op.default_value) && cur != "default" {
                    "NEAR_MATCH".to_string()
                } else {
                    "DEFAULT".to_string()
                },
            }
        }
        _ => OpLiveDetail {
            target_label: op.kind.clone(),
            current_value: "Unknown".to_string(),
            expected_apply: op.apply_value.clone(),
            expected_default: op.default_value.clone(),
            status: "DEFAULT".to_string(),
        },
    }
}

fn service_mode_label(code: &str) -> String {
    match code {
        "2" => "2 (Automatic)".to_string(),
        "3" => "3 (Manual)".to_string(),
        "4" => "4 (Disabled)".to_string(),
        other => other.to_string(),
    }
}

fn classify_value(
    cur: &str,
    apply_val: &str,
    default_val: &str,
    near_rule: Option<&NearMatchRule>,
) -> String {
    if cur.eq_ignore_ascii_case(apply_val) {
        return "APPLIED".to_string();
    }
    if let Ok(cur_num) = cur.parse::<i64>() {
        if let Some(rule) = near_rule {
            if let Some(tb) = rule.toxic_below {
                if cur_num < tb {
                    return "HAZARDOUS".to_string();
                }
            }
            if let Some(ta) = rule.toxic_above {
                if cur_num > ta {
                    return "HAZARDOUS".to_string();
                }
            }
            if rule.toxic_values.contains(&cur_num) {
                return "HAZARDOUS".to_string();
            }
            match rule.kind.as_str() {
                "dword_set" => {
                    if rule.values.contains(&cur_num) {
                        return "NEAR_MATCH".to_string();
                    }
                }
                "numeric_range" => {
                    let lo = rule.min.unwrap_or(i64::MIN);
                    let hi = rule.max.unwrap_or(i64::MAX);
                    if cur_num >= lo && cur_num <= hi && cur != default_val {
                        return "NEAR_MATCH".to_string();
                    }
                }
                "non_default" => {
                    if !default_val.eq_ignore_ascii_case("ABSENT") && cur != default_val {
                        return "NEAR_MATCH".to_string();
                    }
                }
                _ => {}
            }
        } else if !default_val.is_empty()
            && !default_val.eq_ignore_ascii_case("ABSENT")
            && !cur.eq_ignore_ascii_case(default_val)
        {
            return "NEAR_MATCH".to_string();
        }
    } else if !default_val.is_empty()
        && !default_val.eq_ignore_ascii_case("ABSENT")
        && !cur.eq_ignore_ascii_case(default_val)
    {
        return "NEAR_MATCH".to_string();
    }
    "DEFAULT".to_string()
}

pub fn run_full_scan(
    catalog: &Catalog,
    hw: &HardwareProfile,
    snapshot_backed_ids: &HashSet<String>,
) -> FullScanReport {
    let t0 = Instant::now();
    let caches = SystemProbeCaches::collect(hw);

    // Parallel Rayon evaluation across all 350+ tweaks!
    let raw_results: Vec<TweakScanResult> = catalog
        .tweaks
        .par_iter()
        .map(|tweak| scan_single_tweak(tweak, hw, &caches, snapshot_backed_ids))
        .collect();

    let mut results_map: HashMap<String, TweakScanResult> = raw_results
        .into_iter()
        .map(|r| (r.id.clone(), r))
        .collect();

    // Second pass: populate live active_conflicts if any conflicting tweak is APPLIED or NEAR_MATCH
    let active_ids: HashSet<String> = results_map
        .values()
        .filter(|r| r.status == "APPLIED" || r.status == "NEAR_MATCH")
        .map(|r| r.id.clone())
        .collect();

    for tweak in &catalog.tweaks {
        let mut active_conflicts = Vec::new();
        for c_id in &tweak.conflicts_with {
            if active_ids.contains(c_id) {
                active_conflicts.push(c_id.clone());
            }
        }
        if !active_conflicts.is_empty() {
            if let Some(entry) = results_map.get_mut(&tweak.id) {
                entry.active_conflicts = active_conflicts;
            }
        }
    }

    let mut applied_count = 0;
    let mut near_match_count = 0;
    let mut partial_drift_count = 0;
    let mut default_count = 0;
    let mut hazardous_count = 0;
    let mut incompatible_count = 0;

    for r in results_map.values() {
        match r.status.as_str() {
            "APPLIED" => applied_count += 1,
            "NEAR_MATCH" => near_match_count += 1,
            "PARTIAL_DRIFT" => partial_drift_count += 1,
            "HAZARDOUS_CUSTOM" => hazardous_count += 1,
            "INCOMPATIBLE_HARDWARE" | "NOT_INSTALLED" => incompatible_count += 1,
            _ => default_count += 1,
        }
    }

    let bad_tweaks = scan_for_bad_tweaks(hw, &caches.bcd_current, &caches.netsh_tcp, &caches.fsutil_ntfs);
    let scan_duration_ms = t0.elapsed().as_secs_f64() * 1000.0;

    FullScanReport {
        scan_duration_ms,
        total_tweaks: catalog.tweaks.len(),
        applied_count,
        near_match_count,
        partial_drift_count,
        default_count,
        hazardous_count,
        incompatible_count,
        results: results_map,
        bad_tweaks,
    }
}

fn scan_single_tweak(
    tweak: &TweakDef,
    hw: &HardwareProfile,
    caches: &SystemProbeCaches,
    snapshot_backed_ids: &HashSet<String>,
) -> TweakScanResult {
    let (eligible, eligibility_warnings) = evaluate_eligibility(tweak, hw);
    let op_details: Vec<OpLiveDetail> = tweak
        .ops
        .iter()
        .map(|op| evaluate_single_op(op, tweak.near_match_rule.as_ref(), hw, caches))
        .collect();

    let total = op_details.len().max(1);
    let applied_n = op_details.iter().filter(|d| d.status == "APPLIED").count();
    let near_n = op_details.iter().filter(|d| d.status == "NEAR_MATCH").count();
    let hazard_n = op_details.iter().filter(|d| d.status == "HAZARDOUS").count();
    let not_found_n = op_details.iter().filter(|d| d.status == "NOT_FOUND").count();

    let current_summary = op_details
        .first()
        .map(|d| d.current_value.clone())
        .unwrap_or_else(|| "Default".to_string());

    let (status, status_reason) = if !eligible {
        (
            "INCOMPATIBLE_HARDWARE".to_string(),
            eligibility_warnings
                .first()
                .cloned()
                .unwrap_or_else(|| "Hardware or OS build requirement not met.".to_string()),
        )
    } else if not_found_n == total {
        (
            "NOT_INSTALLED".to_string(),
            "Target Windows component or service is not installed on this edition.".to_string(),
        )
    } else if hazard_n > 0 {
        (
            "HAZARDOUS_CUSTOM".to_string(),
            "Out-of-bounds hazardous value detected on system — safe repair recommended.".to_string(),
        )
    } else if applied_n == total {
        (
            "APPLIED".to_string(),
            "Exact target configuration is live on this system.".to_string(),
        )
    } else if applied_n > 0 && applied_n < total {
        (
            "PARTIAL_DRIFT".to_string(),
            format!(
                "{}/{} operations active — partial state or Windows Update drift detected.",
                applied_n, total
            ),
        )
    } else if near_n > 0 {
        let reason = tweak
            .near_match_rule
            .as_ref()
            .map(|r| format!("{} (live: {})", r.explanation, current_summary))
            .unwrap_or_else(|| {
                format!(
                    "Related custom value '{}' is already active (differs from stock default).",
                    current_summary
                )
            });
        ("NEAR_MATCH".to_string(), reason)
    } else {
        (
            "DEFAULT".to_string(),
            "Windows stock configuration active.".to_string(),
        )
    };

    TweakScanResult {
        id: tweak.id.clone(),
        status,
        status_reason,
        current_summary,
        eligible,
        eligibility_warnings,
        active_conflicts: Vec::new(),
        has_snapshot_backup: snapshot_backed_ids.contains(&tweak.id),
        op_details,
    }
}
