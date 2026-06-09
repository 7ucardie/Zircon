//! Async TCP network layer — connects to the Zircon server and dispatches
//! wire packets via channels.
//!
//! # Architecture
//!
//! ```text
//! Bevy systems  ← NetEvent ─ NetworkHandle::poll()
//!      │                           │
//!      └── handle.send(NetOut) ────┤
//!                                  │
//!                           background thread
//!                              (Tokio rt)
//!                                  │
//!                              TCP socket
//!                                  │
//!                            Zircon server
//! ```
//!
//! The Bevy `NetworkPlugin` (windowed feature) inserts a `NetworkHandle`
//! resource and polls it each frame.  In headless builds the handle and
//! channel types are still compiled — only the Bevy glue is gated.

use std::sync::Mutex;

use bytes::{Bytes, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc as tch;

use zircon_protocol::{
    codec::PacketCodec,
    enums::DisconnectReason,
    frame::RawFrame,
    packets::general::{Disconnect, GoodVersion, Ping, PingResponse, Version},
};

// ── Public event / command types ─────────────────────────────────────────────

/// Events delivered from the network thread to game code.
#[derive(Debug, Clone)]
pub enum NetIn {
    /// TCP connected; `Version` packet sent automatically.
    Connected,
    /// Server accepted our version.
    GoodVersion { database_key: Vec<u8> },
    /// Server sent a Ping; PingResponse queued automatically.
    Ping,
    /// Server requested client update — version mismatch.
    CheckVersion,
    /// Server disconnected us.
    Disconnect(DisconnectReason),
    /// Unrecoverable error (I/O or protocol).
    Error(String),
}

/// Commands from game code to the network thread.
#[derive(Debug)]
pub enum NetOut {
    SendVersion(Vec<u8>),
    SendPingResponse(i32),
}

// ── Connection status ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Ready,
    Disconnected(String),
}

// ── NetworkHandle resource ────────────────────────────────────────────────────

/// Owns both ends of the channel pair.
///
/// Insert as a Bevy `Resource` (requires the `windowed` feature) or use
/// directly in headless code.
pub struct NetworkHandle {
    rx: Mutex<tch::UnboundedReceiver<NetIn>>,
    pub tx: tch::UnboundedSender<NetOut>,
    pub status: ConnectionStatus,
}

// Bevy Resource marker — only present when Bevy is compiled in.
#[cfg(feature = "windowed")]
impl bevy::ecs::system::Resource for NetworkHandle {}

impl NetworkHandle {
    /// Drain all pending inbound events (non-blocking).
    pub fn poll(&self) -> Vec<NetIn> {
        let mut rx = self.rx.lock().unwrap();
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    /// Queue an outbound command (best-effort; silently drops if disconnected).
    pub fn send(&self, msg: NetOut) {
        let _ = self.tx.send(msg);
    }
}

// ── Thread spawner ────────────────────────────────────────────────────────────

/// Spawn a background Tokio thread and return the `NetworkHandle`.
pub fn start_network(addr: String) -> NetworkHandle {
    let (in_tx, in_rx) = tch::unbounded_channel::<NetIn>();
    let (out_tx, out_rx) = tch::unbounded_channel::<NetOut>();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(run(addr, in_tx, out_rx));
    });

    NetworkHandle {
        rx: Mutex::new(in_rx),
        tx: out_tx,
        status: ConnectionStatus::Connecting,
    }
}

// ── Async connection loop ─────────────────────────────────────────────────────

