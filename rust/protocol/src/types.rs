//! Primitive type read/write helpers that match .NET BinaryReader/BinaryWriter
//! little-endian encoding.

use bytes::{Buf, BufMut, BytesMut};

use crate::error::ProtocolError;

// ── Read helpers ──────────────────────────────────────────────────────────────

pub fn read_bool(buf: &mut impl Buf) -> Result<bool, ProtocolError> {
    ensure(buf, 1)?;
    Ok(buf.get_u8() != 0)
}

pub fn read_u8(buf: &mut impl Buf) -> Result<u8, ProtocolError> {
    ensure(buf, 1)?;
    Ok(buf.get_u8())
}

pub fn read_i8(buf: &mut impl Buf) -> Result<i8, ProtocolError> {
    ensure(buf, 1)?;
    Ok(buf.get_i8())
}

pub fn read_u16(buf: &mut impl Buf) -> Result<u16, ProtocolError> {
    ensure(buf, 2)?;
    Ok(buf.get_u16_le())
}

pub fn read_i16(buf: &mut impl Buf) -> Result<i16, ProtocolError> {
    ensure(buf, 2)?;
    Ok(buf.get_i16_le())
}

pub fn read_u32(buf: &mut impl Buf) -> Result<u32, ProtocolError> {
    ensure(buf, 4)?;
    Ok(buf.get_u32_le())
}

pub fn read_i32(buf: &mut impl Buf) -> Result<i32, ProtocolError> {
    ensure(buf, 4)?;
    Ok(buf.get_i32_le())
}

pub fn read_u64(buf: &mut impl Buf) -> Result<u64, ProtocolError> {
    ensure(buf, 8)?;
    Ok(buf.get_u64_le())
}

pub fn read_i64(buf: &mut impl Buf) -> Result<i64, ProtocolError> {
    ensure(buf, 8)?;
    Ok(buf.get_i64_le())
}

pub fn read_f32(buf: &mut impl Buf) -> Result<f32, ProtocolError> {
    ensure(buf, 4)?;
    Ok(buf.get_f32_le())
}

pub fn read_f64(buf: &mut impl Buf) -> Result<f64, ProtocolError> {
    ensure(buf, 8)?;
    Ok(buf.get_f64_le())
}

/// Read a .NET length-prefixed UTF-8 string (7-bit encoded length prefix).
pub fn read_string(buf: &mut impl Buf) -> Result<String, ProtocolError> {
    let len = read_7bit_encoded_int(buf)?;
    ensure(buf, len)?;
    let mut bytes = vec![0u8; len];
    buf.copy_to_slice(&mut bytes);
    String::from_utf8(bytes).map_err(|_| ProtocolError::InvalidUtf8)
}

/// Read a byte array: [i32 count][bytes…]
pub fn read_bytes(buf: &mut impl Buf) -> Result<Vec<u8>, ProtocolError> {
    let len = read_i32(buf)? as usize;
    ensure(buf, len)?;
    let mut out = vec![0u8; len];
    buf.copy_to_slice(&mut out);
    Ok(out)
}

/// Point: two i32s (X, Y).
pub fn read_point(buf: &mut impl Buf) -> Result<(i32, i32), ProtocolError> {
    Ok((read_i32(buf)?, read_i32(buf)?))
}

/// Size: two i32s (Width, Height).
pub fn read_size(buf: &mut impl Buf) -> Result<(i32, i32), ProtocolError> {
    Ok((read_i32(buf)?, read_i32(buf)?))
}

/// Color: ARGB as i32.
pub fn read_color(buf: &mut impl Buf) -> Result<i32, ProtocolError> {
    read_i32(buf)
}

/// DateTime: i64 binary representation (100-ns ticks from 0001-01-01).
pub fn read_datetime(buf: &mut impl Buf) -> Result<i64, ProtocolError> {
    read_i64(buf)
}

/// TimeSpan: i64 ticks.
pub fn read_timespan(buf: &mut impl Buf) -> Result<i64, ProtocolError> {
    read_i64(buf)
}

// ── Write helpers ─────────────────────────────────────────────────────────────

