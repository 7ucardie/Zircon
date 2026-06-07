use anyhow::Result;
use tracing::info;

mod network;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    info!("Zircon server starting");

    let addr = "0.0.0.0:7000";
    network::listen(addr).await?;

    Ok(())
}
