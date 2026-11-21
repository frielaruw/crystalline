use crate::types::HardwareProfile;
use winreg::enums::*;
use winreg::RegKey;

#[repr(C)]
struct MemoryStatusEx {
    dw_length: u32,
    dw_memory_load: u32,
    ull_total_phys: u64,
    ull_avail_phys: u64,
    ull_total_page_file: u64,
    ull_avail_page_file: u64,
    ull_total_virtual: u64,
    ull_avail_virtual: u64,
    ull_avail_extended_virtual: u64,
}

#[repr(C)]
struct SystemPowerStatus {
    ac_line_status: u8,
    battery_flag: u8,
    battery_life_percent: u8,
    system_status_flag: u8,
    battery_life_time: u32,
    battery_full_life_time: u32,
}

#[repr(C)]
struct SystemInfo {
    w_processor_architecture: u16,
    w_reserved: u16,
    dw_page_size: u32,
    lp_minimum_application_address: *mut core::ffi::c_void,
    lp_maximum_application_address: *mut core::ffi::c_void,
    dw_active_processor_mask: usize,
    dw_number_of_processors: u32,
    dw_processor_type: u32,
    dw_allocation_granularity: u32,
    w_processor_level: u16,
    w_processor_revision: u16,
}

#[repr(C)]
struct TokenElevation {
    token_is_elevated: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalMemoryStatusEx(lp_buffer: *mut MemoryStatusEx) -> i32;
    fn GetSystemPowerStatus(lp_system_power_status: *mut SystemPowerStatus) -> i32;
    fn GetSystemInfo(lp_system_info: *mut SystemInfo);
    fn GetCurrentProcess() -> *mut core::ffi::c_void;
    fn CloseHandle(h_object: *mut core::ffi::c_void) -> i32;
}

#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(
        process_handle: *mut core::ffi::c_void,
        desired_access: u32,
        token_handle: *mut *mut core::ffi::c_void,
    ) -> i32;
    fn GetTokenInformation(
        token_handle: *mut core::ffi::c_void,
        token_information_class: u32,
        token_information: *mut core::ffi::c_void,
        token_information_length: u32,
        return_length: *mut u32,
    ) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn GetSystemMetrics(n_index: i32) -> i32;
}

pub fn check_is_elevated() -> bool {
    unsafe {
        let mut token: *mut core::ffi::c_void = core::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), 0x0008, &mut token) == 0 {
            return false;
        }
        let mut elevation = TokenElevation {
            token_is_elevated: 0,
        };
        let mut ret_len: u32 = 0;
        let ok = GetTokenInformation(
            token,
            20, // TokenElevation
            &mut elevation as *mut _ as *mut core::ffi::c_void,
            core::mem::size_of::<TokenElevation>() as u32,
            &mut ret_len,
        );
        CloseHandle(token);
        ok != 0 && elevation.token_is_elevated != 0
    }
}

