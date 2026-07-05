use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;

static SCAN_CANCELLED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SortBy {
    Size,
    LastUsed,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum NodeModulesMode {
    Stale,
    All,
}

#[derive(Debug, Deserialize, Clone, Default)]
struct DockerOptions {
    #[serde(default)]
    dangling_images: bool,
    #[serde(default)]
    unused_images: bool,
    #[serde(default)]
    stopped_containers: bool,
    #[serde(default)]
    build_cache: bool,
    #[serde(default)]
    volumes: bool,
}

#[derive(Debug, Deserialize)]
struct ScanRequest {
    #[serde(default)]
    scan_paths: Vec<String>,
    #[serde(default)]
    ignore_patterns: Vec<String>,
    #[serde(default = "default_sort")]
    sort_by: SortBy,
    #[serde(default = "default_node_mode")]
    node_modules_mode: NodeModulesMode,
    #[serde(default = "default_stale_days")]
    stale_days: u64,
    #[serde(default)]
    docker_options: DockerOptions,
}

#[derive(Debug, Deserialize)]
struct CleanupRequest {
    items: Vec<CleanupItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CleanupItem {
    id: String,
    kind: String,
    path: String,
    size_bytes: u64,
    last_used_unix: u64,
    risk: String,
    source: String,
}

#[derive(Debug, Serialize)]
struct ScanResponse {
    total_bytes: u64,
    item_count: usize,
    items: Vec<CleanupItem>,
    cancelled: bool,
}

#[derive(Debug, Serialize)]
struct CleanupResponse {
    freed_bytes: u64,
    success_count: usize,
    failed_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct LogPayload {
    level: String,
    message: String,
}

fn default_sort() -> SortBy {
    SortBy::Size
}

fn default_node_mode() -> NodeModulesMode {
    NodeModulesMode::Stale
}

fn default_stale_days() -> u64 {
    30
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn metadata_unix_time(path: &Path) -> u64 {
    match fs::metadata(path) {
        Ok(meta) => {
            let best = meta
                .accessed()
                .or_else(|_| meta.modified())
                .or_else(|_| meta.created());
            best.and_then(|t| t.duration_since(UNIX_EPOCH).map_err(std::io::Error::other))
                .map(|d| d.as_secs())
                .unwrap_or(0)
        }
        Err(_) => 0,
    }
}

fn dir_size(path: &Path) -> Option<u64> {
    let mut total = 0_u64;
    let mut processed = 0_u64;
    for entry in WalkDir::new(path).follow_links(false).into_iter().flatten() {
        processed = processed.saturating_add(1);
        if processed % 512 == 0 && SCAN_CANCELLED.load(Ordering::Relaxed) {
            return None;
        }
        if entry.file_type().is_file() {
            if let Ok(meta) = entry.metadata() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Some(total)
}

fn normalize_patterns(raw_patterns: &[String]) -> Vec<String> {
    raw_patterns
        .iter()
        .filter_map(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_lowercase())
            }
        })
        .collect()
}

fn should_ignore(path: &Path, patterns: &[String]) -> bool {
    let lower = path.to_string_lossy().to_lowercase();
    patterns.iter().any(|p| lower.contains(p))
}

fn default_scan_roots() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let mut roots = Vec::new();
        for drive in b'A'..=b'Z' {
            let p = format!("{}:\\\\", drive as char);
            let pb = PathBuf::from(p);
            if pb.exists() {
                roots.push(pb);
            }
        }
        return roots;
    }

    #[cfg(not(target_os = "windows"))]
    {
        vec![PathBuf::from("/")]
    }
}

fn default_ignore_patterns() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        vec![
            "\\windows\\".into(),
            "\\program files\\".into(),
            "\\program files (x86)\\".into(),
            "\\$recycle.bin\\".into(),
            "\\system volume information\\".into(),
        ]
    }

    #[cfg(not(target_os = "windows"))]
    {
        vec![
            "/system/".into(),
            "/private/".into(),
            "/applications/".into(),
            "/volumes/".into(),
            "/dev/".into(),
            "/proc/".into(),
            "/sys/".into(),
            "/tmp/".into(),
        ]
    }
}

