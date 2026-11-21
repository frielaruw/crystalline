mod executor;
mod hardware;
mod safety;
mod scanner;
mod snapshot;
mod types;

use executor::{
    apply_tweaks, generate_dry_run_diff, heal_bad_tweak, restore_snapshot_by_id, revert_tweaks,
};
use hardware::probe_hardware;
use scanner::run_full_scan;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use snapshot::{create_windows_restore_point, get_snapshot_backed_ids, list_snapshots};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use types::{Catalog, TweakDef};

#[derive(Debug, Deserialize)]
struct RpcRequest {
    id: u64,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct RpcResponse {
    id: u64,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Snapshots/emergency files are per-user state — they live in %APPDATA%,
/// never inside the app folder, so the app directory stays portable and
/// contains no machine- or user-identifying data.
fn backup_root() -> PathBuf {
    if let Ok(appdata) = env::var("APPDATA") {
        if !appdata.trim().is_empty() {
            return PathBuf::from(appdata).join("Crystalline").join("backups");
        }
    }
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("backups")
}

fn locate_paths() -> (PathBuf, PathBuf) {
    let exe_dir = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    // Search upward for tweaks/catalog.json
    let mut cur = exe_dir.as_path();
    for _ in 0..5 {
        let candidate = cur.join("tweaks").join("catalog.json");
        if candidate.exists() {
            return (candidate, backup_root());
        }
        if let Some(p) = cur.parent() {
            cur = p;
        } else {
            break;
        }
    }
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    (cwd.join("tweaks").join("catalog.json"), backup_root())
}

fn load_catalog(path: &PathBuf) -> Result<Catalog, String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("Failed reading catalog at {:?}: {}", path, e))?;
    serde_json::from_str::<Catalog>(&raw)
        .map_err(|e| format!("Failed parsing catalog JSON: {}", e))
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let (catalog_path, backup_root) = locate_paths();

    let catalog = match load_catalog(&catalog_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("FATAL: {}", e);
            std::process::exit(1);
        }
    };

    let tweak_index: HashMap<String, TweakDef> = catalog
        .tweaks
        .iter()
        .cloned()
        .map(|t| (t.id.clone(), t))
        .collect();

    if args.iter().any(|a| a == "--self-test") {
        let hw = probe_hardware();
        let backed = get_snapshot_backed_ids(&backup_root);
        let report = run_full_scan(&catalog, &hw, &backed);
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "engine": "crystalline-rust-native-1.1",
                "os": hw.os_caption,
                "build": hw.build_number,
                "cpu": hw.cpu_name,
                "ramGb": hw.total_ram_gb,
                "elevated": hw.is_elevated,
                "ssd": hw.primary_drive_ssd,
                "nvme": hw.nvme_present,
                "totalTweaks": report.total_tweaks,
                "scanDurationMs": report.scan_duration_ms,
                "appliedCount": report.applied_count,
                "nearMatchCount": report.near_match_count,
                "partialDriftCount": report.partial_drift_count,
                "defaultCount": report.default_count,
                "hazardousCount": report.hazardous_count,
                "incompatibleCount": report.incompatible_count,
                "badTweaksDetected": report.bad_tweaks.len()
            }))
            .unwrap()
        );
        return;
    }

    // Persistent stdio JSON-RPC server loop for sub-millisecond IPC from Electron
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            Ok(_) => continue,
            Err(_) => break,
        };

        let req: RpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = RpcResponse {
                    id: 0,
                    ok: false,
                    result: None,
                    error: Some(format!("Invalid JSON-RPC request: {}", e)),
                };
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };

        let resp = handle_request(req, &catalog, &tweak_index, &backup_root);
        let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
        let _ = stdout.flush();
    }
}

fn extract_tweak_refs<'a>(
    params: &Value,
    tweak_index: &'a HashMap<String, TweakDef>,
) -> Vec<&'a TweakDef> {
    params
        .get("tweakIds")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|id_v| id_v.as_str().and_then(|id| tweak_index.get(id)))
                .collect()
        })
        .unwrap_or_default()
}

