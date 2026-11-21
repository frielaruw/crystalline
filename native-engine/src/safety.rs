use crate::types::{BadTweakFinding, HardwareProfile, TweakDef, TweakOp};
use std::collections::HashMap;
use winreg::enums::*;
use winreg::RegKey;

/// Immutable kernel/boot/security services that Crystalline will NEVER disable or stop.
const CRITICAL_PROTECTED_SERVICES: &[&str] = &[
    "rpcss",
    "dcomlaunch",
    "rpceptmapper",
    "lsm",
    "plugplay",
    "deviceinstall",
    "bfe",
    "mpssvc",
    "windefend",
    "wdnisdrv",
    "securityhealthservice",
    "profsvc",
    "cryptsvc",
    "samss",
    "eventlog",
    "dhcp",
    "dnscache",
    "nsi",
    "trustedinstaller",
    "gpsvc",
    "power",
    "systemeventsbroker",
    "coremessagingregistrar",
    "winmgmt",
];

/// Immutable registry paths whose deletion or corruption can render Windows unbootable.
const CRITICAL_PROTECTED_SUBKEYS: &[&str] = &[
    r"system\currentcontrolset\control\session manager\knowndlls",
    r"system\currentcontrolset\services\partmgr",
    r"system\currentcontrolset\services\volmgr",
    r"system\currentcontrolset\services\mountmgr",
    r"system\currentcontrolset\services\stornvme\parameters\device",
    r"software\microsoft\windows nt\currentversion\winlogon\shell",
];

/// Validates that a tweak operation never violates immutable OS survival invariants.
pub fn validate_op_safety_firewall(op: &TweakOp) -> Result<(), String> {
    match op.kind.as_str() {
        "service" => {
            let svc_low = op.service_name.trim().to_lowercase();
            if CRITICAL_PROTECTED_SERVICES.contains(&svc_low.as_str()) {
                return Err(format!(
                    "Safety Firewall Blocked: Service '{}' is a protected core Windows boot/RPC/security service.",
                    op.service_name
                ));
            }
            if op.apply_value != "2" && op.apply_value != "3" && op.apply_value != "4" {
                return Err(format!(
                    "Safety Firewall Blocked: Invalid service start mode '{}' for '{}'.",
                    op.apply_value, op.service_name
                ));
            }
        }
        "registry" => {
            let path_low = op.subkey.trim().to_lowercase();
            for forbidden in CRITICAL_PROTECTED_SUBKEYS {
                if path_low == *forbidden && (op.value_type == "DELETE_KEY" || op.value_type == "DELETE_VALUE") {
                    return Err(format!(
                        "Safety Firewall Blocked: Destructive operation on critical boot path '{}'.",
                        op.subkey
                    ));
                }
            }
            // Value clamping sanity rules for notorious dangerous registry keys
            let name_low = op.name.trim().to_lowercase();
            if name_low == "waittokillservicetimeout" {
                if let Ok(ms) = op.apply_value.trim().parse::<u64>() {
                    if ms < 1500 {
                        return Err(format!(
                            "Safety Firewall Blocked: WaitToKillServiceTimeout={}ms (<1500ms) risks NTFS/database corruption during shutdown.",
                            ms
                        ));
                    }
                }
            }
            if name_low == "mousedataqueuesize" || name_low == "keyboarddataqueuesize" {
                if let Ok(q) = op.apply_value.trim().parse::<u64>() {
                    if q < 16 {
                        return Err(format!(
                            "Safety Firewall Blocked: {}={} (<16) starves HID ring buffer and drops mouse/keyboard input.",
                            op.name, q
                        ));
                    }
                }
            }
            if name_low == "enablelua" && op.apply_value.trim() == "0" {
                return Err(
                    "Safety Firewall Blocked: Setting EnableLUA=0 breaks UWP/MSIX sandboxing and Windows Security."
                        .to_string(),
                );
            }
        }
        _ => {}
    }
    Ok(())
}

