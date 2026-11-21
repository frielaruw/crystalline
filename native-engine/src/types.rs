use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub version: String,
    pub generated_at: String,
    pub categories: Vec<CategoryMeta>,
    pub presets: Vec<PresetDef>,
    pub tweaks: Vec<TweakDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryMeta {
    pub id: String,
    pub name: String,
    pub short_name: String,
    pub icon: String,
    pub accent: String,
    pub description: String,
    pub subcategories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetDef {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub risk: String,
    pub tweak_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakDef {
    pub id: String,
    pub title: String,
    pub pad_label: String,
    pub category: String,
    pub subcategory: String,
    pub description: String,
    pub technical_notes: String,
    pub safety_notes: String,
    pub risk: String, // "safe" | "moderate" | "caution" | "expert"
    pub reboot_required: bool,
    pub explorer_restart: bool,
    pub requires_admin: bool,
    pub tags: Vec<String>,
    #[serde(default)]
    pub eligibility: EligibilityRule,
    #[serde(default)]
    pub conflicts_with: Vec<String>,
    #[serde(default)]
    pub near_match_rule: Option<NearMatchRule>,
    pub ops: Vec<TweakOp>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EligibilityRule {
    pub min_build: Option<u32>,
    pub max_build: Option<u32>,
    pub min_ram_gb: Option<u32>,
    #[serde(default)]
    pub requires_ssd: bool,
    #[serde(default)]
    pub desktop_only: bool,
    #[serde(default)]
    pub warn_on_laptop: bool,
    #[serde(default)]
    pub warn_if_printers: bool,
    #[serde(default)]
    pub warn_if_bluetooth: bool,
    #[serde(default)]
    pub warn_if_xbox: bool,
    #[serde(default)]
    pub warn_if_touchscreen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NearMatchRule {
    pub kind: String, // "dword_set" | "numeric_range" | "non_default" | "service_alt"
    #[serde(default)]
    pub values: Vec<i64>,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub toxic_below: Option<i64>,
    pub toxic_above: Option<i64>,
    #[serde(default)]
    pub toxic_values: Vec<i64>,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakOp {
    pub kind: String, // "registry" | "service" | "schtask" | "powercfg" | "netsh_tcp" | "fsutil"
    #[serde(default)]
    pub hive: String,
    #[serde(default)]
    pub subkey: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value_type: String, // "DWORD" | "QWORD" | "SZ" | "EXPAND_SZ" | "BINARY" | "KEY_ONLY" | "DELETE_VALUE"
    #[serde(default)]
    pub apply_value: String,
    #[serde(default)]
    pub default_value: String,
    #[serde(default)]
    pub service_name: String,
    #[serde(default)]
    pub task_path: String,
    #[serde(default)]
    pub subgroup_guid: String,
    #[serde(default)]
    pub setting_guid: String,
    #[serde(default)]
    pub param_key: String,
    #[serde(default)]
    pub apply_cmd: String,
    #[serde(default)]
    pub revert_cmd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareProfile {
    pub os_caption: String,
    pub os_edition: String,
    pub os_display_version: String,
    pub build_number: u32,
    pub is_windows_11: bool,
    pub cpu_name: String,
    pub logical_cores: u32,
    pub total_ram_mb: u64,
    pub available_ram_mb: u64,
    pub total_ram_gb: u32,
    pub gpu_names: Vec<String>,
    pub primary_drive_ssd: bool,
    pub nvme_present: bool,
    pub is_laptop_battery: bool,
    pub has_touchscreen: bool,
    pub has_bluetooth: bool,
    pub installed_printers: Vec<String>,
    pub has_xbox_devices: bool,
    pub active_nic_guids: Vec<String>,
    pub active_power_scheme: String,
    pub is_elevated: bool,
    pub vbs_enabled: bool,
    pub ucpd_active: bool,
    pub system_restore_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpLiveDetail {
    pub target_label: String,
    pub current_value: String,
    pub expected_apply: String,
    pub expected_default: String,
    pub status: String, // "APPLIED" | "DEFAULT" | "NEAR_MATCH" | "HAZARDOUS" | "NOT_FOUND"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakScanResult {
    pub id: String,
    pub status: String, // "APPLIED" | "NEAR_MATCH" | "PARTIAL_DRIFT" | "DEFAULT" | "HAZARDOUS_CUSTOM" | "INCOMPATIBLE_HARDWARE" | "NOT_INSTALLED"
    pub status_reason: String,
    pub current_summary: String,
    pub eligible: bool,
    pub eligibility_warnings: Vec<String>,
    pub active_conflicts: Vec<String>,
    pub has_snapshot_backup: bool,
    pub op_details: Vec<OpLiveDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BadTweakFinding {
    pub id: String,
    pub title: String,
    pub severity: String, // "critical" | "high" | "warning"
    pub current_value: String,
    pub safe_value: String,
    pub hazard_explanation: String,
    pub target_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullScanReport {
    pub scan_duration_ms: f64,
    pub total_tweaks: usize,
    pub applied_count: usize,
    pub near_match_count: usize,
    pub partial_drift_count: usize,
    pub default_count: usize,
    pub hazardous_count: usize,
    pub incompatible_count: usize,
    pub results: HashMap<String, TweakScanResult>,
    pub bad_tweaks: Vec<BadTweakFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturedOpState {
    pub op_index: usize,
    pub kind: String,
    pub resolved_hive: String,
    pub resolved_subkey: String,
    pub name: String,
    pub existed: bool,
    pub captured_type: String,
    pub captured_value: String,
    pub service_name: String,
    pub task_path: String,
    pub subgroup_guid: String,
    pub setting_guid: String,
    pub revert_cmd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRecord {
    pub snapshot_id: String,
    pub created_at: String,
    pub label: String,
    pub tweak_ids: Vec<String>,
    pub sha256: String,
    pub emergency_reg_path: String,
    pub emergency_cmd_path: String,
    pub states: HashMap<String, Vec<CapturedOpState>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffEntry {
    pub tweak_id: String,
    pub tweak_title: String,
    pub risk: String,
    pub requires_admin: bool,
    pub target: String,
    pub before: String,
    pub after: String,
    pub warnings: Vec<String>,
    pub blocked: bool,
    pub block_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionReport {
    pub ok: bool,
    pub action: String,
    pub duration_ms: f64,
    pub snapshot_id: Option<String>,
    pub emergency_reg_path: Option<String>,
    pub succeeded: Vec<String>,
    pub failed: Vec<(String, String)>,
    pub ucpd_warnings: Vec<String>,
    pub reboot_needed: bool,
    pub explorer_restart_needed: bool,
}
