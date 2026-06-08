//! MirDB session loader — reads System.db / Users.db binary files.
//!
//! # File format (unencrypted)
//!
//! ```text
//! [i32: collection count]
//! [for each collection]
//!   [DbMapping header]
//!   [i32: data blob size]
//!   [data blob bytes]:
//!     [i32: next-object index counter]
//!     [i32: object count]
//!     [for each object]
//!       [i32: RawData size]
//!       [RawData bytes]  — fields in mapping property order
//! ```
//!
//! # Encryption
//!
//! The IsEncrypted check reads bytes 5–20 of the file and compares them
//! against known type-name prefixes ("Library.SystemMo", "Server.DBModels.",
//! "Client.UserModel").  If the file starts with one of those, it is NOT
//! encrypted.  We only support unencrypted files; encrypted files return
//! `DbError::Encrypted`.

use std::path::Path;

use crate::{
    binary::BinReader,
    error::DbError,
    mapping::DbMapping,
};

/// The raw binary content of one object in a collection.
#[derive(Debug)]
pub struct RawObject {
    pub raw_data: Vec<u8>,
}

/// One deserialized collection from the binary file.
#[derive(Debug)]
pub struct RawCollection {
    pub mapping: DbMapping,
    /// The next-ID counter (high-water mark for auto-increment).
    pub next_index: i32,
    pub objects: Vec<RawObject>,
}

/// Load all collections from a MirDB binary file (System.db or Users.db).
///
/// Returns an error if the file is AES-encrypted (key not provided).
pub fn load_file(path: &Path) -> Result<Vec<RawCollection>, DbError> {
    let bytes = std::fs::read(path)?;
    parse_bytes(&bytes)
}

fn is_encrypted(bytes: &[u8]) -> bool {
    if bytes.len() < 21 {
        return false;
    }
    let slice = &bytes[5..21];
    if slice.starts_with(b"Plugin.") {
        return false;
    }
    let known = [b"Server.DBModels." as &[u8], b"Library.SystemMo", b"Client.UserModel"];
    !known.iter().any(|k| slice == *k)
}

fn parse_bytes(bytes: &[u8]) -> Result<Vec<RawCollection>, DbError> {
    if is_encrypted(bytes) {
        return Err(DbError::Encrypted);
    }

    let mut r = BinReader::new(bytes);
    let count = r.read_i32()? as usize;
    let mut collections = Vec::with_capacity(count);

    for _ in 0..count {
        let mapping = DbMapping::read(&mut r)?;
        let data_len = r.read_i32()? as usize;
        let data = r.read_bytes(data_len)?;
        let col = parse_collection(mapping, &data)?;
        collections.push(col);
    }

    Ok(collections)
}

fn parse_collection(mapping: DbMapping, data: &[u8]) -> Result<RawCollection, DbError> {
    let mut r = BinReader::new(data);
    let next_index = r.read_i32()?;
    let count = r.read_i32()? as usize;
    let mut objects = Vec::with_capacity(count);

    for _ in 0..count {
        let size = r.read_i32()? as usize;
        let raw_data = r.read_bytes(size)?;
        objects.push(RawObject { raw_data });
    }

    Ok(RawCollection { mapping, next_index, objects })
}
