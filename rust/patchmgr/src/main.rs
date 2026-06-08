//! `zircon-patchmgr` CLI entry point.

use std::{collections::HashMap, path::PathBuf};

use clap::{Parser, Subcommand};
use zircon_patchmgr::{
    compress,
    patch_info::{read_plist, write_plist, PatchInfo},
    scanner,
};

#[derive(Parser)]
#[command(
    name = "zircon-patchmgr",
    about = "Zircon client patch manager — diffs, compresses, and indexes client files"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan a clean client directory, diff against an existing PList.Bin,
    /// compress new/changed files, and write a new PList.Bin.
    Build {
        /// Path to the clean client directory to scan.
        client_dir: PathBuf,
        /// Existing PList.Bin to diff against (omit for a full rebuild).
        #[arg(long)]
        existing: Option<PathBuf>,
        /// Output directory for compressed `.gz` patch files.
        #[arg(long, default_value = "Patch")]
        patch_dir: PathBuf,
        /// Output path for the new PList.Bin.
        #[arg(long, default_value = "PList.Bin")]
        out: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Build { client_dir, existing, patch_dir, out } => {
            cmd_build(client_dir, existing, patch_dir, out)
        }
    }
}

/// Derive the `.gz` patch file name from a relative client path.
/// Mirrors C#: `file.FileName.Replace("\\", "-") + ".gz"`
fn make_patch_name(file_name: &str) -> String {
    file_name.replace('\\', "-").replace('/', "-") + ".gz"
}

/// Normalize path separators to `/` for cross-platform consistency.
fn normalize(s: &str) -> String {
    s.replace('\\', "/")
}

fn cmd_build(
    client_dir: PathBuf,
    existing: Option<PathBuf>,
    patch_dir: PathBuf,
    out: PathBuf,
) -> anyhow::Result<()> {
    // ── Step 1: scan ──────────────────────────────────────────────────────
    eprintln!("Scanning {}...", client_dir.display());
    let entries = scanner::scan(&client_dir)?;
    eprintln!("  {} files found", entries.len());

    // ── Step 2: load existing PList.Bin ───────────────────────────────────
    let existing_map: HashMap<String, PatchInfo> = match &existing {
        Some(p) if p.exists() => {
            eprintln!("Loading existing {}...", p.display());
            read_plist(p)?
                .into_iter()
                .map(|pi| (normalize(&pi.file_name), pi))
                .collect()
        }
        Some(p) => {
            eprintln!("Warning: {} not found — performing full build", p.display());
            HashMap::new()
        }
        None => HashMap::new(),
    };

    // ── Step 3: build patch list ──────────────────────────────────────────
    std::fs::create_dir_all(&patch_dir)?;

    let mut result: Vec<PatchInfo> = Vec::with_capacity(entries.len());
    let mut compressed_count = 0usize;
    let mut total_compressed: i64 = 0;

    for entry in &entries {
        let norm_rel = normalize(&entry.rel_path);
        let patch_name = make_patch_name(&norm_rel);
        let src = client_dir.join(&entry.rel_path);
        let dst = patch_dir.join(&patch_name);

        let compressed_len = match existing_map.get(&norm_rel) {
            Some(ex) if ex.checksum == entry.checksum => ex.compressed_len,
            _ => {
                let len = compress::gzip_file(&src, &dst)
                    .map_err(|e| anyhow::anyhow!("compressing {}: {e}", entry.rel_path))?;
                compressed_count += 1;
                total_compressed += len;
                len
            }
        };

        result.push(PatchInfo {
            file_name: norm_rel,
            compressed_len,
            checksum: entry.checksum,
        });
    }

    // ── Step 4: write PList.Bin ───────────────────────────────────────────
    write_plist(&result, &out)?;

    let mb = total_compressed as f64 / 1_000_000.0;
    eprintln!(
        "Done: {} total files, {} compressed ({:.1} MB), PList.Bin → {}",
        result.len(),
        compressed_count,
        mb,
        out.display(),
    );

    Ok(())
}