fn cache_dirs() -> Vec<(String, PathBuf, String)> {
    let mut results = Vec::new();

    #[cfg(target_os = "windows")]
    {
        let local_app_data = std::env::var("LOCALAPPDATA").ok().map(PathBuf::from);
        let app_data = std::env::var("APPDATA").ok().map(PathBuf::from);

        if let Some(dir) = local_app_data.clone() {
            results.push((
                "npm_cache".into(),
                dir.join("npm-cache"),
                "Clearing cache may slow next package install".into(),
            ));
            results.push((
                "pnpm_cache".into(),
                dir.join("pnpm").join("store"),
                "Clearing cache may slow next package install".into(),
            ));
            results.push((
                "yarn_cache".into(),
                dir.join("Yarn").join("Cache"),
                "Clearing cache may slow next package install".into(),
            ));
        }

        if let Some(dir) = app_data {
            results.push((
                "npm_cache".into(),
                dir.join("npm-cache"),
                "Clearing cache may slow next package install".into(),
            ));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home) = dirs::home_dir() {
            results.push((
                "npm_cache".into(),
                home.join(".npm"),
                "Clearing cache may slow next package install".into(),
            ));
            results.push((
                "pnpm_cache".into(),
                home.join(".pnpm-store"),
                "Clearing cache may slow next package install".into(),
            ));
            results.push((
                "pnpm_cache".into(),
                home.join("Library").join("Caches").join("pnpm"),
                "Clearing cache may slow next package install".into(),
            ));
            results.push((
                "yarn_cache".into(),
                home.join("Library").join("Caches").join("Yarn"),
                "Clearing cache may slow next package install".into(),
            ));
            results.push((
                "yarn_cache".into(),
                home.join(".yarn").join("cache"),
                "Clearing cache may slow next package install".into(),
            ));
        }
    }

    results
}

fn parse_human_size(s: &str) -> u64 {
    let token = s
        .trim()
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .replace(',', "")
        .to_uppercase();
    if token.is_empty() {
        return 0;
    }

    let units = [
        ("TB", 1024_u64.pow(4)),
        ("GB", 1024_u64.pow(3)),
        ("MB", 1024_u64.pow(2)),
        ("KB", 1024_u64),
        ("B", 1_u64),
    ];

    for (unit, mul) in units {
        if token.ends_with(unit) {
            let num = token.trim_end_matches(unit);
            if let Ok(v) = num.parse::<f64>() {
                return (v * mul as f64) as u64;
            }
        }
    }

    token.parse::<u64>().unwrap_or(0)
}

fn docker_available() -> bool {
    Command::new("docker")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn scan_docker_dangling_bytes() -> u64 {
    let out = Command::new("docker")
        .args(["image", "ls", "--filter", "dangling=true", "--format", "{{.Size}}"])
        .output();

    let Ok(out) = out else {
        return 0;
    };
    if !out.status.success() {
        return 0;
    }

    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(parse_human_size)
        .sum()
}

fn scan_docker_df_reclaimable() -> std::collections::HashMap<String, u64> {
    let mut reclaim = std::collections::HashMap::new();
    let out = Command::new("docker")
        .args(["system", "df", "--format", "{{json .}}"])
        .output();

    let Ok(out) = out else {
        return reclaim;
    };
    if !out.status.success() {
        return reclaim;
    }

    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            let ty = v
                .get("Type")
                .and_then(|x| x.as_str())
                .unwrap_or_default()
                .to_lowercase();
            let reclaimable_raw = v
                .get("Reclaimable")
                .and_then(|x| x.as_str())
                .unwrap_or_default();
            let reclaimable_num = reclaimable_raw.split('(').next().unwrap_or_default();
            let bytes = parse_human_size(reclaimable_num);
            if !ty.is_empty() {
                reclaim.insert(ty, bytes);
            }
        }
    }

    reclaim
}

fn push_item(
    items: &mut Vec<CleanupItem>,
    kind: &str,
    path: String,
    size_bytes: u64,
    last_used_unix: u64,
    risk: &str,
    source: &str,
) {
    if size_bytes == 0 {
        return;
    }
    let id = format!("{}:{}", kind, path);
    items.push(CleanupItem {
        id,
        kind: kind.to_string(),
        path,
        size_bytes,
        last_used_unix,
        risk: risk.to_string(),
        source: source.to_string(),
    });
}