/// Evaluates hardware & OS build eligibility for a tweak, returning `(eligible, warnings)`.
pub fn evaluate_eligibility(tweak: &TweakDef, hw: &HardwareProfile) -> (bool, Vec<String>) {
    let mut eligible = true;
    let mut warnings = Vec::new();
    let e = &tweak.eligibility;

    if let Some(min_b) = e.min_build {
        if hw.build_number < min_b {
            eligible = false;
            warnings.push(format!(
                "Requires Windows Build {} or newer (detected Build {}).",
                min_b, hw.build_number
            ));
        }
    }
    if let Some(max_b) = e.max_build {
        if hw.build_number > max_b {
            eligible = false;
            warnings.push(format!(
                "Only applicable up to Windows Build {} (detected Build {}).",
                max_b, hw.build_number
            ));
        }
    }
    if let Some(min_ram) = e.min_ram_gb {
        if hw.total_ram_gb < min_ram {
            eligible = false;
            warnings.push(format!(
                "Requires at least {} GB RAM to avoid memory exhaustion (detected {} GB).",
                min_ram, hw.total_ram_gb
            ));
        }
    }
    if e.requires_ssd && !hw.primary_drive_ssd {
        eligible = false;
        warnings.push("Requires an SSD/NVMe primary drive (rotational HDD detected).".to_string());
    }
    if e.desktop_only && hw.is_laptop_battery {
        eligible = false;
        warnings.push("Desktop-only tweak: battery-backed laptop detected (prevents sleep/battery loss).".to_string());
    }
    if e.warn_on_laptop && hw.is_laptop_battery {
        warnings.push("Laptop battery detected: this tweak increases idle power draw on battery.".to_string());
    }
    if e.warn_if_printers && !hw.installed_printers.is_empty() {
        warnings.push(format!(
            "Physical/network printer(s) detected ({}) — disabling Print Spooler will stop printing.",
            hw.installed_printers.join(", ")
        ));
    }
    if e.warn_if_bluetooth && hw.has_bluetooth {
        warnings.push("Bluetooth hardware detected — disabling Bluetooth services will disconnect wireless peripherals.".to_string());
    }
    if e.warn_if_xbox && hw.has_xbox_devices {
        warnings.push("Xbox controller / Game Pass service active — modifying this may affect Xbox sign-in or controller sync.".to_string());
    }
    if e.warn_if_touchscreen && hw.has_touchscreen {
        warnings.push("Touch/pen digitizer detected — disabling touch input services will disable screen touch.".to_string());
    }
    if hw.ucpd_active {
        for op in &tweak.ops {
            let sub = op.subkey.to_lowercase();
            if sub.contains("taskband") || sub.contains("shell\\associations") {
                warnings.push("Windows 11 UCPD.sys driver is active and may intercept direct registry writes to this shell key.".to_string());
                break;
            }
        }
    }

    (eligible, warnings)
}

