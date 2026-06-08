//! Wire protocol for Zircon.
//!
//! # Frame format
//!
//! Every packet on the wire is prefixed by a 4-byte little-endian **total
//! length** (including the 4-byte length field itself), followed by a 2-byte
//! little-endian **packet type index**, then the packet fields.
//!
//! ```text
//! ┌───────────────────┬────────────────────┬────────────────────┐
//! │  length (4 bytes) │  type id (2 bytes) │  payload (n bytes) │
//! └───────────────────┴────────────────────┴────────────────────┘
//! ```
//!
//! The type index matches the C# packet list which is sorted as:
//!   1. `Library.Network.GeneralPackets` namespace (alphabetical by name)
//!   2. All other packets (alphabetical by name, ignoring namespace)
//!
//! Field serialisation mirrors `BinaryWriter`/`BinaryReader` little-endian
//! encoding.  Strings are length-prefixed using the .NET 7-bit-encoded-int
//! format.  Lists are prefixed with a 4-byte count.  Optional class
//! references are preceded by a 1-byte null flag (0 = null, 1 = present).

pub mod codec;
pub mod enums;
pub mod error;
pub mod frame;
pub mod packets;
pub mod shared;
pub mod types;
pub mod wire;

pub use codec::{PacketCodec, PacketId};
pub use error::ProtocolError;
pub use frame::RawFrame;
