//! Database access layer for Zircon.
//!
//! Uses `sqlx` with SQLite (same database format as the C# server's MirDB
//! layer).  Async runtime is Tokio.

pub mod error;
pub mod pool;

pub use error::DbError;
pub use pool::DbPool;
