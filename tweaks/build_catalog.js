'use strict';
const fs = require('fs');
const path = require('path');

const cpuGpuGaming = require('./modules/cpu_gpu_gaming');
const netPowerStorage = require('./modules/network_power_storage');
const privAiDebloat = require('./modules/privacy_ai_debloat');
const expSvcTasksSec = require('./modules/explorer_services_tasks_security');

const allTweaks = [
  ...cpuGpuGaming,
  ...netPowerStorage,
  ...privAiDebloat,
  ...expSvcTasksSec,
];

const categories = [
  {
    id: 'cpu_kernel',
    name: 'CPU, Scheduler & Kernel',
    shortName: 'CPU & Kernel',
    icon: 'cpu',
    accent: '#38bdf8',
    description: 'NT scheduler quanta, MMCSS thread priority, core parking, EcoQoS, and kernel ETW/crash policies.',
  },
  {
    id: 'gpu_display',
    name: 'GPU, Display & DirectX',
    shortName: 'GPU & Display',
    icon: 'gpu',
    accent: '#60a5fa',
    description: 'HAGS, MPO flicker mitigation, DXGI Flip-Model, GameDVR, TDR stability, and DWM composition.',
  },
  {
    id: 'gaming_input',
    name: 'Gaming, Input & Peripherals',
    shortName: 'Gaming & Input',
    icon: 'gamepad',
    accent: '#22d3ee',
    description: '1:1 linear mouse curve, HID queues, Game Mode, USB selective suspend, audio ducking, and hotkey traps.',
  },
  {
    id: 'network_tcp',
    name: 'Network, TCP/IP & DNS',
    shortName: 'Network & TCP',
    icon: 'network',
    accent: '#2dd4bf',
    description: 'Per-NIC Nagle control, AFD UDP fast-path, TCP stack tuning, DoH, LLMNR/WPAD defense, and P2P throttling.',
  },
  {
    id: 'power_acpi',
    name: 'Power, ACPI & Thermal',
    shortName: 'Power & ACPI',
    icon: 'bolt',
    accent: '#f59e0b',
    description: 'PCIe ASPM, NVMe APST idle states, Fast Startup, Wake Timers, and hidden powercfg controls.',
  },
  {
    id: 'storage_ntfs',
    name: 'Storage, NTFS & Memory',
    shortName: 'Storage & NTFS',
    icon: 'drive',
    accent: '#34d399',
    description: 'NTFS 8.3/LastAccess wear reduction, Win32 long paths, SSD TRIM integrity, and Reserved Storage.',
  },
  {
    id: 'privacy_telemetry',
    name: 'Privacy & Telemetry',
    shortName: 'Privacy',
    icon: 'shield',
    accent: '#a78bfa',
    description: 'Diagnostic data policy, Advertising ID, Activity History, typing harvesting, and UWP permission locks.',
  },
  {
    id: 'ai_debloat',
    name: 'AI, Copilot, Recall & Debloat',
    shortName: 'AI & Debloat',
    icon: 'sparkles',
    accent: '#f472b6',
    description: 'Windows Recall/Click-to-Do policies, Copilot, Edge Startup Boost, Bing Start search, and ad/bloat blocks.',
  },
  {
    id: 'explorer_ui',
    name: 'Explorer, Shell & Taskbar',
    shortName: 'Explorer & UI',
    icon: 'window',
    accent: '#818cf8',
    description: 'Classic Win11 context menu, End Task right-click, folder auto-discovery fix, extensions, and clean UI.',
  },
  {
    id: 'services',
    name: 'Windows Services (Gated)',
    shortName: 'Services',
    icon: 'cog',
    accent: '#fb923c',
    description: 'Hardware-guarded service tuning with automatic Printer, Bluetooth, Xbox, SSD, and Touch safety checks.',
  },
  {
    id: 'scheduled_tasks',
    name: 'Scheduled Tasks',
    shortName: 'Tasks',
    icon: 'clock',
    accent: '#c084fc',
    description: 'Disable CompatTelRunner CPU spikes, CEIP telemetry collectors, and background maintenance bloat.',
  },
  {
    id: 'security_updates',
    name: 'Security & Update Control',
    shortName: 'Security & WU',
    icon: 'lock',
    accent: '#10b981',
    description: 'Prevent forced reboots, stop WU GPU driver overrides, LSASS/WDigest/PrintNightmare/AutoRun hardening.',
  },
].map((cat) => ({
  ...cat,
  subcategories: [
    ...new Set(allTweaks.filter((t) => t.category === cat.id).map((t) => t.subcategory)),
  ],
}));

