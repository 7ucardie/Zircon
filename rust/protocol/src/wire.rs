//! WireRead / WireWrite traits + implementations for every type that appears in
//! packet fields, matching .NET BinaryReader/BinaryWriter little-endian encoding.
//!
//! # Null flags
//! The C# reflection engine writes a `bool` null flag before every class-type
//! property. Rust represents nullable classes as `Option<T>`:
//! - `Option<T>` WireRead: reads 1 bool; if false returns None, else reads T.
//! - `Option<T>` WireWrite: writes true + T, or false.
//!
//! # List encoding
//! Two helpers mirror the two paths in Packet.cs:
//! - `read_prim_list` / `write_prim_list`: for `List<T>` where T is a primitive
//!   or enum (types present in TypeRead/TypeWrite) — no per-item null flag.
//! - `read_class_list` / `write_class_list`: for `List<T>` where T is a class
//!   — each item is preceded by a bool null flag; null items are skipped.

use std::collections::BTreeMap;

use bytes::{Buf, BufMut, BytesMut};

use crate::error::ProtocolError;

// ── Core trait definitions ─────────────────────────────────────────────────

pub trait WireRead: Sized {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError>;
}

pub trait WireWrite {
    fn wire_write(&self, buf: &mut BytesMut);
}

// ── Primitive impls ────────────────────────────────────────────────────────

macro_rules! impl_prim {
    ($t:ty, $read:ident, $write_le:ident, $put_le:ident, $size:expr) => {
        impl WireRead for $t {
            fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
                if buf.remaining() < $size {
                    return Err(ProtocolError::BufferTooShort {
                        need: $size,
                        have: buf.remaining(),
                    });
                }
                Ok(buf.$read())
            }
        }
        impl WireWrite for $t {
            fn wire_write(&self, buf: &mut BytesMut) {
                buf.$put_le(*self);
            }
        }
    };
}

impl WireRead for bool {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        if buf.remaining() < 1 {
            return Err(ProtocolError::BufferTooShort { need: 1, have: 0 });
        }
        Ok(buf.get_u8() != 0)
    }
}
impl WireWrite for bool {
    fn wire_write(&self, buf: &mut BytesMut) {
        buf.put_u8(*self as u8);
    }
}

impl WireRead for u8 {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        if buf.remaining() < 1 {
            return Err(ProtocolError::BufferTooShort { need: 1, have: 0 });
        }
        Ok(buf.get_u8())
    }
}
impl WireWrite for u8 {
    fn wire_write(&self, buf: &mut BytesMut) {
        buf.put_u8(*self);
    }
}

impl WireRead for i8 {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        if buf.remaining() < 1 {
            return Err(ProtocolError::BufferTooShort { need: 1, have: 0 });
        }
        Ok(buf.get_i8())
    }
}
impl WireWrite for i8 {
    fn wire_write(&self, buf: &mut BytesMut) {
        buf.put_i8(*self);
    }
}

impl_prim!(u16, get_u16_le, write_u16_le, put_u16_le, 2);
impl_prim!(i16, get_i16_le, write_i16_le, put_i16_le, 2);
impl_prim!(u32, get_u32_le, write_u32_le, put_u32_le, 4);
impl_prim!(i32, get_i32_le, write_i32_le, put_i32_le, 4);
impl_prim!(u64, get_u64_le, write_u64_le, put_u64_le, 8);
impl_prim!(i64, get_i64_le, write_i64_le, put_i64_le, 8);
impl_prim!(f32, get_f32_le, write_f32_le, put_f32_le, 4);
impl_prim!(f64, get_f64_le, write_f64_le, put_f64_le, 8);

// ── String (7-bit encoded length prefix + UTF-8) ──────────────────────────

impl WireRead for String {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        let len = read_7bit_int(buf)?;
        if buf.remaining() < len {
            return Err(ProtocolError::BufferTooShort { need: len, have: buf.remaining() });
        }
        let mut bytes = vec![0u8; len];
        buf.copy_to_slice(&mut bytes);
        String::from_utf8(bytes).map_err(|_| ProtocolError::InvalidUtf8)
    }
}
impl WireWrite for String {
    fn wire_write(&self, buf: &mut BytesMut) {
        write_7bit_int(buf, self.len());
        buf.put(self.as_bytes());
    }
}

// ── Byte array: [i32 length][bytes] ───────────────────────────────────────

