//! Parallel directory scanner with MD5 hashing.
//!
//! Mirrors C# `CreateVersion()` which uses `Parallel.For` with up to 8 threads
//! to compute MD5 checksums; rayon handles thread pooling automatically.

use std::path::Path;

use rayon::prelude::*;
use walkdir::WalkDir;

/// One scanned file entry.
pub struct FileEntry {
    /// Path relative to the scanned root, using the OS path separator.
    pub rel_path: String,
    pub checksum: [u8; 16],
}

/// Recursively scan `dir`, computing MD5 of each file in parallel.
///
/// Results are sorted by `rel_path` for deterministic `PList.Bin` output.
pub fn scan(dir: &Path) -> anyhow::Result<Vec<FileEntry>> {
    let dir = dir.canonicalize()?;

    let mut paths: Vec<_> = WalkDir::new(&dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();

    // Sort so the output PList.Bin is deterministic.
    paths.sort();

    let entries: Vec<FileEntry> = paths
        .into_par_iter()
        .filter_map(|path| {
            let rel = path.strip_prefix(&dir).ok()?.to_string_lossy().into_owned();
            let bytes = std::fs::read(&path).ok()?;
            let digest = md5::compute(&bytes);
            Some(FileEntry { rel_path: rel, checksum: *digest })
        })
        .collect();

    Ok(entries)
}
