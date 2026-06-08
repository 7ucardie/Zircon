//! Network layer — TCP listener and per-connection I/O tasks.

pub mod connection;

use std::net::SocketAddr;

use tokio::{net::TcpListener, sync::mpsc};
use tracing::{error, info};

use crate::config::Config;
use connection::{ConnectionHandle, InboundFrame};

/// A new accepted TCP connection, ready to be registered with the game loop.
pub struct NewConnection {
    pub handle: ConnectionHandle,
}

/// Bind a TCP listener and return it together with the actual local address
/// (useful when `port = 0` lets the OS pick a free port in tests).
pub async fn bind_listener(config: &Config) -> anyhow::Result<(TcpListener, SocketAddr)> {
    let addr = config.bind_addr();
    let listener = TcpListener::bind(&addr).await?;
    let local_addr = listener.local_addr()?;
    info!("Listening on {local_addr}");
    Ok((listener, local_addr))
}

/// Run the accept loop on an already-bound listener.
/// Spawns reader + writer tasks for every accepted connection and forwards
/// `NewConnection` events to the game loop.  Returns on fatal error or when
/// the game loop drops `new_conn_tx`.
pub async fn run_listener(
    listener: TcpListener,
    new_conn_tx: mpsc::Sender<NewConnection>,
    inbound_tx: mpsc::Sender<InboundFrame>,
) -> anyhow::Result<()> {
    let mut next_id: u32 = 1;

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                error!("accept error: {e}");
                continue;
            }
        };

        let conn_id = next_id;
        next_id = next_id.wrapping_add(1);

        info!(conn_id, %peer, "new connection");
        let _ = stream.set_nodelay(true);

        let handle = connection::spawn(conn_id, stream, peer, inbound_tx.clone());

        if new_conn_tx.send(NewConnection { handle }).await.is_err() {
            break; // game loop gone
        }
    }

    Ok(())
}

/// Convenience entry point used by `main`.
pub async fn start_listener(
    config: Config,
    new_conn_tx: mpsc::Sender<NewConnection>,
    inbound_tx: mpsc::Sender<InboundFrame>,
) -> anyhow::Result<()> {
    let (listener, _) = bind_listener(&config).await?;
    run_listener(listener, new_conn_tx, inbound_tx).await
}
