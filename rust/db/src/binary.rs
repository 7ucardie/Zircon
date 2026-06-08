//! .NET BinaryReader-compatible byte-slice reader.
//!
//! All integers are little-endian, matching .NET BinaryReader defaults.
//! Strings use the 7-bit-encoded-length prefix followed by UTF-8 bytes.

use crate::error::DbError;

pub struct BinReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BinReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    fn need(&self, n: usize) -> Result<(), DbError> {
        if self.remaining() < n {
            Err(DbError::BufferUnderflow { need: n, have: self.remaining() })
        } else {
            Ok(())
        }
    }

    pub fn read_bool(&mut self) -> Result<bool, DbError> {
        self.need(1)?;
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v != 0)
    }

    pub fn read_u8(&mut self) -> Result<u8, DbError> {
        self.need(1)?;
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    pub fn read_i8(&mut self) -> Result<i8, DbError> {
        Ok(self.read_u8()? as i8)
    }

    pub fn read_i16(&mut self) -> Result<i16, DbError> {
        self.need(2)?;
        let v = i16::from_le_bytes(self.data[self.pos..self.pos + 2].try_into().unwrap());
        self.pos += 2;
        Ok(v)
    }

    pub fn read_u16(&mut self) -> Result<u16, DbError> {
        self.need(2)?;
        let v = u16::from_le_bytes(self.data[self.pos..self.pos + 2].try_into().unwrap());
        self.pos += 2;
        Ok(v)
    }

    pub fn read_i32(&mut self) -> Result<i32, DbError> {
        self.need(4)?;
        let v = i32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }

    pub fn read_u32(&mut self) -> Result<u32, DbError> {
        self.need(4)?;
        let v = u32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }

    pub fn read_i64(&mut self) -> Result<i64, DbError> {
        self.need(8)?;
        let v = i64::from_le_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        Ok(v)
    }

    pub fn read_u64(&mut self) -> Result<u64, DbError> {
        self.need(8)?;
        let v = u64::from_le_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        Ok(v)
    }

    pub fn read_f32(&mut self) -> Result<f32, DbError> {
        self.need(4)?;
        let v = f32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        Ok(v)
    }

    pub fn read_f64(&mut self) -> Result<f64, DbError> {
        self.need(8)?;
        let v = f64::from_le_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        Ok(v)
    }

    /// Read a 2-byte UTF-16-LE char (matches BinaryReader.ReadChar).
    pub fn read_char(&mut self) -> Result<char, DbError> {
        let code_unit = self.read_u16()?;
        char::from_u32(code_unit as u32).ok_or(DbError::InvalidUtf16(code_unit))
    }

    /// Read a .NET Decimal (16 bytes = 4 x i32 from Decimal.GetBits()).
    pub fn read_decimal(&mut self) -> Result<[i32; 4], DbError> {
        Ok([self.read_i32()?, self.read_i32()?, self.read_i32()?, self.read_i32()?])
    }

    /// Read a .NET BinaryReader string: 7-bit-encoded length + UTF-8 bytes.
    pub fn read_string(&mut self) -> Result<String, DbError> {
        let len = self.read_7bit_int()?;
        if len == 0 {
            return Ok(String::new());
        }
        self.need(len)?;
        let bytes = &self.data[self.pos..self.pos + len];
        let s = std::str::from_utf8(bytes).map_err(|_| DbError::InvalidUtf8)?.to_owned();
        self.pos += len;
        Ok(s)
    }

    /// Read exactly `n` raw bytes.
    pub fn read_bytes(&mut self, n: usize) -> Result<Vec<u8>, DbError> {
        self.need(n)?;
        let v = self.data[self.pos..self.pos + n].to_vec();
        self.pos += n;
        Ok(v)
    }

    /// Read a Byte[] field: i32 length (no null flag) + raw bytes.
    pub fn read_byte_array(&mut self) -> Result<Vec<u8>, DbError> {
        let len = self.read_i32()? as usize;
        self.read_bytes(len)
    }

    /// Read an optional Int32[] field: bool flag + (i32 count + i32s).
    pub fn read_i32_array(&mut self) -> Result<Option<Vec<i32>>, DbError> {
        if !self.read_bool()? {
            return Ok(None);
        }
        let count = self.read_i32()? as usize;
        let mut v = Vec::with_capacity(count);
        for _ in 0..count {
            v.push(self.read_i32()?);
        }
        Ok(Some(v))
    }

    /// Read an optional Point[] field: bool flag + (i32 count + (i32,i32) pairs).
    pub fn read_point_array(&mut self) -> Result<Option<Vec<(i32, i32)>>, DbError> {
        if !self.read_bool()? {
            return Ok(None);
        }
        let count = self.read_i32()? as usize;
        let mut v = Vec::with_capacity(count);
        for _ in 0..count {
            v.push((self.read_i32()?, self.read_i32()?));
        }
        Ok(Some(v))
    }

    /// Read an optional BitArray: bool flag + (i32 byte_count + bytes).
    pub fn read_bit_array(&mut self) -> Result<Option<Vec<u8>>, DbError> {
        if !self.read_bool()? {
            return Ok(None);
        }
        let byte_count = self.read_i32()? as usize;
        Ok(Some(self.read_bytes(byte_count)?))
    }

    /// Read a MirDB Stats field: bool flag + (i32 count + (i32 key, i32 value) pairs).
    /// This matches Stats(BinaryReader) wrapped in DBValue's null flag.
    pub fn read_stats(&mut self) -> Result<Option<Vec<(i32, i32)>>, DbError> {
        if !self.read_bool()? {
            return Ok(None);
        }
        let count = self.read_i32()? as usize;
        let mut v = Vec::with_capacity(count);
        for _ in 0..count {
            v.push((self.read_i32()?, self.read_i32()?));
        }
        Ok(Some(v))
    }

    fn read_7bit_int(&mut self) -> Result<usize, DbError> {
        let mut result: usize = 0;
        let mut shift = 0usize;
        loop {
            if shift > 35 {
                return Err(DbError::BadVarInt);
            }
            let byte = self.read_u8()? as usize;
            result |= (byte & 0x7F) << shift;
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
        }
        Ok(result)
    }
}