impl WireRead for Vec<u8> {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        let len = i32::wire_read(buf)? as usize;
        if buf.remaining() < len {
            return Err(ProtocolError::BufferTooShort { need: len, have: buf.remaining() });
        }
        let mut out = vec![0u8; len];
        buf.copy_to_slice(&mut out);
        Ok(out)
    }
}
impl WireWrite for Vec<u8> {
    fn wire_write(&self, buf: &mut BytesMut) {
        i32::wire_write(&(self.len() as i32), buf);
        buf.put(self.as_slice());
    }
}

// ── Option<T> — null flag + T ─────────────────────────────────────────────

impl<T: WireRead> WireRead for Option<T> {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        if bool::wire_read(buf)? {
            Ok(Some(T::wire_read(buf)?))
        } else {
            Ok(None)
        }
    }
}
impl<T: WireWrite> WireWrite for Option<T> {
    fn wire_write(&self, buf: &mut BytesMut) {
        match self {
            None => false.wire_write(buf),
            Some(v) => {
                true.wire_write(buf);
                v.wire_write(buf);
            }
        }
    }
}

// ── .NET Decimal: 4 consecutive i32s from decimal.GetBits() ──────────────

/// A 128-bit .NET decimal represented as its four raw i32 components.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NetDecimal(pub [i32; 4]);

impl WireRead for NetDecimal {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(NetDecimal([
            i32::wire_read(buf)?,
            i32::wire_read(buf)?,
            i32::wire_read(buf)?,
            i32::wire_read(buf)?,
        ]))
    }
}
impl WireWrite for NetDecimal {
    fn wire_write(&self, buf: &mut BytesMut) {
        for v in &self.0 {
            v.wire_write(buf);
        }
    }
}

// ── Point (i32 X, i32 Y) and Size (i32 Width, i32 Height) ─────────────────

/// System.Drawing.Point — two i32s (X, Y).
pub type Point = (i32, i32);

impl WireRead for (i32, i32) {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok((i32::wire_read(buf)?, i32::wire_read(buf)?))
    }
}
impl WireWrite for (i32, i32) {
    fn wire_write(&self, buf: &mut BytesMut) {
        self.0.wire_write(buf);
        self.1.wire_write(buf);
    }
}

// ── Stats — wraps SortedDictionary<Stat,int> ─────────────────────────────
//
// Wire format (as a property in another object):
//   [bool null-flag-for-Stats]      ← handled by Option<Stats>
//     [bool null-flag-for-Values]   ← inside Stats::WireRead
//     [i32 count]
//     [i32 key][i32 value] × count  ← sorted by key (BTreeMap)

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats(pub BTreeMap<i32, i32>);

impl WireRead for Stats {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        // Read null flag for the inner SortedDictionary<Stat,int> Values
        if !bool::wire_read(buf)? {
            return Ok(Stats(BTreeMap::new()));
        }
        let count = i32::wire_read(buf)? as usize;
        let mut map = BTreeMap::new();
        for _ in 0..count {
            let k = i32::wire_read(buf)?;
            let v = i32::wire_read(buf)?;
            map.insert(k, v);
        }
        Ok(Stats(map))
    }
}
impl WireWrite for Stats {
    fn wire_write(&self, buf: &mut BytesMut) {
        // Write null flag for Values (always present)
        true.wire_write(buf);
        i32::wire_write(&(self.0.len() as i32), buf);
        for (&k, &v) in &self.0 {
            k.wire_write(buf);
            v.wire_write(buf);
        }
    }
}

// ── IntDict — Dictionary<SomeEnum, int> ───────────────────────────────────
//
// Both key (enum underlying = i32) and value (int) use the primitive path.
// Wire format: [i32 count][(i32 key)(i32 value)...]

#[derive(Debug, Clone, Default, PartialEq)]
pub struct IntDict(pub Vec<(i32, i32)>);

impl WireRead for IntDict {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        let count = i32::wire_read(buf)? as usize;
        let mut v = Vec::with_capacity(count);
        for _ in 0..count {
            v.push((i32::wire_read(buf)?, i32::wire_read(buf)?));
        }
        Ok(IntDict(v))
    }
}
impl WireWrite for IntDict {
    fn wire_write(&self, buf: &mut BytesMut) {
        i32::wire_write(&(self.0.len() as i32), buf);
        for (k, v) in &self.0 {
            k.wire_write(buf);
            v.wire_write(buf);
        }
    }
}

// ── ClassList / PrimList — nullable list wrappers ────────────────────────
//
// C# serializes every List<T> property with an outer bool null-flag:
//   [bool: not_null] [i32: count] [items...]
//
// ClassList<T> handles List<T> where T is a class (per-item bool flag).
// PrimList<T>  handles List<T> where T is a primitive/enum (no per-item flag).
//
// Both types include the outer null flag in their WireRead/WireWrite, so they
// can be used directly as field types in packet structs (not wrapped in Option).

