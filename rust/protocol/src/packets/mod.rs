//! Packet type modules — GeneralPackets, ClientPackets, ServerPackets.
//!
//! All types implement the `PacketCodec` trait from `crate::codec`.

pub mod client;
pub mod general;
pub mod server;

// Re-export namespaced sub-modules so callers can write
//   use zircon_protocol::packets::general::*;
//   use zircon_protocol::packets::client::*;
//   use zircon_protocol::packets::server::*;

// ── Internal macro used in sibling modules ────────────────────────────────
//
// Define a packet struct + PacketCodec impl in one go.
//
// Usage:
//   packet!(id = N; StructName)                     — zero-field packet
//   packet!(id = N; StructName { f1: T1, f2: T2 }) — packet with fields
//
// All field types must implement WireRead + WireWrite.

macro_rules! packet {
    // Zero-field packet
    (id = $id:expr; $name:ident) => {
        #[derive(Debug, Clone, Default)]
        pub struct $name;

        impl crate::codec::PacketCodec for $name {
            fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId($id) }
            fn decode(_payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
                Ok(Self)
            }
            fn encode(&self) -> bytes::Bytes {
                crate::frame::RawFrame::encode(Self::packet_id().0, &[])
            }
        }
    };

    // Packet with one or more fields
    (id = $id:expr; $name:ident { $( $f:ident : $t:ty ),+ $(,)? }) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            $( pub $f: $t, )+
        }

        impl crate::codec::PacketCodec for $name {
            fn packet_id() -> crate::codec::PacketId { crate::codec::PacketId($id) }

            fn decode(payload: bytes::Bytes) -> Result<Self, crate::error::ProtocolError> {
                use crate::wire::WireRead;
                let mut buf = payload;
                Ok(Self {
                    $( $f: <$t as WireRead>::wire_read(&mut buf)?, )+
                })
            }

            fn encode(&self) -> bytes::Bytes {
                use crate::wire::WireWrite;
                let mut buf = bytes::BytesMut::new();
                $( WireWrite::wire_write(&self.$f, &mut buf); )+
                crate::frame::RawFrame::encode(Self::packet_id().0, &buf)
            }
        }
    };
}

pub(crate) use packet;
