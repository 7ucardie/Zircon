use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use zircon_patchmgr::{compress, patch_info, scanner};

// ── Shared types ──────────────────────────────────────────────────────────────

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct PatchConfig {
    pub client_dir: String,
    pub patch_dir:  String,
    pub plist_path: String,
    pub ftp_host:   String,
    pub ftp_user:   String,
    pub use_sftp:   bool,
}

#[derive(Serialize)]
pub struct FileScanResult {
    pub path:     String,
    pub checksum: String,
}

#[derive(Serialize)]
pub struct PatchBuildResult {
    pub total_files: usize,
    pub compressed:  usize,
    pub plist_path:  String,
}

// ── Config persistence ────────────────────────────────────────────────────────

fn config_file(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    Ok(base.join("patch_config.json"))
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Scan a directory and return relative paths + MD5 checksums.
#[tauri::command]
pub fn scan_client_dir(path: String) -> Result<Vec<FileScanResult>, String> {
    scanner::scan(Path::new(&path))
        .map(|entries| {
            entries
                .into_iter()
                .map(|e| FileScanResult {
                    path:     e.rel_path,
                    checksum: e.checksum.iter().map(|b| format!("{b:02x}")).collect(),
                })
                .collect()
        })
        .map_err(|e| e.to_string())
}

/// Scan `client_dir`, GZIP each file into `patch_dir`, and write `PList.Bin`.
#[tauri::command]
pub fn build_patches(
    client_dir: String,
    patch_dir:  String,
    plist_path: String,
) -> Result<PatchBuildResult, String> {
    let src       = Path::new(&client_dir);
    let patch_out = Path::new(&patch_dir);
    let plist_out = Path::new(&plist_path);

    let entries = scanner::scan(src).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(patch_out).map_err(|e| e.to_string())?;

    let mut result     = Vec::with_capacity(entries.len());
    let mut compressed = 0usize;

    for entry in &entries {
        let norm       = entry.rel_path.replace('\\', "/");
        let patch_name = norm.replace('/', "-") + ".gz";
        let src_file   = src.join(&entry.rel_path);
        let dst_file   = patch_out.join(&patch_name);

        let len = compress::gzip_file(&src_file, &dst_file)
            .map_err(|e| format!("{}: {e}", entry.rel_path))?;
        compressed += 1;

        result.push(patch_info::PatchInfo {
            file_name:      norm,
            compressed_len: len,
            checksum:       entry.checksum,
        });
    }

    patch_info::write_plist(&result, plist_out).map_err(|e| e.to_string())?;

    Ok(PatchBuildResult {
        total_files: result.len(),
        compressed,
        plist_path,
    })
}

/// Load saved configuration from the app data directory.
#[tauri::command]
pub fn read_patch_config(app: AppHandle) -> PatchConfig {
    config_file(&app)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Persist configuration to the app data directory.
#[tauri::command]
pub fn save_patch_config(app: AppHandle, config: PatchConfig) -> Result<(), String> {
    let path = config_file(&app)?;
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}
