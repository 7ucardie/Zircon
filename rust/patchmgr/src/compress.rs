//! GZIP file compression helper.
//!
//! Uses `flate2::Compression::default()` (level 6) matching C#
//! `GZipStream(stream, CompressionMode.Compress)` default.

use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::Path,
};

use flate2::{write::GzEncoder, Compression};

/// Compress `src` with GZIP and write to `dst`.
/// Returns the size of the compressed output in bytes.
pub fn gzip_file(src: &Path, dst: &Path) -> io::Result<i64> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let input = std::fs::read(src)?;
    let out = BufWriter::new(File::create(dst)?);
    let mut encoder = GzEncoder::new(out, Compression::default());
    encoder.write_all(&input)?;
    encoder.finish()?;
    Ok(dst.metadata()?.len() as i64)
}
