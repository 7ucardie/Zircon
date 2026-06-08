//! Map cell loader — reads binary `.map` or JSON `.map.json` files and builds
//! a walkability grid matching the C# `Map.Load()` implementation.
//!
//! Binary format (from ServerLibrary/Models/Map.cs):
//!   bytes 22-23  : width  (LE u16)
//!   bytes 24-25  : height (LE u16)
//!   cell data starts at: 28 + width*height/4*3   (skips embedded mini-map)
//!   per cell (14 bytes): first byte is the flag
//!   walkable iff (flag & 0x03) == 0x03

use std::path::Path;

use tracing::{info, warn};
use zircon_db::MapInfo;

/// A loaded map's cell grid.
pub struct GameMap {
    pub info: MapInfo,
    pub width: u32,
    pub height: u32,
    /// Flat column-major array: index = x * height + y.
    walkable: Vec<bool>,
    pub walkable_count: usize,
}

impl GameMap {
    pub fn new_empty(info: MapInfo) -> Self {
        Self { info, width: 0, height: 0, walkable: vec![], walkable_count: 0 }
    }

    /// Try to load cells from `maps_dir/<file_name>.map` then `.map.json`;
    /// returns an empty map on failure (dev-mode graceful degradation).
    pub fn load(info: MapInfo, maps_dir: &Path) -> Self {
        let name = &info.file_name.clone();
        let bin_path  = maps_dir.join(format!("{name}.map"));
        let json_path = maps_dir.join(format!("{name}.map.json"));

        if bin_path.exists() {
            match Self::load_binary(info.clone(), &bin_path) {
                Ok(m) => {
                    info!(
                        map      = %name,
                        width    = m.width,
                        height   = m.height,
                        walkable = m.walkable_count,
                        "map loaded (binary)",
                    );
                    return m;
                }
                Err(e) => warn!(path = %bin_path.display(), "binary map load failed: {e}"),
            }
        }
        if json_path.exists() {
            match Self::load_json(info.clone(), &json_path) {
                Ok(m) => {
                    info!(
                        map      = %name,
                        width    = m.width,
                        height   = m.height,
                        walkable = m.walkable_count,
                        "map loaded (json)",
                    );
                    return m;
                }
                Err(e) => warn!(path = %json_path.display(), "json map load failed: {e}"),
            }
        }

        warn!(file_name = %name, "map cells not found — starting with empty map");
        Self::new_empty(info)
    }

    fn load_binary(info: MapInfo, path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)?;
        anyhow::ensure!(bytes.len() >= 28, "map file too short ({} bytes)", bytes.len());

        let width  = u16::from_le_bytes([bytes[22], bytes[23]]) as u32;
        let height = u16::from_le_bytes([bytes[24], bytes[25]]) as u32;
        let cell_start = 28 + (width as usize).saturating_mul(height as usize) / 4 * 3;

        let h = height as usize;
        let cell_count = (width as usize).saturating_mul(h);
        let mut walkable = vec![false; cell_count];
        let mut walkable_count = 0usize;

        for x in 0..width as usize {
            for y in 0..h {
                let pos = cell_start + (x * h + y) * 14;
                if pos >= bytes.len() {
                    break;
                }
                let flag = bytes[pos];
                if (flag & 0x03) == 0x03 {
                    walkable[x * h + y] = true;
                    walkable_count += 1;
                }
            }
        }

        Ok(Self { info, width, height, walkable, walkable_count })
    }

    fn load_json(info: MapInfo, path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let v: serde_json::Value = serde_json::from_str(&text)?;

        let width  = v["width"].as_u64().unwrap_or(0) as u32;
        let height = v["height"].as_u64().unwrap_or(0) as u32;

        let h = height as usize;
        let cell_count = (width as usize).saturating_mul(h);
        let mut walkable = vec![false; cell_count];
        let mut walkable_count = 0usize;

        if let Some(rows) = v["layout"].as_array() {
            for (y, row) in rows.iter().enumerate() {
                if y >= h {
                    break;
                }
                for (x, ch) in row.as_str().unwrap_or("").chars().enumerate() {
                    if x >= width as usize {
                        break;
                    }
                    if ch == '.' {
                        walkable[x * h + y] = true;
                        walkable_count += 1;
                    }
                }
            }
        }

        Ok(Self { info, width, height, walkable, walkable_count })
    }

    /// Returns true iff (x, y) is within bounds and walkable.
    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return false;
        }
        self.walkable[x as usize * self.height as usize + y as usize]
    }
}