#[derive(Debug, Clone, Default)]
pub struct ClassList<T>(pub Vec<T>);

impl<T: WireRead> WireRead for ClassList<T> {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        if !bool::wire_read(buf)? {
            return Ok(ClassList(Vec::new()));
        }
        read_class_list(buf).map(ClassList)
    }
}
impl<T: WireWrite> WireWrite for ClassList<T> {
    fn wire_write(&self, buf: &mut BytesMut) {
        true.wire_write(buf);
        write_class_list(&self.0, buf);
    }
}

#[derive(Debug, Clone, Default)]
pub struct PrimList<T>(pub Vec<T>);

impl<T: WireRead> WireRead for PrimList<T> {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        if !bool::wire_read(buf)? {
            return Ok(PrimList(Vec::new()));
        }
        read_prim_list(buf).map(PrimList)
    }
}
impl<T: WireWrite> WireWrite for PrimList<T> {
    fn wire_write(&self, buf: &mut BytesMut) {
        true.wire_write(buf);
        write_prim_list(&self.0, buf);
    }
}

// ── List helpers ──────────────────────────────────────────────────────────

/// Read `List<T>` where T is a primitive/enum (no per-item null flag).
pub fn read_prim_list<T: WireRead>(buf: &mut impl Buf) -> Result<Vec<T>, ProtocolError> {
    let count = i32::wire_read(buf)? as usize;
    (0..count).map(|_| T::wire_read(buf)).collect()
}

/// Write `List<T>` where T is a primitive/enum (no per-item null flag).
pub fn write_prim_list<T: WireWrite>(items: &[T], buf: &mut BytesMut) {
    i32::wire_write(&(items.len() as i32), buf);
    for item in items {
        item.wire_write(buf);
    }
}

/// Read `List<T>` where T is a class (each item preceded by bool null flag).
/// Null items are simply omitted from the returned Vec.
pub fn read_class_list<T: WireRead>(buf: &mut impl Buf) -> Result<Vec<T>, ProtocolError> {
    let count = i32::wire_read(buf)? as usize;
    let mut v = Vec::with_capacity(count);
    for _ in 0..count {
        if bool::wire_read(buf)? {
            v.push(T::wire_read(buf)?);
        }
    }
    Ok(v)
}

/// Write `List<T>` where T is a class (each item preceded by bool null flag).
pub fn write_class_list<T: WireWrite>(items: &[T], buf: &mut BytesMut) {
    i32::wire_write(&(items.len() as i32), buf);
    for item in items {
        true.wire_write(buf);
        item.wire_write(buf);
    }
}

// ── Internal helpers ───────────────────────────────────────────────────────

fn read_7bit_int(buf: &mut impl Buf) -> Result<usize, ProtocolError> {
    let mut result: usize = 0;
    let mut shift = 0usize;
    loop {
        if buf.remaining() < 1 {
            return Err(ProtocolError::BufferTooShort { need: 1, have: 0 });
        }
        let byte = buf.get_u8() as usize;
        result |= (byte & 0x7F) << shift;
        shift += 7;
        if byte & 0x80 == 0 || shift >= 35 {
            break;
        }
    }
    Ok(result)
}

fn write_7bit_int(buf: &mut BytesMut, mut v: usize) {
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        buf.put_u8(byte);
        if v == 0 {
            break;
        }
    }
}

// ── BytesMut → Buf bridging (for write_le on primitive writes) ────────────
//
// The macro above relies on BytesMut methods; we need to supply them.
// BytesMut implements BufMut but not Buf.  The get_* methods are only on Buf.
// For the write macros we emit buf.put_*_le which IS on BufMut.
// The read macros operate on impl Buf, so that's fine.

#[allow(dead_code)]
trait BufMutExt: BufMut {
    fn write_u16_le(&mut self, v: u16) { self.put_u16_le(v); }
    fn write_i16_le(&mut self, v: i16) { self.put_i16_le(v); }
    fn write_u32_le(&mut self, v: u32) { self.put_u32_le(v); }
    fn write_i32_le(&mut self, v: i32) { self.put_i32_le(v); }
    fn write_u64_le(&mut self, v: u64) { self.put_u64_le(v); }
    fn write_i64_le(&mut self, v: i64) { self.put_i64_le(v); }
    fn write_f32_le(&mut self, v: f32) { self.put_f32_le(v); }
    fn write_f64_le(&mut self, v: f64) { self.put_f64_le(v); }
}
impl<B: BufMut> BufMutExt for B {}
