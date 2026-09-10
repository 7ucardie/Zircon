//! TCP front-end: one reader and one writer task per connection, feeding the
//! single-threaded game loop through channels.

use std::net::SocketAddr;

use mir_proto::{decode_frame, encode, ClientMessage, ServerMessage};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::world::ConnId;

#[derive(Debug)]
pub enum Inbound {
    Connected {
        conn: ConnId,
        addr: SocketAddr,
        tx: mpsc::UnboundedSender<ServerMessage>,
    },
    Message {
        conn: ConnId,
        msg: ClientMessage,
    },
    Disconnected {
        conn: ConnId,
    },
}

pub async fn listen(
    addr: SocketAddr,
    inbound: mpsc::UnboundedSender<Inbound>,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "listening");
    let mut next_conn: ConnId = 1;
    loop {
        let (stream, peer) = listener.accept().await?;
        let conn = next_conn;
        next_conn += 1;
        let inbound = inbound.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(stream, peer, conn, inbound).await {
                tracing::debug!(conn, "connection ended: {e}");
            }
        });
    }
}

async fn handle(
    stream: TcpStream,
    peer: SocketAddr,
    conn: ConnId,
    inbound: mpsc::UnboundedSender<Inbound>,
) -> anyhow::Result<()> {
    stream.set_nodelay(true)?;
    let (mut reader, mut writer) = stream.into_split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();
    inbound.send(Inbound::Connected {
        conn,
        addr: peer,
        tx,
    })?;

    let writer_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let frame = match encode(&msg) {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!("encode failed: {e}");
                    continue;
                }
            };
            if writer.write_all(&frame).await.is_err() {
                break;
            }
        }
    });

    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let result: anyhow::Result<()> = async {
        loop {
            let n = reader.read(&mut chunk).await?;
            if n == 0 {
                return Ok(());
            }
            buf.extend_from_slice(&chunk[..n]);
            while let Some(msg) = decode_frame::<ClientMessage>(&mut buf)? {
                inbound.send(Inbound::Message { conn, msg })?;
            }
        }
    }
    .await;
    let _ = inbound.send(Inbound::Disconnected { conn });
    writer_task.abort();
    result
}
