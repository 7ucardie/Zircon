use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::ProtocolError;

/// A single raw (not yet decoded) packet frame read off the wire.
#[derive(Debug, Clone)]
pub struct RawFrame {
    pub packet_id: u16,
    pub payload: Bytes,
}

impl RawFrame {
    /// Attempt to parse the next frame from `buf`.
    ///
    /// Returns `Ok(Some(frame))` when a complete frame is available.
    /// Returns `Ok(None)` when the buffer does not yet contain a full frame.
    /// Returns `Err(_)` on a protocol violation.
    pub fn parse(buf: &mut BytesMut) -> Result<Option<Self>, ProtocolError> {
        if buf.len() < 4 {
            return Ok(None);
        }

        // Peek length without consuming (little-endian u32).
        let total_len = {
            let b = buf.as_ref();
            u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize
        };

        if total_len < 6 {
            // Minimum frame = 4 (length field) + 2 (type id)
            return Err(ProtocolError::LengthOverflow);
        }

        if buf.len() < total_len {
            return Ok(None);
        }

        buf.advance(4); // consume length prefix
        let packet_id = buf.get_u16_le();
        let payload_len = total_len - 6; // subtract length(4) + id(2)
        let payload = buf.split_to(payload_len).freeze();

        Ok(Some(RawFrame { packet_id, payload }))
    }

    /// Encode a packet id + payload into a framed `Bytes`.
    pub fn encode(packet_id: u16, payload: &[u8]) -> Bytes {
        let total_len = (4 + 2 + payload.len()) as u32;
        let mut out = BytesMut::with_capacity(total_len as usize);
        out.put_u32_le(total_len);
        out.put_u16_le(packet_id);
        out.put(payload);
        out.freeze()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_empty_payload() {
        let encoded = RawFrame::encode(7, &[]);
        let mut buf = BytesMut::from(encoded.as_ref());
        let frame = RawFrame::parse(&mut buf).unwrap().unwrap();
        assert_eq!(frame.packet_id, 7);
        assert!(frame.payload.is_empty());
    }

    #[test]
    fn roundtrip_with_payload() {
        let payload = b"hello";
        let encoded = RawFrame::encode(42, payload);
        let mut buf = BytesMut::from(encoded.as_ref());
        let frame = RawFrame::parse(&mut buf).unwrap().unwrap();
        assert_eq!(frame.packet_id, 42);
        assert_eq!(&frame.payload[..], payload);
    }

    #[test]
    fn partial_frame_returns_none() {
        let encoded = RawFrame::encode(1, &[0xAA, 0xBB]);
        let partial = &encoded[..encoded.len() - 1];
        let mut buf = BytesMut::from(partial);
        assert!(RawFrame::parse(&mut buf).unwrap().is_none());
    }
}