/// Scans the live Windows system for dangerous / broken "bad tweaks" commonly applied by low-quality scripts.
pub fn scan_for_bad_tweaks(
    hw: &HardwareProfile,
    bcd_cache: &HashMap<String, String>,
    netsh_cache: &HashMap<String, String>,
    fsutil_cache: &HashMap<String, String>,
) -> Vec<BadTweakFinding> {
    let mut findings = Vec::new();
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let read64 = KEY_READ | KEY_WOW64_64KEY;

    // 1. WaitToKillServiceTimeout < 1500ms
    if let Ok(ctrl) = hklm.open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Control", read64) {
        if let Ok(val) = ctrl.get_value::<String, _>("WaitToKillServiceTimeout") {
            if let Ok(ms) = val.trim().parse::<u64>() {
                if ms < 1500 {
                    findings.push(BadTweakFinding {
                        id: "heal_wait_to_kill_service".to_string(),
                        title: "Unsafe WaitToKillServiceTimeout (< 1500ms)".to_string(),
                        severity: "critical".to_string(),
                        current_value: format!("{} ms", ms),
                        safe_value: "5000 ms (Windows Default)".to_string(),
                        hazard_explanation: "Forcibly kills Windows services during shutdown before disk/update buffers flush, causing silent file corruption.".to_string(),
                        target_path: r"HKLM\SYSTEM\CurrentControlSet\Control\WaitToKillServiceTimeout".to_string(),
                    });
                }
            }
        }

        // 2. SvcHostSplitThresholdInKB misconfigured when Xbox or default expected
        if let Ok(split) = ctrl.get_value::<u32, _>("SvcHostSplitThresholdInKB") {
            if split > 3_800_000 && hw.total_ram_gb < 12 && hw.has_xbox_devices {
                findings.push(BadTweakFinding {
                    id: "heal_svchost_split".to_string(),
                    title: "Forced SvcHost Grouping Conflicting with XboxGipSvc".to_string(),
                    severity: "warning".to_string(),
                    current_value: format!("{} KB", split),
                    safe_value: "3670016 KB (0x380000 Default)".to_string(),
                    hazard_explanation: "Disabling svchost process isolation is documented to hang XboxGipSvc on 'Starting' and reduce service crash isolation.".to_string(),
                    target_path: r"HKLM\SYSTEM\CurrentControlSet\Control\SvcHostSplitThresholdInKB".to_string(),
                });
            }
        }
    }

    // 3. MouseDataQueueSize < 16
    if let Ok(mou) = hklm.open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services\mouclass\Parameters", read64) {
        if let Ok(q) = mou.get_value::<u32, _>("MouseDataQueueSize") {
            if q > 0 && q < 16 {
                findings.push(BadTweakFinding {
                    id: "heal_mouse_queue".to_string(),
                    title: "Starved MouseDataQueueSize (< 16)".to_string(),
                    severity: "high".to_string(),
                    current_value: format!("{} packets", q),
                    safe_value: "100 packets (Default) or >= 32".to_string(),
                    hazard_explanation: "High-polling-rate mice (1000Hz–8000Hz) overflow tiny HID queues under CPU load, causing cursor stutter and dropped clicks.".to_string(),
                    target_path: r"HKLM\SYSTEM\CurrentControlSet\Services\mouclass\Parameters\MouseDataQueueSize".to_string(),
                });
            }
        }
    }

    // 4. KeyboardDataQueueSize < 16
    if let Ok(kbd) = hklm.open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services\kbdclass\Parameters", read64) {
        if let Ok(q) = kbd.get_value::<u32, _>("KeyboardDataQueueSize") {
            if q > 0 && q < 16 {
                findings.push(BadTweakFinding {
                    id: "heal_kbd_queue".to_string(),
                    title: "Starved KeyboardDataQueueSize (< 16)".to_string(),
                    severity: "high".to_string(),
                    current_value: format!("{} packets", q),
                    safe_value: "100 packets (Default) or >= 32".to_string(),
                    hazard_explanation: "Tiny keyboard input ring buffer drops rapid keystrokes during high CPU utilization.".to_string(),
                    target_path: r"HKLM\SYSTEM\CurrentControlSet\Services\kbdclass\Parameters\KeyboardDataQueueSize".to_string(),
                });
            }
        }
    }

    // 5. MenuShowDelay == 0
    if let Ok(desk) = hkcu.open_subkey_with_flags(r"Control Panel\Desktop", read64) {
        if let Ok(msd) = desk.get_value::<String, _>("MenuShowDelay") {
            if msd.trim() == "0" {
                findings.push(BadTweakFinding {
                    id: "heal_menu_show_delay_zero".to_string(),
                    title: "MenuShowDelay Set to 0ms (Shell Hover Glitch)".to_string(),
                    severity: "warning".to_string(),
                    current_value: "0 ms".to_string(),
                    safe_value: "20 ms (Instant without hover race condition)".to_string(),
                    hazard_explanation: "Zero-millisecond menu delay causes nested context menus to prematurely dismiss when moving cursor diagonally.".to_string(),
                    target_path: r"HKCU\Control Panel\Desktop\MenuShowDelay".to_string(),
                });
            }
        }
    }

    // 6. Spectre / Meltdown / KVA Shadow disabled
    if let Ok(mm) = hklm.open_subkey_with_flags(
        r"SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management",
        read64,
    ) {
        if let Ok(fso) = mm.get_value::<u32, _>("FeatureSettingsOverride") {
            if (fso & 3) == 3 {
                findings.push(BadTweakFinding {
                    id: "heal_cpu_mitigations".to_string(),
                    title: "CPU Speculative Execution Mitigations Disabled".to_string(),
                    severity: "high".to_string(),
                    current_value: format!("FeatureSettingsOverride = {}", fso),
                    safe_value: "Delete override (OS Hardware Default)".to_string(),
                    hazard_explanation: "Exposes kernel memory to speculative-execution side-channel attacks from untrusted browser/application code.".to_string(),
                    target_path: r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\FeatureSettingsOverride".to_string(),
                });
            }
        }
    }

    // 7. SSD TRIM Disabled!
    if hw.primary_drive_ssd {
        if let Some(trim_val) = fsutil_cache.get("DisableDeleteNotification") {
            if trim_val == "1" {
                findings.push(BadTweakFinding {
                    id: "heal_ssd_trim".to_string(),
                    title: "NVMe/SSD TRIM Disabled (DisableDeleteNotification = 1)".to_string(),
                    severity: "critical".to_string(),
                    current_value: "1 (TRIM Disabled)".to_string(),
                    safe_value: "0 (TRIM Enabled)".to_string(),
                    hazard_explanation: "Prevents SSD garbage collection, severely degrading write speeds and shortening NAND flash lifespan.".to_string(),
                    target_path: "fsutil behavior set DisableDeleteNotification 0".to_string(),
                });
            }
        }
    }

    // 8. TCP Receive Window Auto-Tuning Disabled!
    if let Some(autotune) = netsh_cache.get("Receive Window Auto-Tuning Level") {
        if autotune.to_lowercase().contains("disabled") {
            findings.push(BadTweakFinding {
                id: "heal_tcp_autotuning".to_string(),
                title: "TCP Window Auto-Tuning Disabled (Throughput Crippled)".to_string(),
                severity: "critical".to_string(),
                current_value: autotune.clone(),
                safe_value: "normal".to_string(),
                hazard_explanation: "Caps TCP receive window to 64KB, throttling high-speed fiber/cable downloads by up to 80–95% with zero latency benefit.".to_string(),
                target_path: "netsh int tcp set global autotuninglevel=normal".to_string(),
            });
        }
    }

    // 9. BCDEdit useplatformclock = Yes (Forces slow HPET)
    if let Some(upc) = bcd_cache.get("useplatformclock") {
        if upc.eq_ignore_ascii_case("yes") || upc.eq_ignore_ascii_case("true") {
            findings.push(BadTweakFinding {
                id: "heal_bcd_useplatformclock".to_string(),
                title: "Forced HPET Platform Clock (bcdedit useplatformclock = Yes)".to_string(),
                severity: "critical".to_string(),
                current_value: upc.clone(),
                safe_value: "Deleted (Invariant TSC hardware timer)".to_string(),
                hazard_explanation: "Forces Windows off fast CPU Invariant TSC onto high-overhead HPET queries, causing severe micro-stutter and FPS drops on modern CPUs.".to_string(),
                target_path: "bcdedit /deletevalue useplatformclock".to_string(),
            });
        }
    }

    findings
}
