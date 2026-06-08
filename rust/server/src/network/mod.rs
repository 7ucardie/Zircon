//! Network layer — TCP listener and per-connection I/O tasks.

pub mod connection;

use tokio::{net::TcpListener, sync::mpsc};
use tracing::{error, info};

use crate::config::Config;
use connection::{ConnectionHandle, InboundFrame};

/// A new accepted TCP connection, ready to be registered with the game loop.
pub struct NewConnection {
    pub handle: ConnectionHandle,
}

/// Start the TCP listener.  For every accepted connection it spawns reader +
/// writer tasks and pushes a `NewConnection` to the game loop via `new_conn_tx`.
///
/// Runs forever; returns only on fatal error.
pub async fn start_listener(
    config: Config,
    new_conn_tx: mpsc::Sender<NewConnection>,
    inbound_tx: mpsc::Sender<InboundFrame>,
) -> anyhow::Result<()> {
    let addr = config.bind_addr();
    let listener = TcpListener::bind(&addr).await?;
    info!("Listening on {addr}");

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