async fn run(
    addr: String,
    tx: tch::UnboundedSender<NetIn>,
    mut out_rx: tch::UnboundedReceiver<NetOut>,
) {
    let stream = match tokio::net::TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => {
            let _ = tx.send(NetIn::Error(format!("connect {addr}: {e}")));
            return;
        }
    };

    let (mut read_half, mut write_half) = stream.into_split();
    let mut buf = BytesMut::with_capacity(4096);

    loop {
        tokio::select! {
            // ── Inbound: data arriving from the server ────────────────────
            result = read_half.read_buf(&mut buf) => {
                match result {
                    Ok(0) => {
                        let _ = tx.send(NetIn::Disconnect(DisconnectReason::ConnectionLost));
                        return;
                    }
                    Ok(_) => {
                        loop {
                            match RawFrame::parse(&mut buf) {
                                Ok(Some(frame)) => {
                                    if let Some(reply) = dispatch_frame(&frame, &tx) {
                                        if write_half.write_all(&reply).await.is_err() {
                                            return;
                                        }
                                    }
                                    // If dispatch_frame sends a Disconnect event, keep reading
                                    // (server may send more frames before closing).
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    let _ = tx.send(NetIn::Error(format!("protocol: {e}")));
                                    return;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(NetIn::Error(format!("read: {e}")));
                        return;
                    }
                }
            }

            // ── Outbound: commands from game code ─────────────────────────
            msg = out_rx.recv() => {
                let Some(msg) = msg else { return };
                let bytes = outbound_to_bytes(msg);
                if write_half.write_all(&bytes).await.is_err() {
                    return;
                }
            }
        }
    }
}

// ── Pure frame dispatch ───────────────────────────────────────────────────────

/// Handle one received frame.
///
/// Returns `Some(Bytes)` when the protocol requires an immediate reply,
/// e.g. `Connected → Version`, `Ping → PingResponse`.
///
/// Separated from the async I/O so it can be unit-tested without sockets.
pub fn dispatch_frame(frame: &RawFrame, tx: &tch::UnboundedSender<NetIn>) -> Option<Bytes> {
    match frame.packet_id {
        0 => {
            // CheckVersion — server wants us to update the client
            let _ = tx.send(NetIn::CheckVersion);
            None
        }
        1 => {
            // Connected — auto-send Version with empty hash for PoC
            let _ = tx.send(NetIn::Connected);
            Some(Version { client_hash: vec![] }.encode())
        }
        2 => {
            let reason = Disconnect::decode(frame.payload.clone())
                .map(|p| p.reason)
                .unwrap_or(DisconnectReason::Unknown);
            let _ = tx.send(NetIn::Disconnect(reason));
            None
        }
        3 => {
            let key = GoodVersion::decode(frame.payload.clone())
                .map(|p| p.database_key)
                .unwrap_or_default();
            let _ = tx.send(NetIn::GoodVersion { database_key: key });
            None
        }
        4 => {
            // Ping — auto-reply with zero latency for PoC
            let _ = tx.send(NetIn::Ping);
            Some(PingResponse { ping: 0 }.encode())
        }
        _ => None,
    }
}

fn outbound_to_bytes(msg: NetOut) -> Bytes {
    match msg {
        NetOut::SendVersion(hash) => Version { client_hash: hash }.encode(),
        NetOut::SendPingResponse(ping) => PingResponse { ping }.encode(),
    }
}

// ── Bevy plugin (windowed only) ───────────────────────────────────────────────

#[cfg(feature = "windowed")]
pub use bevy_glue::NetworkPlugin;

#[cfg(feature = "windowed")]
mod bevy_glue {
    use super::*;
    use bevy::prelude::*;

    /// Spawns the network thread and wires events into the Bevy app.
    pub struct NetworkPlugin {
        pub server_addr: String,
    }

    /// Bevy event wrapping each received network message.
    #[derive(Event, Debug, Clone)]
    pub struct NetEvent(pub NetIn);

    impl Plugin for NetworkPlugin {
        fn build(&self, app: &mut App) {
            let handle = start_network(self.server_addr.clone());
            app.insert_resource(handle)
                .add_event::<NetEvent>()
                .add_systems(Update, poll_network);
        }
    }

    fn poll_network(
        mut handle: ResMut<NetworkHandle>,
        mut events: EventWriter<NetEvent>,
    ) {
        let msgs = handle.poll();
        for msg in msgs {
            match &msg {
                NetIn::Connected => handle.status = ConnectionStatus::Connected,
                NetIn::GoodVersion { .. } => handle.status = ConnectionStatus::Ready,
                NetIn::Disconnect(r) => {
                    handle.status = ConnectionStatus::Disconnected(format!("{r:?}"));
                }
                NetIn::Error(e) => {
                    handle.status = ConnectionStatus::Disconnected(e.clone());
                }
                NetIn::Ping | NetIn::CheckVersion => {}
            }
            events.send(NetEvent(msg));
        }
    }
}

// ── Pure UI helpers ──────────────────────────────────────────────────────────
// Kept here (always compiled) so tests run even in headless/CI builds.

/// HP/MP bar fill percentage (0–100), clamped and safe against zero max.
pub fn bar_pct(current: u32, max: u32) -> f32 {
    if max == 0 {
        return 0.0;
    }
    (current as f32 / max as f32 * 100.0).clamp(0.0, 100.0)
}

/// One-line status string for the HUD overlay.
pub fn status_label(status: &ConnectionStatus) -> String {
    match status {
        ConnectionStatus::Connecting    => "Status: Connecting…".to_string(),
        ConnectionStatus::Connected     => "Status: Connected".to_string(),
        ConnectionStatus::Ready         => "Status: Online".to_string(),
        ConnectionStatus::Disconnected(r) => format!("Disconnected: {r}"),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use zircon_protocol::codec::PacketCodec;
    use zircon_protocol::packets::general::{Connected, Ping};

    fn make_frame(packet_id: u16, payload: &[u8]) -> RawFrame {
        // Build a complete frame, then parse it to get a RawFrame.
        let encoded = RawFrame::encode(packet_id, payload);
        let mut buf = BytesMut::from(encoded.as_ref());
        RawFrame::parse(&mut buf).unwrap().unwrap()
    }

    #[test]
    fn connected_frame_sends_version_reply() {
        let (tx, mut rx) = tch::unbounded_channel();
        let frame = make_frame(1, &Connected.encode()[6..]); // payload only
        let reply = dispatch_frame(&frame, &tx);
        assert!(reply.is_some(), "should reply with Version bytes");
        // Version packet id = 6
        let reply = reply.unwrap();
        assert!(reply.len() >= 6, "reply too short");
        // packet_id at bytes [4..6]
        let pid = u16::from_le_bytes([reply[4], reply[5]]);
        assert_eq!(pid, 6, "reply should be a Version packet (id=6)");
        // event sent
        assert!(matches!(rx.try_recv(), Ok(NetIn::Connected)));
    }

    #[test]
    fn ping_frame_sends_ping_response() {
        let (tx, mut rx) = tch::unbounded_channel();
        let frame = make_frame(4, &Ping.encode()[6..]); // zero-field, empty payload
        let reply = dispatch_frame(&frame, &tx);
        assert!(reply.is_some(), "should reply with PingResponse");
        let reply = reply.unwrap();
        let pid = u16::from_le_bytes([reply[4], reply[5]]);
        assert_eq!(pid, 5, "reply should be PingResponse (id=5)");
        assert!(matches!(rx.try_recv(), Ok(NetIn::Ping)));
    }

    #[test]
    fn disconnect_frame_sends_disconnect_event() {
        let (tx, mut rx) = tch::unbounded_channel();
        // Encode a Disconnect packet and strip the 6-byte frame header for payload
        let full = Disconnect { reason: DisconnectReason::BadVersion }.encode();
        let frame = make_frame(2, &full[6..]);
        let reply = dispatch_frame(&frame, &tx);
        assert!(reply.is_none(), "Disconnect needs no reply");
        match rx.try_recv() {
            Ok(NetIn::Disconnect(r)) => assert_eq!(r, DisconnectReason::BadVersion),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn good_version_frame_fires_event() {
        let (tx, mut rx) = tch::unbounded_channel();
        let full = GoodVersion { database_key: vec![1, 2, 3] }.encode();
        let frame = make_frame(3, &full[6..]);
        let reply = dispatch_frame(&frame, &tx);
        assert!(reply.is_none());
        match rx.try_recv() {
            Ok(NetIn::GoodVersion { database_key }) => assert_eq!(database_key, &[1, 2, 3]),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn unknown_packet_ignored() {
        let (tx, mut rx) = tch::unbounded_channel();
        let frame = make_frame(999, &[]);
        let reply = dispatch_frame(&frame, &tx);
        assert!(reply.is_none());
        assert!(rx.try_recv().is_err(), "should send no events for unknown packets");
    }

    #[test]
    fn outbound_version_encodes_correctly() {
        let hash = vec![0xABu8, 0xCD];
        let bytes = outbound_to_bytes(NetOut::SendVersion(hash.clone()));
        // Parse it back
        let mut buf = BytesMut::from(bytes.as_ref());
        let frame = RawFrame::parse(&mut buf).unwrap().unwrap();
        assert_eq!(frame.packet_id, 6);
        let v = Version::decode(frame.payload).unwrap();
        assert_eq!(v.client_hash, hash);
    }

    #[test]
    fn outbound_ping_response_encodes_correctly() {
        let bytes = outbound_to_bytes(NetOut::SendPingResponse(42));
        let mut buf = BytesMut::from(bytes.as_ref());
        let frame = RawFrame::parse(&mut buf).unwrap().unwrap();
        assert_eq!(frame.packet_id, 5);
        let pr = PingResponse::decode(frame.payload).unwrap();
        assert_eq!(pr.ping, 42);
    }

    // ── bar_pct / status_label (UI helpers, always compiled) ──────────────

    #[test]
    fn bar_pct_normal_range() {
        assert_eq!(bar_pct(50, 100), 50.0);
        assert_eq!(bar_pct(100, 100), 100.0);
        assert_eq!(bar_pct(0, 100), 0.0);
    }

    #[test]
    fn bar_pct_zero_max_returns_zero() {
        assert_eq!(bar_pct(0, 0), 0.0);
        assert_eq!(bar_pct(99, 0), 0.0);
    }

    #[test]
    fn bar_pct_overflow_clamped_to_100() {
        assert_eq!(bar_pct(150, 100), 100.0);
    }

    #[test]
    fn bar_pct_fractional() {
        let p = bar_pct(1, 3);
        assert!((p - 33.333).abs() < 0.01, "expected ~33.3, got {p}");
    }

    #[test]
    fn status_label_variants() {
        assert!(status_label(&ConnectionStatus::Connecting).contains("Connecting"));
        assert!(status_label(&ConnectionStatus::Connected).contains("Connected"));
        assert!(status_label(&ConnectionStatus::Ready).contains("Online"));
        let disc = status_label(&ConnectionStatus::Disconnected("BadVersion".into()));
        assert!(disc.contains("BadVersion"), "{disc}");
    }
}
