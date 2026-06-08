//! `PatchInformation` struct and `PList.Bin` binary (de)serialization.
//!
//! Wire format (mirrors C# `BinaryWriter`/`BinaryReader`):
//!   FileName        — 7-bit variable-length byte count + UTF-8 bytes
//!   CompressedLength — i64 little-endian
//!   checksum length  — i32 little-endian (always 16 for MD5)
//!   checksum bytes   — raw bytes
//!
//! `PList.Bin` is a stream of these records with no leading count; the reader
//! consumes until EOF (matching `GetPatchInformation` in `PMain.cs`).

use std::path::Path;

/// One entry in PList.Bin.
#[derive(Debug, Clone)]
pub struct PatchInfo {
    /// Relative path from the client root (uses `/` separators in Rust output).
    pub file_name: String,
    /// Compressed size in bytes of the corresponding `.gz` patch file.
    pub compressed_len: i64,
    /// MD5 hash of the original (uncompressed) file.
    pub checksum: [u8; 16],
}

// ── Public helpers ────────────────────────────────────────────────────────────

/// Read all entries from a PList.Bin file (no leading count — reads until EOF).
pub fn read_plist(path: &Path) -> anyhow::Result<Vec<PatchInfo>> {
    let data = std::fs::read(path)?;
    let mut pos = 0usize;
    let mut list = Vec::new();
    while pos < data.len() {
        list.push(read_entry(&data, &mut pos)?);
    }
    Ok(list)
}

/// Write all entries to a PList.Bin file (no leading count).
pub fn write_plist(entries: &[PatchInfo], path: &Path) -> anyhow::Result<()> {
    let mut buf = Vec::with_capacity(entries.len() * 64);
    for e in entries {
        write_entry(e, &mut buf);
    }
    std::fs::write(path, &buf)?;
    Ok(())
}

// ── Entry (de)serialization ───────────────────────────────────────────────────

fn read_entry(data: &[u8], pos: &mut usize) -> anyhow::Result<PatchInfo> {
    let file_name = read_bw_string(data, pos)?;

    anyhow::ensure!(*pos + 8 <= data.len(), "unexpected EOF reading CompressedLength");
    let compressed_len = i64::from_le_bytes(data[*pos..*pos + 8].try_into().unwrap());
    *pos += 8;

    anyhow::ensure!(*pos + 4 <= data.len(), "unexpected EOF reading checksum length");
    let cs_len = i32::from_le_bytes(data[*pos..*pos + 4].try_into().unwrap()) as usize;
    *pos += 4;

    anyhow::ensure!(*pos + cs_len <= data.len(), "unexpected EOF reading checksum bytes");
    let mut checksum = [0u8; 16];
    checksum[..cs_len.min(16)].copy_from_slice(&data[*pos..*pos + cs_len.min(16)]);
    *pos += cs_len;

    Ok(PatchInfo { file_name, compressed_len, checksum })
}

fn write_entry(e: &PatchInfo, buf: &mut Vec<u8>) {
    write_bw_string(buf, &e.file_name);
    buf.extend_from_slice(&e.compressed_len.to_le_bytes());
    buf.extend_from_slice(&(e.checksum.len() as i32).to_le_bytes());
    buf.extend_from_slice(&e.checksum);
}

// ── 7-bit BinaryWriter string encoding ───────────────────────────────────────

fn write_bw_string(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    let mut len = bytes.len();
    loop {
        let low7 = (len & 0x7F) as u8;
        len >>= 7;
        buf.push(if len > 0 { low7 | 0x80 } else { low7 });
        if len == 0 {
            break;
        }
    }
    buf.extend_from_slice(bytes);
}

fn read_bw_string(data: &[u8], pos: &mut usize) -> anyhow::Result<String> {
    let mut value = 0usize;
    let mut shift = 0u32;
    loop {
        anyhow::ensure!(*pos < data.len(), "unexpected EOF reading BinaryWriter string length");
        let b = data[*pos];
        *pos += 1;
        value |= ((b & 0x7F) as usize) << shift;
        shift += 7;
        if (b & 0x80) == 0 {
            break;
        }
        anyhow::ensure!(shift < 35, "malformed 7-bit integer");
    }
    anyhow::ensure!(*pos + value <= data.len(), "unexpected EOF reading string bytes");
    let s = std::str::from_utf8(&data[*pos..*pos + value])?.to_owned();
    *pos += value;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_entry() {
        let original = PatchInfo {
            file_name: "Data/Maps/Map001.bin".into(),
            compressed_len: 123456,
            checksum: [
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
            ],
        };
        let mut buf = Vec::new();
        write_entry(&original, &mut buf);
        let mut pos = 0;
        let decoded = read_entry(&buf, &mut pos).unwrap();
        assert_eq!(pos, buf.len());
        assert_eq!(decoded.file_name, original.file_name);
        assert_eq!(decoded.compressed_len, original.compressed_len);
        assert_eq!(decoded.checksum, original.checksum);
    }

    #[test]
    fn bw_string_empty() {
        let mut buf = Vec::new();
        write_bw_string(&mut buf, "");
        assert_eq!(buf, &[0x00]);
        let mut pos = 0;
        assert_eq!(read_bw_string(&buf, &mut pos).unwrap(), "");
        assert_eq!(pos, 1);
    }

    #[test]
    fn bw_string_long() {
        // 128-char string requires 2-byte length prefix
        let s: String = "x".repeat(128);
        let mut buf = Vec::new();
        write_bw_string(&mut buf, &s);
        assert_eq!(buf[0], 0x80); // lower 7 bits = 0, more bytes follow
        assert_eq!(buf[1], 0x01); // upper bits = 1 → total 128
        let mut pos = 0;
        assert_eq!(read_bw_string(&buf, &mut pos).unwrap(), s);
    }
}