#[tauri::command]
fn scan_cleanup_targets(payload: ScanRequest, app: AppHandle) -> Result<ScanResponse, String> {
    let mut items: Vec<CleanupItem> = Vec::new();
    let mut cancelled = false;
    let mut scanned_dirs = 0_u64;
    SCAN_CANCELLED.store(false, Ordering::SeqCst);
    emit_log(&app, "info", "Scan started");

    let scan_roots: Vec<PathBuf> = if payload.scan_paths.is_empty() {
        default_scan_roots()
    } else {
        payload
            .scan_paths
            .iter()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .collect()
    };

    let mut patterns = normalize_patterns(&payload.ignore_patterns);
    patterns.extend(default_ignore_patterns());

    let now = now_unix();
    let stale_before = now.saturating_sub(payload.stale_days.saturating_mul(86_400));

    'roots: for root in scan_roots {
        emit_log(&app, "info", format!("Scanning root: {}", root.to_string_lossy()));
        let mut iter = WalkDir::new(&root).follow_links(false).into_iter();

        while let Some(entry) = iter.next() {
            if SCAN_CANCELLED.load(Ordering::Relaxed) {
                cancelled = true;
                emit_log(&app, "warn", "Scan cancelled by user");
                break 'roots;
            }
            let Ok(entry) = entry else {
                continue;
            };

            let p = entry.path();
            if entry.file_type().is_dir() {
                scanned_dirs = scanned_dirs.saturating_add(1);
                if scanned_dirs % 400 == 0 {
                    emit_log(
                        &app,
                        "info",
                        format!(
                            "Progress: scanned {} directories, found {} candidate(s)",
                            scanned_dirs,
                            items.len()
                        ),
                    );
                }
            }
            if entry.file_type().is_dir() && should_ignore(p, &patterns) {
                iter.skip_current_dir();
                continue;
            }

            if entry.file_type().is_dir()
                && entry
                    .file_name()
                    .to_str()
                    .map(|n| n.eq_ignore_ascii_case("node_modules"))
                    .unwrap_or(false)
            {
                let last_used = metadata_unix_time(p);
                let include = match payload.node_modules_mode {
                    NodeModulesMode::All => true,
                    NodeModulesMode::Stale => last_used == 0 || last_used <= stale_before,
                };

                if include {
                    emit_log(
                        &app,
                        "info",
                        format!("Analyzing node_modules: {}", p.to_string_lossy()),
                    );
                    let Some(size) = dir_size(p) else {
                        cancelled = true;
                        emit_log(&app, "warn", "Scan cancelled while computing directory size");
                        break 'roots;
                    };
                    push_item(
                        &mut items,
                        "node_modules",
                        p.to_string_lossy().to_string(),
                        size,
                        last_used,
                        "Deleting node_modules requires reinstalling dependencies",
                        "filesystem",
                    );
                }
                iter.skip_current_dir();
            }
        }
    }

    if !cancelled {
        for (kind, cache_path, risk) in cache_dirs() {
            if SCAN_CANCELLED.load(Ordering::Relaxed) {
                cancelled = true;
                emit_log(&app, "warn", "Scan cancelled by user");
                break;
            }
            if cache_path.exists() && !should_ignore(&cache_path, &patterns) {
                emit_log(
                    &app,
                    "info",
                    format!("Analyzing cache: {}", cache_path.to_string_lossy()),
                );
                let Some(size) = dir_size(&cache_path) else {
                    cancelled = true;
                    emit_log(&app, "warn", "Scan cancelled while computing cache size");
                    break;
                };
                let last_used = metadata_unix_time(&cache_path);
                push_item(
                    &mut items,
                    &kind,
                    cache_path.to_string_lossy().to_string(),
                    size,
                    last_used,
                    &risk,
                    "package_manager",
                );
            }
        }
    }

    if !cancelled && docker_available() {
        emit_log(&app, "info", "Analyzing docker reclaimable data");
        let docker_df = scan_docker_df_reclaimable();

        if payload.docker_options.dangling_images {
            let size = scan_docker_dangling_bytes();
            push_item(
                &mut items,
                "docker_dangling_images",
                "docker://dangling_images".into(),
                size,
                0,
                "Dangling image cleanup is usually safe",
                "docker",
            );
        }

        if payload.docker_options.unused_images {
            let size = *docker_df.get("images").unwrap_or(&0);
            push_item(
                &mut items,
                "docker_unused_images",
                "docker://unused_images".into(),
                size,
                0,
                "Unused images may be pulled again later",
                "docker",
            );
        }

        if payload.docker_options.stopped_containers {
            let size = *docker_df.get("containers").unwrap_or(&0);
            push_item(
                &mut items,
                "docker_stopped_containers",
                "docker://stopped_containers".into(),
                size,
                0,
                "Stopped containers may include useful debug state",
                "docker",
            );
        }

        if payload.docker_options.build_cache {
            let size = *docker_df.get("build cache").unwrap_or(&0);
            push_item(
                &mut items,
                "docker_build_cache",
                "docker://build_cache".into(),
                size,
                0,
                "Build cache cleanup increases next build duration",
                "docker",
            );
        }

        if payload.docker_options.volumes {
            let size = *docker_df.get("local volumes").unwrap_or(&0);
            push_item(
                &mut items,
                "docker_volumes",
                "docker://volumes".into(),
                size,
                0,
                "Volume cleanup may remove persisted data",
                "docker",
            );
        }
    }

    match payload.sort_by {
        SortBy::Size => items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes)),
        SortBy::LastUsed => items.sort_by(|a, b| a.last_used_unix.cmp(&b.last_used_unix)),
    }

    let total_bytes = items.iter().map(|i| i.size_bytes).sum();
    emit_log(
        &app,
        if cancelled { "warn" } else { "success" },
        format!(
            "Scan finished: {} candidate(s), {}, cancelled={}",
            items.len(),
            total_bytes,
            cancelled
        ),
    );

    Ok(ScanResponse {
        total_bytes,
        item_count: items.len(),
        items,
        cancelled,
    })
}

