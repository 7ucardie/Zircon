//! Packet ID registry.
//!
//! The C# server sorts packet types to assign stable integer IDs.
//! We reproduce the same ordering here as a `const` array so IDs match
//! without reflection.
//!
//! Ordering rules (from Packet.cs):
//!   1. `Library.Network.GeneralPackets` namespace sorts to position 0 (all
//!      names within it sort alphabetically).
//!   2. All remaining packets sort alphabetically by name only (ignoring
//!      namespace).

/// Stable numeric identifier for a packet type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PacketId(pub u16);

/// All known packet names in their canonical sorted order.
///
/// The index into this array is the wire `PacketId`.
///
/// GeneralPackets (sorted by name) come first, then ClientPackets and
/// ServerPackets interleaved alphabetically by name.
pub static PACKET_NAMES: &[&str] = &[
    // ── GeneralPackets (namespace prefix = sorted first) ─────────────────
    "CheckVersion",
    "Connected",
    "Disconnect",
    "GoodVersion",
    "Ping",
    "PingResponse",
    "Version",
    // ── ClientPackets + ServerPackets, merged by name (alphabetical) ──────
    // Both namespaces share many names; they are distinct types so each
    // name appears twice (client variant then server variant would collide).
    // The C# sort is: same namespace = name order; cross-namespace = name
    // order (GeneralPackets excluded).  Within the same name the namespace
    // tiebreaker is alphabetical: ClientPackets < ServerPackets.
    "Activation",           // ClientPackets::Activation
    "Activation",           // ServerPackets::Activation  (same sort name)
    "ChangePassword",       // Client
    "ChangePassword",       // Server
    "DeleteCharacter",      // Client
    "DeleteCharacter",      // Server
    "Login",                // Client
    "Login",                // Server
    "Logout",               // Client  (no server twin)
    "NewAccount",           // Client
    "NewAccount",           // Server
    "NewCharacter",         // Client
    "NewCharacter",         // Server
    "RequestActivationKey", // Client
    "RequestActivationKey", // Server
    "RequestPasswordReset", // Client
    "RequestPasswordReset", // Server
    "ResetPassword",        // Client
    "ResetPassword",        // Server
    "SelectLanguage",       // Client  (no server twin)
    "StartGame",            // Server
    // … additional packets will be appended here as they are ported
];

/// Look up the `PacketId` for a given packet name.
///
/// Returns `None` if the name is not registered.
pub fn id_by_name(name: &str) -> Option<PacketId> {
    PACKET_NAMES
        .iter()
        .position(|&n| n == name)
        .map(|i| PacketId(i as u16))
}

/// A trait implemented by every packet type.
pub trait PacketCodec: Sized {
    /// The stable wire ID for this packet type.
    fn packet_id() -> PacketId;

    /// Decode the packet payload (after the 6-byte frame header has been
    /// stripped by `RawFrame::parse`).
    fn decode(payload: bytes::Bytes) -> Result<Self, crate::ProtocolError>;

    /// Encode the packet fields into a complete framed byte sequence
    /// (including the 4-byte length prefix and 2-byte packet ID).
    fn encode(&self) -> bytes::Bytes;
}