pub fn write_bool(buf: &mut BytesMut, v: bool) {
    buf.put_u8(v as u8);
}

pub fn write_u8(buf: &mut BytesMut, v: u8) {
    buf.put_u8(v);
}

pub fn write_i8(buf: &mut BytesMut, v: i8) {
    buf.put_i8(v);
}

pub fn write_u16(buf: &mut BytesMut, v: u16) {
    buf.put_u16_le(v);
}

pub fn write_i16(buf: &mut BytesMut, v: i16) {
    buf.put_i16_le(v);
}

pub fn write_u32(buf: &mut BytesMut, v: u32) {
    buf.put_u32_le(v);
}

pub fn write_i32(buf: &mut BytesMut, v: i32) {
    buf.put_i32_le(v);
}

pub fn write_u64(buf: &mut BytesMut, v: u64) {
    buf.put_u64_le(v);
}

pub fn write_i64(buf: &mut BytesMut, v: i64) {
    buf.put_i64_le(v);
}

pub fn write_f32(buf: &mut BytesMut, v: f32) {
    buf.put_f32_le(v);
}

pub fn write_f64(buf: &mut BytesMut, v: f64) {
    buf.put_f64_le(v);
}

/// Write a .NET length-prefixed UTF-8 string.
pub fn write_string(buf: &mut BytesMut, s: &str) {
    let bytes = s.as_bytes();
    write_7bit_encoded_int(buf, bytes.len());
    buf.put(bytes);
}

/// Byte array: [i32 count][bytes…]
pub fn write_bytes(buf: &mut BytesMut, data: &[u8]) {
    write_i32(buf, data.len() as i32);
    buf.put(data);
}

pub fn write_point(buf: &mut BytesMut, x: i32, y: i32) {
    write_i32(buf, x);
    write_i32(buf, y);
}

pub fn write_size(buf: &mut BytesMut, w: i32, h: i32) {
    write_i32(buf, w);
    write_i32(buf, h);
}

pub fn write_color(buf: &mut BytesMut, argb: i32) {
    write_i32(buf, argb);
}

pub fn write_datetime(buf: &mut BytesMut, binary: i64) {
    write_i64(buf, binary);
}

pub fn write_timespan(buf: &mut BytesMut, ticks: i64) {
    write_i64(buf, ticks);
}

// ── Internal ──────────────────────────────────────────────────────────────────

fn ensure(buf: &impl Buf, n: usize) -> Result<(), ProtocolError> {
    if buf.remaining() < n {
        Err(ProtocolError::BufferTooShort { need: n, have: buf.remaining() })
    } else {
        Ok(())
    }
}

/// .NET's 7-bit encoded integer (used for string length prefix).
fn read_7bit_encoded_int(buf: &mut impl Buf) -> Result<usize, ProtocolError> {
    let mut result: usize = 0;
    let mut shift = 0usize;
    loop {
        ensure(buf, 1)?;
        let byte = buf.get_u8() as usize;
        result |= (byte & 0x7F) << shift;
        shift += 7;
        if byte & 0x80 == 0 || shift >= 35 {
            break;
        }
    }
    Ok(result)
}

fn write_7bit_encoded_int(buf: &mut BytesMut, mut v: usize) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn string_roundtrip_ascii() {
        let mut buf = BytesMut::new();
        write_string(&mut buf, "hello");
        let frozen = buf.freeze();
        let mut cursor = std::io::Cursor::new(frozen.as_ref());
        let s = read_string(&mut cursor).unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn string_roundtrip_empty() {
        let mut buf = BytesMut::new();
        write_string(&mut buf, "");
        let frozen = buf.freeze();
        let mut cursor = std::io::Cursor::new(frozen.as_ref());
        let s = read_string(&mut cursor).unwrap();
        assert_eq!(s, "");
    }

    #[test]
    fn bool_roundtrip() {
        let mut buf = BytesMut::new();
        write_bool(&mut buf, true);
        write_bool(&mut buf, false);
        let mut b = buf.freeze();
        assert!(read_bool(&mut b).unwrap());
        assert!(!read_bool(&mut b).unwrap());
    }
}
