//! Per-client connection — two async Tokio tasks that bridge
//! the TCP stream to/from the game loop via mpsc channels.
//!
//! Architecture:
//!   TCP stream → reader task → inbound_tx (frames to game loop)
//!   outbound_rx (bytes from game loop) → writer task → TCP stream

use std::net::SocketAddr;

use bytes::{Bytes, BytesMut};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};
use tracing::{debug, error};
use zircon_protocol::RawFrame;

/// Channel sizes for the per-connection queues.
const INBOUND_CAP: usize = 256;
const OUTBOUND_CAP: usize = 256;

/// A decoded inbound frame from a client.
#[derive(Debug)]
pub struct InboundFrame {
    pub conn_id: u32,
    pub frame: RawFrame,
}

/// Handle held by the game loop to talk to one connection.
pub struct ConnectionHandle {
    pub conn_id: u32,
    pub addr: SocketAddr,
    /// Send encoded frames down to the client.
    pub outbound_tx: mpsc::Sender<Bytes>,
}

impl ConnectionHandle {
    /// Queue a pre-encoded packet for sending.
    pub fn send(&self, data: Bytes) {
        let _ = self.outbound_tx.try_send(data);
    }

    pub fn is_closed(&self) -> bool {
        self.outbound_tx.is_closed()
    }
}

/// Spawn the reader + writer tasks for one accepted TCP connection.
///
/// Returns a `ConnectionHandle` for the game loop and a `Receiver` that
/// delivers decoded inbound frames to the game loop.
pub fn spawn(
    conn_id: u32,
    stream: TcpStream,
    addr: SocketAddr,
    inbound_tx: mpsc::Sender<InboundFrame>,
) -> ConnectionHandle {
    let (outbound_tx, outbound_rx) = mpsc::channel::<Bytes>(OUTBOUND_CAP);
    let (read_half, write_half) = stream.into_split();

    // Spawn reader
    tokio::spawn(reader_task(conn_id, read_half, inbound_tx, INBOUND_CAP));

    // Spawn writer
    tokio::spawn(writer_task(conn_id, write_half, outbound_rx));

    ConnectionHandle { conn_id, addr, outbound_tx }
}

async fn reader_task(
    conn_id: u32,
    mut read_half: tokio::net::tcp::OwnedReadHalf,
    inbound_tx: mpsc::Sender<InboundFrame>,
    _cap: usize,
) {
    let mut buf = BytesMut::with_capacity(8 * 1024);

    loop {
        match read_half.read_buf(&mut buf).await {
            Ok(0) => {
                debug!(conn_id, "connection closed by peer");
                break;
            }
            Ok(_) => {}
            Err(e) => {
                error!(conn_id, "read error: {e}");
                break;
            }
        }

        // Parse all complete frames from the buffer.
        loop {
            match RawFrame::parse(&mut buf) {
                Ok(Some(frame)) => {
                    if inbound_tx.send(InboundFrame { conn_id, frame }).await.is_err() {
                        // Game loop dropped the channel — server shutting down.
                        return;
                    }
                }
                Ok(None) => break, // need more bytes
                Err(e) => {
                    error!(conn_id, "frame parse error: {e}");
                    return;
                }
            }
        }
    }
}

async fn writer_task(
    conn_id: u32,
    mut write_half: tokio::net::tcp::OwnedWriteHalf,
    mut outbound_rx: mpsc::Receiver<Bytes>,
) {
    while let Some(data) = outbound_rx.recv().await {
        if let Err(e) = write_half.write_all(&data).await {
            error!(conn_id, "write error: {e}");
            break;
        }
    }
    debug!(conn_id, "writer task exiting");
}