pub fn probe_hardware() -> HardwareProfile {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let read64 = KEY_READ | KEY_WOW64_64KEY;

    // 1. OS Build & Edition
    let (mut os_caption, mut os_edition, mut os_display_version, mut build_number) = (
        "Windows".to_string(),
        "Pro".to_string(),
        "".to_string(),
        19045u32,
    );
    if let Ok(cv) = hklm.open_subkey_with_flags(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", read64) {
        if let Ok(p) = cv.get_value::<String, _>("ProductName") {
            os_caption = p;
        }
        if let Ok(e) = cv.get_value::<String, _>("EditionID") {
            os_edition = e;
        }
        if let Ok(dv) = cv.get_value::<String, _>("DisplayVersion") {
            os_display_version = dv;
        }
        if let Ok(b_str) = cv.get_value::<String, _>("CurrentBuildNumber") {
            if let Ok(b) = b_str.trim().parse::<u32>() {
                build_number = b;
            }
        }
    }
    let is_windows_11 = build_number >= 22000;
    if is_windows_11 && os_caption.contains("Windows 10") {
        os_caption = os_caption.replace("Windows 10", "Windows 11");
    }

    // 2. CPU & Logical Cores
    let mut cpu_name = "x86_64 Processor".to_string();
    if let Ok(cpu0) = hklm.open_subkey_with_flags(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0", read64) {
        if let Ok(n) = cpu0.get_value::<String, _>("ProcessorNameString") {
            cpu_name = n.trim().to_string();
        }
    }
    let logical_cores = unsafe {
        let mut si: SystemInfo = core::mem::zeroed();
        GetSystemInfo(&mut si);
        si.dw_number_of_processors.max(1)
    };

    // 3. Physical RAM
    let (total_ram_mb, available_ram_mb) = unsafe {
        let mut mem: MemoryStatusEx = core::mem::zeroed();
        mem.dw_length = core::mem::size_of::<MemoryStatusEx>() as u32;
        if GlobalMemoryStatusEx(&mut mem) != 0 {
            (
                mem.ull_total_phys / (1024 * 1024),
                mem.ull_avail_phys / (1024 * 1024),
            )
        } else {
            (16384, 8192)
        }
    };
    let total_ram_gb = ((total_ram_mb as f64) / 1024.0).round() as u32;

    // 4. GPUs
    let mut gpu_names = Vec::new();
    if let Ok(disp) = hklm.open_subkey_with_flags(
        r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}",
        read64,
    ) {
        for idx in ["0000", "0001", "0002", "0003"] {
            if let Ok(sub) = disp.open_subkey_with_flags(idx, read64) {
                if let Ok(desc) = sub.get_value::<String, _>("DriverDesc") {
                    let trimmed = desc.trim().to_string();
                    if !trimmed.is_empty()
                        && !trimmed.contains("Basic Display")
                        && !gpu_names.contains(&trimmed)
                    {
                        gpu_names.push(trimmed);
                    }
                }
            }
        }
    }

    // 5. Storage NVMe / SSD inspection
    let mut nvme_present = false;
    if let Ok(nvme_svc) = hklm.open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services\stornvme", read64) {
        if let Ok(start) = nvme_svc.get_value::<u32, _>("Start") {
            if start == 0 {
                nvme_present = true;
            }
        }
        if nvme_svc.open_subkey_with_flags("Enum", read64).map(|e| e.get_value::<u32, _>("Count").unwrap_or(0) > 0).unwrap_or(false) {
            nvme_present = true;
        }
    }
    let mut primary_drive_ssd = nvme_present;
    if !primary_drive_ssd {
        if let Ok(disk_enum) = hklm.open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services\disk\Enum", read64) {
            if let Ok(disk0) = disk_enum.get_value::<String, _>("0") {
                let up = disk0.to_uppercase();
                if up.contains("NVME") || up.contains("SSD") || up.contains("WDS") || up.contains("SAMSUNG") || up.contains("CRUCIAL") || up.contains("KINGSTON") || up.contains("VIRTIO") {
                    primary_drive_ssd = true;
                }
            }
        }
    }

    // 6. Battery / Laptop vs Desktop
    let is_laptop_battery = unsafe {
        let mut sps: SystemPowerStatus = core::mem::zeroed();
        if GetSystemPowerStatus(&mut sps) != 0 {
            sps.battery_flag != 128 && sps.battery_flag != 255
        } else {
            false
        }
    };

    // 7. Touchscreen
    let has_touchscreen = unsafe {
        let dig = GetSystemMetrics(94); // SM_DIGITIZER
        (dig & 0x07) != 0
    };

    // 8. Bluetooth
    let mut has_bluetooth = false;
    if let Ok(bth) = hklm.open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services\BTHPORT\Parameters\Devices", read64) {
        if bth.enum_keys().next().is_some() {
            has_bluetooth = true;
        }
    }
    if !has_bluetooth {
        if let Ok(bth_enum) = hklm.open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Enum\BTH", read64) {
            if bth_enum.enum_keys().next().is_some() {
                has_bluetooth = true;
            }
        }
    }

    // 9. Physical / Network Printers
    let mut installed_printers = Vec::new();
    if let Ok(printers) = hklm.open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Control\Print\Printers", read64) {
        for key_name in printers.enum_keys().flatten() {
            let low = key_name.to_lowercase();
            let is_virtual = low.contains("print to pdf")
                || low.contains("xps document writer")
                || low == "fax"
                || low.contains("onenote");
            if !is_virtual {
                installed_printers.push(key_name);
            }
        }
    }

    // 10. Xbox / Gamepad devices
    let mut has_xbox_devices = false;
    if let Ok(xgip) = hklm.open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services\XboxGipSvc", read64) {
        if let Ok(s) = xgip.get_value::<u32, _>("Start") {
            if s == 2 {
                has_xbox_devices = true;
            }
        }
    }

    // 11. Active IPv4 NIC Interface GUIDs
    let mut active_nic_guids = Vec::new();
    if let Ok(ifaces) = hklm.open_subkey_with_flags(
        r"SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces",
        read64,
    ) {
        for guid in ifaces.enum_keys().flatten() {
            if let Ok(ifk) = ifaces.open_subkey_with_flags(&guid, read64) {
                let dhcp_ip = ifk.get_value::<String, _>("DhcpIPAddress").unwrap_or_default();
                let static_ip = ifk
                    .get_value::<Vec<String>, _>("IPAddress")
                    .ok()
                    .and_then(|v| v.into_iter().next())
                    .unwrap_or_default();
                let ip = if !dhcp_ip.is_empty() && dhcp_ip != "0.0.0.0" {
                    dhcp_ip
                } else {
                    static_ip
                };
                if !ip.is_empty() && ip != "0.0.0.0" && !ip.starts_with("127.") && !ip.starts_with("169.254.") {
                    active_nic_guids.push(guid);
                }
            }
        }
    }

    // 12. Active Power Scheme
    let mut active_power_scheme = "381b4222-f694-41f0-9685-ff5bb260df2e".to_string();
    if let Ok(ps) = hklm.open_subkey_with_flags(
        r"SYSTEM\CurrentControlSet\Control\Power\User\PowerSchemes",
        read64,
    ) {
        if let Ok(g) = ps.get_value::<String, _>("ActivePowerScheme") {
            if !g.trim().is_empty() {
                active_power_scheme = g.trim().to_string();
            }
        }
    }

    // 13. VBS / HVCI
    let vbs_enabled = hklm
        .open_subkey_with_flags(
            r"SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity",
            read64,
        )
        .and_then(|k| k.get_value::<u32, _>("Enabled"))
        .unwrap_or(0)
        == 1;

    // 14. UCPD.sys (Windows 11 User Choice Protection Driver)
    let ucpd_active = hklm
        .open_subkey_with_flags(r"SYSTEM\CurrentControlSet\Services\UCPD", read64)
        .and_then(|k| k.get_value::<u32, _>("Start"))
        .map(|s| s <= 2)
        .unwrap_or(false);

    // 15. System Restore enabled
    let system_restore_enabled = hklm
        .open_subkey_with_flags(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\SystemRestore",
            read64,
        )
        .and_then(|k| k.get_value::<u32, _>("RPSessionInterval"))
        .unwrap_or(0)
        > 0;

    HardwareProfile {
        os_caption,
        os_edition,
        os_display_version,
        build_number,
        is_windows_11,
        cpu_name,
        logical_cores,
        total_ram_mb,
        available_ram_mb,
        total_ram_gb,
        gpu_names,
        primary_drive_ssd,
        nvme_present,
        is_laptop_battery,
        has_touchscreen,
        has_bluetooth,
        installed_printers,
        has_xbox_devices,
        active_nic_guids,
        active_power_scheme,
        is_elevated: check_is_elevated(),
        vbs_enabled,
        ucpd_active,
        system_restore_enabled,
    }
}
