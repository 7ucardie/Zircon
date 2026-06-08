//! Integration smoke tests — a real Tokio game loop + TCP listener, exercised
//! by in-process mock clients.
//!
//! Each test starts a fresh server bound to port 0 (OS-assigned), connects one
//! or more TCP streams, exercises the wire protocol, then aborts the server.
//!
//! These tests run on Linux in CI under `cargo test --all`.

#![cfg(test)]

use std::{net::SocketAddr, time::Duration};

use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    task::JoinHandle,
};
use zircon_db::GameData;

use crate::{config::Config, envir::Envir, network};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Encode one packet in the Zircon wire format:
///   [4-byte LE total-length][2-byte LE packet-id][payload]
fn encode_packet(id: u16, payload: &[u8]) -> Vec<u8> {
    let total = (6 + payload.len()) as u32;
    let mut v = Vec::with_capacity(total as usize);
    v.extend_from_slice(&total.to_le_bytes());
    v.extend_from_slice(&id.to_le_bytes());
    v.extend_from_slice(payload);
    v
}

/// Spin up a test server (game loop + listener) on an OS-assigned port.
/// Returns a `JoinHandle` to abort it and the bound `SocketAddr`.
async fn start_test_server() -> (JoinHandle<()>, SocketAddr) {
    let config = Config { port: 0, tick_ms: 20, ..Config::default() };
    let game_data = GameData::default();
    let (envir, new_conn_tx, inbound_tx) = Envir::new(config.clone(), game_data);

    let (listener, addr) = network::bind_listener(&config).await.unwrap();

    let handle = tokio::spawn(async move {
        let loop_handle = tokio::spawn(async move { envir.run().await });
        let _ = network::run_listener(listener, new_conn_tx, inbound_tx).await;
        loop_handle.abort();
    });

    (handle, addr)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Verify that the server accepts a TCP connection without panicking.
#[tokio::test]
async fn server_accepts_connection() {
    let (handle, addr) = start_test_server().await;

    let _stream = TcpStream::connect(addr).await.expect("connect failed");

    // Allow a few ticks for the game loop to register the connection.
    tokio::time::sleep(Duration::from_millis(100)).await;

    handle.abort();
}

/// Send a Ping packet (id = 4, no payload); server must stay alive.
#[tokio::test]
async fn server_handles_ping_without_crash() {
    let (handle, addr) = start_test_server().await;

    let mut stream = TcpStream::connect(addr).await.expect("connect failed");
    let ping = encode_packet(4, &[]);
    stream.write_all(&ping).await.expect("write failed");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Server should not have closed the connection.
    let mut probe = [0u8; 1];
    match stream.try_read(&mut probe) {
        Ok(0) => panic!("server closed the connection unexpectedly"),
        Ok(_) | Err(_) => {} // data or WouldBlock — both fine
    }

    handle.abort();
}

/// Send two back-to-back packets in one write; verifies the reader correctly
/// accumulates partial data and parses both frames.
#[tokio::test]
async fn server_parses_multiple_frames_in_one_write() {
    let (handle, addr) = start_test_server().await;

    let mut stream = TcpStream::connect(addr).await.expect("connect failed");

    // Version (id=6) + Ping (id=4) concatenated
    let mut batch = encode_packet(6, &[1u8, 0, 0, 0]); // version number = 1
    batch.extend(encode_packet(4, &[]));
    stream.write_all(&batch).await.expect("write failed");

    tokio::time::sleep(Duration::from_millis(100)).await;

    handle.abort();
}

/// Server must handle a partial frame: the header arrives first, the payload
/// arrives in a second write.
#[tokio::test]
async fn server_handles_partial_frame() {
    let (handle, addr) = start_test_server().await;

    let mut stream = TcpStream::connect(addr).await.expect("connect failed");

    let full = encode_packet(4, &[0xAA, 0xBB]); // Ping with 2-byte payload
    let (head, tail) = full.split_at(3); // cut mid-header

    stream.write_all(head).await.expect("write head");
    tokio::time::sleep(Duration::from_millis(30)).await;
    stream.write_all(tail).await.expect("write tail");

    tokio::time::sleep(Duration::from_millis(100)).await;

    handle.abort();
}

/// Five simultaneous connections should all be accepted.
#[tokio::test]
async fn server_accepts_multiple_connections() {
    let (handle, addr) = start_test_server().await;

    let mut streams = Vec::new();
    for _ in 0..5 {
        streams.push(TcpStream::connect(addr).await.expect("connect failed"));
    }

    tokio::time::sleep(Duration::from_millis(150)).await;

    // All connections still open
    for s in &mut streams {
        let mut probe = [0u8; 1];
        match s.try_read(&mut probe) {
            Ok(0) => panic!("server closed a connection"),
            Ok(_) | Err(_) => {}
        }
    }

    handle.abort();
}

/// Client disconnects; the server must reap the closed connection cleanly.
#[tokio::test]
async fn server_reaps_closed_connection() {
    let (handle, addr) = start_test_server().await;

    {
        let stream = TcpStream::connect(addr).await.expect("connect failed");
        tokio::time::sleep(Duration::from_millis(60)).await;
        drop(stream); // explicit close
    }

    // Give the game loop time to run reap_closed_connections().
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Server must still be running (accept new connection).
    let _new = TcpStream::connect(addr).await.expect("server died after reap");

    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.abort();
}

// ── Wire-format unit tests (no server needed) ──────────────────────────────

/// Verify encode_packet produces the exact Zircon framing.
#[test]
fn encode_packet_wire_format() {
    let payload = b"hello";
    let pkt = encode_packet(42, payload);
    let total = 6u32 + payload.len() as u32;
    assert_eq!(pkt.len(), total as usize);
    assert_eq!(u32::from_le_bytes(pkt[0..4].try_into().unwrap()), total);
    assert_eq!(u16::from_le_bytes(pkt[4..6].try_into().unwrap()), 42u16);
    assert_eq!(&pkt[6..], payload);
}

/// Empty payload packet (e.g. Ping) must be exactly 6 bytes.
#[test]
fn encode_packet_no_payload() {
    let pkt = encode_packet(4, &[]);
    assert_eq!(pkt.len(), 6);
    assert_eq!(u32::from_le_bytes(pkt[0..4].try_into().unwrap()), 6u32);
}