// Curated zero-risk engineered presets (pad banks)
const presets = [
  {
    id: 'preset_safe_gaming',
    name: 'Competitive Low-Latency Gaming',
    subtitle: 'Scheduler 0x26, MMCSS, GameDVR Off, 1:1 Mouse, Nagle Off, USB Suspend Off, Audio Ducking Off',
    risk: 'safe',
    tweakIds: [
      'cpu_win32_priority_26',
      'cpu_mmcss_sys_resp_10',
      'cpu_mmcss_net_throttle_off',
      'cpu_mmcss_games_cpu_prio',
      'cpu_mmcss_games_sched_high',
      'cpu_mmcss_games_sfio_high',
      'cpu_power_throttling_off',
      'gpu_disable_gamedvr',
      'gpu_gamedvr_audio_off',
      'game_bar_disable_nexus',
      'input_mouse_raw_linear',
      'input_sticky_keys_off',
      'input_filter_keys_off',
      'input_usb_selective_suspend_off',
      'input_audio_ducking_none',
      'net_disable_nagle_active_nic',
      'net_afd_fast_send_1500',
      'net_delivery_opt_no_p2p',
      'net_qos_unlimit_bandwidth',
      'pwr_pcie_aspm_off',
      'sec_wu_exclude_drivers',
    ],
  },
  {
    id: 'preset_zero_telemetry_ai',
    name: 'Zero-Telemetry & AI Privacy Clean',
    subtitle: 'Disables Telemetry, Recall AI, Copilot, Edge Startup Boost, Bing Start Search, Bloat Ads & DiagTrack',
    risk: 'safe',
    tweakIds: [
      'priv_allow_telemetry_0',
      'priv_disable_feedback_nags',
      'priv_disable_advertising_id',
      'priv_disable_tailored_exp',
      'priv_disable_activity_history',
      'priv_disable_typing_harvest',
      'priv_disable_ceip',
      'priv_disable_app_inventory',
      'ai_disable_windows_copilot',
      'ai_disable_notepad_ai',
      'ai_disable_paint_ai',
      'ai_edge_disable_copilot_sidebar',
      'ai_edge_disable_startup_boost',
      'ai_edge_disable_bg_mode',
      'ai_edge_disable_telemetry',
      'ai_disable_bing_start_search',
      'ai_disable_search_highlights',
      'ai_disable_consumer_features',
      'ai_disable_finish_setup_nag',
      'ai_disable_settings_ads',
      'ai_disable_widgets_policy',
      'svc_diagtrack_disable',
      'svc_dmwappush_disable',
      'task_compat_appraiser',
      'task_ceip_consolidator',
    ],
  },
  {
    id: 'preset_dev_workstation',
    name: 'Developer & Power-User Shell',
    subtitle: 'Win32 Long Paths, No 8.3 Names, Classic Win11 Context Menu, Taskbar End Task, Extensions & Verbose Boot',
    risk: 'safe',
    tweakIds: [
      'stor_ntfs_disable_8dot3',
      'stor_ntfs_disable_last_access',
      'stor_long_paths_enable',
      'stor_enforce_ssd_trim',
      'ui_win11_classic_context_menu',
      'ui_taskbar_end_task',
      'ui_explorer_show_extensions',
      'ui_explorer_show_hidden',
      'ui_explorer_launch_this_pc',
      'ui_explorer_disable_auto_discovery',
      'ui_explorer_full_path_titlebar',
      'ui_menu_show_delay_20',
      'ui_verbose_boot_status',
      'ui_dark_mode_full',
      'pwr_disable_fast_startup',
      'sec_wu_no_auto_reboot',
      'sec_wu_exclude_drivers',
    ],
  },
  {
    id: 'preset_security_shield',
    name: 'Defensive Workstation Hardening',
    subtitle: 'Blocks SMBv1, LLMNR, WPAD, USB AutoRun, WDigest Plaintext, PrintNightmare & MSI Escalation',
    risk: 'safe',
    tweakIds: [
      'net_disable_smbv1',
      'net_disable_llmnr',
      'net_disable_wpad',
      'net_syn_attack_protect',
      'sec_disable_autorun_all',
      'sec_disable_remote_assist',
      'sec_disable_wdigest_cleartext',
      'sec_block_always_install_elevated',
      'sec_restrict_anon_sam',
      'sec_disable_lm_hash',
      'sec_enforce_ntlmv2',
      'sec_uac_secure_desktop',
      'sec_printnightmare_admin_only',
      'sec_safe_dll_search',
      'sec_disable_ip_source_routing',
      'sec_disable_icmp_redirects',
      'svc_remoteregistry_disable',
    ],
  },
];

// Strict schema & integrity validation
const seenIds = new Set();
const catIds = new Set(categories.map((c) => c.id));
for (const t of allTweaks) {
  if (!t.id || seenIds.has(t.id)) {
    throw new Error(`Duplicate or missing tweak ID: ${t.id}`);
  }
  seenIds.add(t.id);
  if (!catIds.has(t.category)) {
    throw new Error(`Invalid category '${t.category}' on tweak '${t.id}'`);
  }
  if (!Array.isArray(t.ops) || t.ops.length === 0) {
    throw new Error(`Tweak '${t.id}' has zero operations!`);
  }
}

for (const t of allTweaks) {
  for (const cid of t.conflictsWith) {
    if (!seenIds.has(cid)) {
      throw new Error(`Tweak '${t.id}' references unknown conflict ID '${cid}'`);
    }
  }
}
for (const p of presets) {
  for (const tid of p.tweakIds) {
    if (!seenIds.has(tid)) {
      throw new Error(`Preset '${p.id}' references unknown tweak ID '${tid}'`);
    }
  }
}

const catalog = {
  version: '1.0.0',
  generatedAt: new Date().toISOString(),
  categories,
  presets,
  tweaks: allTweaks,
};

const outPath = path.join(__dirname, 'catalog.json');
fs.writeFileSync(outPath, JSON.stringify(catalog, null, 2), 'utf8');

const counts = {};
for (const c of categories) {
  counts[c.id] = allTweaks.filter((t) => t.category === c.id).length;
}
console.log(`Catalog built & validated: ${allTweaks.length} tweaks across ${categories.length} categories.`);
console.log(JSON.stringify(counts, null, 2));
