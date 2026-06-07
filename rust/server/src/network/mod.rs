use anyhow::Result;
use bytes::BytesMut;
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
};
use tracing::{error, info};
use zircon_protocol::RawFrame;

/// Start the TCP listener and accept connections.
pub async fn listen(addr: &str) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!("Listening on {addr}");

    loop {
        let (socket, peer) = listener.accept().await?;
        info!("New connection from {peer}");
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket).await {
                error!("Connection {peer} error: {e}");
            }
            info!("Connection {peer} closed");
        });
    }
}

async fn handle_connection(mut socket: TcpStream) -> Result<()> {
    let mut buf = BytesMut::with_capacity(4096);

    loop {
        let n = socket.read_buf(&mut buf).await?;
        if n == 0 {
            break; // client disconnected
        }

        while let Some(frame) = RawFrame::parse(&mut buf)? {
            tracing::debug!(packet_id = frame.packet_id, len = frame.payload.len(), "rx frame");
            // TODO: dispatch frame to packet handlers
            let _ = frame;
        }
    }

    Ok(())
}
