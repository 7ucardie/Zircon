//! Server configuration, loaded from environment or defaults.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    /// Bind address for the game port.
    pub address: String,
    /// Game TCP port.
    pub port: u16,
    /// Path to the database directory containing System.db / Users.db.
    pub db_path: PathBuf,
    /// Milliseconds per game tick (default 50 ms = 20 TPS).
    pub tick_ms: u64,
    /// Maximum simultaneous connections.
    pub max_connections: usize,
    /// How often to save user data (in ticks).  At 20 TPS, 1200 = 60 s.
    pub save_interval_ticks: u64,
    /// Per-connection inbound packet queue cap (anti-DDoS).
    pub max_packet_queue: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            address:              "0.0.0.0".into(),
            port:                 7000,
            db_path:              PathBuf::from("Database"),
            tick_ms:              50,
            max_connections:      1000,
            save_interval_ticks:  1200,
            max_packet_queue:     100,
        }
    }
}

impl Config {
    /// Override fields from environment variables where present.
    pub fn from_env() -> Self {
        let mut c = Config::default();
        if let Ok(v) = std::env::var("ZIRCON_PORT") {
            if let Ok(n) = v.parse() { c.port = n; }
        }
        if let Ok(v) = std::env::var("ZIRCON_DB") {
            c.db_path = PathBuf::from(v);
        }
        if let Ok(v) = std::env::var("ZIRCON_TICK_MS") {
            if let Ok(n) = v.parse() { c.tick_ms = n; }
        }
        c
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}
