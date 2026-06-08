//! DBMapping reader — reads the schema header that precedes each collection
//! in a MirDB binary file.

use crate::{binary::BinReader, error::DbError};

/// One property entry from the file's DBMapping header.
#[derive(Debug, Clone)]
pub struct PropDef {
    /// The C# property name (e.g. "ItemName").
    pub name: String,
    /// The full CLR type name stored by DBValue.Save (e.g. "System.Int32").
    pub type_name: String,
}

/// The mapping header for one collection.
#[derive(Debug)]
pub struct DbMapping {
    /// Full CLR type name of the collection's element (e.g.
    /// "Library.SystemModels.ItemInfo").
    pub type_name: String,
    pub properties: Vec<PropDef>,
}

impl DbMapping {
    /// Read a DbMapping from the binary stream (DBMapping(assemblies, reader) in C#).
    pub fn read(r: &mut BinReader<'_>) -> Result<Self, DbError> {
        let type_name = r.read_string()?;
        let count = r.read_i32()? as usize;
        let mut properties = Vec::with_capacity(count);
        for _ in 0..count {
            let name = r.read_string()?;
            let type_name_prop = r.read_string()?;
            properties.push(PropDef { name, type_name: type_name_prop });
        }
        Ok(DbMapping { type_name, properties })
    }
}

/// Skip a single value whose CLR type name is `type_name`.
///
/// This is used to advance past properties that don't have a corresponding
/// Rust field (schema evolution — new properties added in C# not yet in Rust).
pub fn skip_value(type_name: &str, r: &mut BinReader<'_>) -> Result<(), DbError> {
    match type_name {
        "System.Boolean" => { r.read_bool()?; }
        "System.Byte"    => { r.read_u8()?; }
        "System.SByte"   => { r.read_i8()?; }
        "System.Char"    => { r.read_char()?; }
        "System.Int16"   => { r.read_i16()?; }
        "System.UInt16"  => { r.read_u16()?; }
        "System.Int32"   => { r.read_i32()?; }
        "System.UInt32"  => { r.read_u32()?; }
        "System.Int64"   => { r.read_i64()?; }
        "System.UInt64"  => { r.read_u64()?; }
        "System.Single"  => { r.read_f32()?; }
        "System.Double"  => { r.read_f64()?; }
        // 16-byte CLR Decimal
        "System.Decimal" => { r.read_decimal()?; }
        // DateTime.ToBinary() and TimeSpan.Ticks — both i64
        "System.DateTime" | "System.TimeSpan" => { r.read_i64()?; }
        // Color.ToArgb() — i32
        "System.Drawing.Color" => { r.read_i32()?; }
        // Point: (i32, i32), Size: (i32, i32)
        "System.Drawing.Point" | "System.Drawing.Size" => {
            r.read_i32()?;
            r.read_i32()?;
        }
        // String: 7-bit len + bytes
        "System.String" => { r.read_string()?; }
        // Byte[]: i32 len (no null flag) + bytes
        "System.Byte[]" => { r.read_byte_array()?; }
        // Int32[]: bool + (i32 count + i32s)
        "System.Int32[]" => { r.read_i32_array()?; }
        // Point[]: bool + (i32 count + point pairs)
        "System.Drawing.Point[]" => { r.read_point_array()?; }
        // Stats: bool + (i32 count + (i32 key, i32 value) pairs)
        "Library.Stats" => { r.read_stats()?; }
        // BitArray: bool + (i32 byte_count + bytes)
        "System.Collections.BitArray" => { r.read_bit_array()?; }
        other => return Err(DbError::UnknownType(other.to_owned())),
    }
    Ok(())
}
