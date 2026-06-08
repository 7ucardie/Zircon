use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error("buffer underflow: need {need} bytes, have {have}")]
    BufferUnderflow { need: usize, have: usize },

    #[error("invalid UTF-8 in string field")]
    InvalidUtf8,

    #[error("invalid UTF-16 char: {0:#06x}")]
    InvalidUtf16(u16),

    #[error("encrypted database files are not supported in this build")]
    Encrypted,

    #[error("unknown type in mapping: {0}")]
    UnknownType(String),

    #[error("unexpected 7-bit int encoding (too many continuation bytes)")]
    BadVarInt,
}
