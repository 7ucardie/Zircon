//! Database access layer for Zircon.
//!
//! # MirDB binary loader
//!
//! The C# server stores game data in two binary files:
//! - `System.db` — static game content (items, maps, monsters, spells, …)
//! - `Users.db` — mutable user data (accounts, characters, inventory, …)
//!
//! `GameData::load_system` reads `System.db` and returns typed collections
//! for the most-used model types.
//!
//! # SQLite (future)
//!
//! The `pool` module wraps `sqlx::SqlitePool` for future SQLite-based user
//! data storage.

pub mod binary;
pub mod error;
pub mod mapping;
pub mod models;
pub mod pool;
pub mod session;

pub use error::DbError;
pub use models::{ItemInfo, MagicInfo, MapInfo, MonsterInfo};
pub use pool::DbPool;
pub use session::{RawCollection, RawObject};

use std::path::Path;

/// All static game data loaded from `System.db`.
#[derive(Debug, Default)]
pub struct GameData {
    pub items: Vec<ItemInfo>,
    pub maps: Vec<MapInfo>,
    pub monsters: Vec<MonsterInfo>,
    pub magic: Vec<MagicInfo>,
}

impl GameData {
    /// Load game data from the `System.db` file at `path`.
    pub fn load_system(path: &Path) -> Result<Self, DbError> {
        let cols = session::load_file(path)?;
        Ok(GameData {
            items:    models::load_typed(&cols, "ItemInfo",    ItemInfo::from_props)?,
            maps:     models::load_typed(&cols, "MapInfo",     MapInfo::from_props)?,
            monsters: models::load_typed(&cols, "MonsterInfo", MonsterInfo::from_props)?,
            magic:    models::load_typed(&cols, "MagicInfo",   MagicInfo::from_props)?,
        })
    }
}
