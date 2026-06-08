use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("buffer too short: need {need} bytes, have {have}")]
    BufferTooShort { need: usize, have: usize },

    #[error("unknown packet type id: {0}")]
    UnknownPacketId(u16),

    #[error("string is not valid UTF-8")]
    InvalidUtf8,

    #[error("packet length field overflows available buffer")]
    LengthOverflow,

    #[error("unknown enum discriminant: {0}")]
    UnknownEnumValue(i32),
}
