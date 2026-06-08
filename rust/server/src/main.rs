use anyhow::Result;
use tracing::info;

mod config;
mod envir;
mod map;
mod network;
mod objects;

use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    info!("Zircon server starting");

    let cfg = Config::from_env();

    // Load static game data (System.db).  Missing file → empty data (dev mode).
    let system_db = cfg.db_path.join("System.db");
    let game_data = if system_db.exists() {
        info!(path = %system_db.display(), "loading System.db");
        zircon_db::GameData::load_system(&system_db)?
    } else {
        info!("System.db not found — starting with empty game data");
        zircon_db::GameData::default()
    };
    info!(
        items = game_data.items.len(),
        maps  = game_data.maps.len(),
        monsters = game_data.monsters.len(),
        magic = game_data.magic.len(),
        "game data loaded"
    );

    // Build the game loop and get the channel endpoints.
    let (envir, new_conn_tx, inbound_tx) = envir::Envir::new(cfg.clone(), game_data);

    // Spawn the game loop.
    let loop_handle = tokio::spawn(async move { envir.run().await });

    // Start the TCP listener (blocks until shutdown / fatal error).
    let listener_result =
        network::start_listener(cfg, new_conn_tx, inbound_tx).await;

    if let Err(e) = listener_result {
        tracing::error!("listener error: {e}");
    }

    loop_handle.abort();
    Ok(())
}