#[tauri::command]
fn stop_scan(app: AppHandle) -> Result<(), String> {
    SCAN_CANCELLED.store(true, Ordering::SeqCst);
    emit_log(&app, "warn", "Stopping scan...");
    Ok(())
}

fn emit_log(app: &AppHandle, level: &str, message: impl Into<String>) {
    let _ = app.emit(
        "cleanup-log",
        LogPayload {
            level: level.to_string(),
            message: message.into(),
        },
    );
}

fn run_docker_cleanup(kind: &str) -> Result<(), String> {
    let args: &[&str] = match kind {
        "docker_dangling_images" => &["image", "prune", "-f"],
        "docker_unused_images" => &["image", "prune", "-a", "-f"],
        "docker_stopped_containers" => &["container", "prune", "-f"],
        "docker_build_cache" => &["builder", "prune", "-f"],
        "docker_volumes" => &["volume", "prune", "-f"],
        _ => return Err(format!("Unsupported docker cleanup kind: {kind}")),
    };

    let out = Command::new("docker")
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute docker command: {e}"))?;

    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

#[tauri::command]
fn execute_cleanup(payload: CleanupRequest, app: AppHandle) -> Result<CleanupResponse, String> {
    let mut freed_bytes = 0_u64;
    let mut success_count = 0_usize;
    let mut failed_count = 0_usize;

    emit_log(&app, "info", format!("Starting cleanup for {} item(s)", payload.items.len()));

    for item in payload.items {
        emit_log(
            &app,
            "info",
            format!("Cleaning: {} ({})", item.path, item.kind),
        );

        let result = if item.kind.starts_with("docker_") {
            run_docker_cleanup(&item.kind)
        } else {
            let target = PathBuf::from(&item.path);
            if target.as_os_str().is_empty() || !target.exists() {
                Err("Path does not exist".to_string())
            } else {
                fs::remove_dir_all(&target).map_err(|e| e.to_string())
            }
        };

        match result {
            Ok(()) => {
                success_count += 1;
                freed_bytes = freed_bytes.saturating_add(item.size_bytes);
                emit_log(&app, "success", format!("Done: {}", item.path));
            }
            Err(err) => {
                failed_count += 1;
                emit_log(&app, "error", format!("Failed: {} | {}", item.path, err));
            }
        }
    }

    emit_log(&app, "info", "Cleanup finished");

    Ok(CleanupResponse {
        freed_bytes,
        success_count,
        failed_count,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            scan_cleanup_targets,
            stop_scan,
            execute_cleanup
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