fn handle_request(
    req: RpcRequest,
    catalog: &Catalog,
    tweak_index: &HashMap<String, TweakDef>,
    backup_root: &PathBuf,
) -> RpcResponse {
    let hw = probe_hardware();

    match req.method.as_str() {
        "bootstrap" => {
            let backed = get_snapshot_backed_ids(backup_root);
            let scan = run_full_scan(catalog, &hw, &backed);
            let snapshots = list_snapshots(backup_root);
            RpcResponse {
                id: req.id,
                ok: true,
                result: Some(json!({
                    "hardware": hw,
                    "scan": scan,
                    "snapshots": snapshots
                })),
                error: None,
            }
        }
        "scan" => {
            let backed = get_snapshot_backed_ids(backup_root);
            let scan = run_full_scan(catalog, &hw, &backed);
            RpcResponse {
                id: req.id,
                ok: true,
                result: Some(serde_json::to_value(scan).unwrap()),
                error: None,
            }
        }
        "hardware" => RpcResponse {
            id: req.id,
            ok: true,
            result: Some(serde_json::to_value(hw).unwrap()),
            error: None,
        },
        "dry_run" => {
            let tweaks = extract_tweak_refs(&req.params, tweak_index);
            let mode = req
                .params
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("apply");
            let diff = generate_dry_run_diff(&tweaks, &hw, mode, backup_root);
            RpcResponse {
                id: req.id,
                ok: true,
                result: Some(serde_json::to_value(diff).unwrap()),
                error: None,
            }
        }
        "apply" => {
            let tweaks = extract_tweak_refs(&req.params, tweak_index);
            let allow_override = req
                .params
                .get("allowOverride")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let exec_rep = apply_tweaks(&tweaks, &hw, backup_root, allow_override);
            let backed = get_snapshot_backed_ids(backup_root);
            let scan = run_full_scan(catalog, &hw, &backed);
            let snapshots = list_snapshots(backup_root);
            RpcResponse {
                id: req.id,
                ok: true, // per-op outcomes live in report.succeeded/failed
                result: Some(json!({
                    "report": exec_rep,
                    "scan": scan,
                    "snapshots": snapshots
                })),
                error: None,
            }
        }
        "revert" => {
            let tweaks = extract_tweak_refs(&req.params, tweak_index);
            let exec_rep = revert_tweaks(&tweaks, &hw, backup_root);
            let backed = get_snapshot_backed_ids(backup_root);
            let scan = run_full_scan(catalog, &hw, &backed);
            RpcResponse {
                id: req.id,
                ok: true,
                result: Some(json!({
                    "report": exec_rep,
                    "scan": scan
                })),
                error: None,
            }
        }
        "restore_snapshot" => {
            let snap_id = req
                .params
                .get("snapshotId")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let exec_rep = restore_snapshot_by_id(snap_id, &hw, backup_root);
            let backed = get_snapshot_backed_ids(backup_root);
            let scan = run_full_scan(catalog, &hw, &backed);
            RpcResponse {
                id: req.id,
                ok: true,
                result: Some(json!({
                    "report": exec_rep,
                    "scan": scan
                })),
                error: None,
            }
        }
        "list_snapshots" => RpcResponse {
            id: req.id,
            ok: true,
            result: Some(serde_json::to_value(list_snapshots(backup_root)).unwrap()),
            error: None,
        },
        "create_restore_point" => {
            let label = req
                .params
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("Manual Checkpoint");
            match create_windows_restore_point(label, hw.is_elevated) {
                Ok(msg) => RpcResponse {
                    id: req.id,
                    ok: true,
                    result: Some(json!({ "message": msg })),
                    error: None,
                },
                Err(err) => RpcResponse {
                    id: req.id,
                    ok: false,
                    result: None,
                    error: Some(err),
                },
            }
        }
        "heal_bad_tweak" => {
            let fid = req
                .params
                .get("findingId")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match heal_bad_tweak(fid, &hw) {
                Ok(msg) => {
                    let backed = get_snapshot_backed_ids(backup_root);
                    let scan = run_full_scan(catalog, &hw, &backed);
                    RpcResponse {
                        id: req.id,
                        ok: true,
                        result: Some(json!({ "message": msg, "scan": scan })),
                        error: None,
                    }
                }
                Err(err) => RpcResponse {
                    id: req.id,
                    ok: false,
                    result: None,
                    error: Some(err),
                },
            }
        }
        "restart_explorer" => {
            // `&` not `&&`: if taskkill fails (explorer already dead) we still
            // relaunch it, otherwise the user is stranded without a shell.
            let _ = std::process::Command::new("cmd")
                .args(["/C", "taskkill /f /im explorer.exe & start explorer.exe"])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .spawn();
            RpcResponse {
                id: req.id,
                ok: true,
                result: Some(json!({ "message": "Windows Explorer restarted." })),
                error: None,
            }
        }
        other => RpcResponse {
            id: req.id,
            ok: false,
            result: None,
            error: Some(format!("Unknown RPC method: {}", other)),
        },
    }
}
