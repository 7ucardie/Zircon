//! Map loading — reads `.map.json` files produced by the C# server or MapConverter.
//!
//! The JSON format is:
//! ```json
//! { "width": N, "height": M, "layout": ["row0", "row1", ...] }
//! ```
//! where `'.'` = walkable cell, `'#'` = blocked cell.

use serde::Deserialize;
use std::path::Path;

/// Parsed `.map.json` file.
#[derive(Debug, Clone, Deserialize)]
pub struct MapFile {
    pub width: u32,
    pub height: u32,
    pub layout: Vec<String>,
}

impl MapFile {
    /// Load and parse a `.map.json` file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("cannot read {:?}: {e}", path))?;
        Self::from_str(&data)
    }

    /// Parse from an in-memory JSON string (useful for tests).
    pub fn from_str(json: &str) -> anyhow::Result<Self> {
        let m: MapFile = serde_json::from_str(json)
            .map_err(|e| anyhow::anyhow!("invalid map JSON: {e}"))?;
        anyhow::ensure!(
            m.layout.len() == m.height as usize,
            "layout row count {} != height {}",
            m.layout.len(),
            m.height
        );
        for (i, row) in m.layout.iter().enumerate() {
            anyhow::ensure!(
                row.chars().count() == m.width as usize,
                "row {} length {} != width {}",
                i,
                row.chars().count(),
                m.width
            );
        }
        Ok(m)
    }

    /// Returns `true` if cell `(x, y)` is walkable.
    pub fn is_walkable(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let Some(row) = self.layout.get(y as usize) else { return false };
        row.as_bytes().get(x as usize).copied() == Some(b'.')
    }

    /// Count of walkable cells.
    pub fn walkable_count(&self) -> usize {
        self.layout.iter().flat_map(|row| row.bytes()).filter(|&b| b == b'.').count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL_MAP: &str = r####"{"width":3,"height":3,"layout":["###","#.#","###"]}"####;

    #[test]
    fn parse_small_map() {
        let m = MapFile::from_str(SMALL_MAP).unwrap();
        assert_eq!(m.width, 3);
        assert_eq!(m.height, 3);
        assert_eq!(m.layout.len(), 3);
    }

    #[test]
    fn centre_is_walkable() {
        let m = MapFile::from_str(SMALL_MAP).unwrap();
        assert!(m.is_walkable(1, 1));
    }

    #[test]
    fn corners_are_blocked() {
        let m = MapFile::from_str(SMALL_MAP).unwrap();
        assert!(!m.is_walkable(0, 0));
        assert!(!m.is_walkable(2, 0));
        assert!(!m.is_walkable(0, 2));
        assert!(!m.is_walkable(2, 2));
    }

    #[test]
    fn out_of_bounds_is_not_walkable() {
        let m = MapFile::from_str(SMALL_MAP).unwrap();
        assert!(!m.is_walkable(3, 0));
        assert!(!m.is_walkable(0, 3));
        assert!(!m.is_walkable(100, 100));
    }

    #[test]
    fn walkable_count_correct() {
        let m = MapFile::from_str(SMALL_MAP).unwrap();
        assert_eq!(m.walkable_count(), 1);
    }

    #[test]
    fn open_map_all_walkable() {
        let json = r#"{"width":2,"height":2,"layout":["..",".."]}"#;
        let m = MapFile::from_str(json).unwrap();
        assert_eq!(m.walkable_count(), 4);
        assert!(m.is_walkable(0, 0));
        assert!(m.is_walkable(1, 1));
    }

    #[test]
    fn invalid_json_returns_error() {
        assert!(MapFile::from_str("not json").is_err());
    }

    #[test]
    fn load_missing_file_returns_error() {
        assert!(MapFile::load(Path::new("/nonexistent/map.json")).is_err());
    }

    #[test]
    fn wrong_row_count_errors() {
        let json = r#"{"width":2,"height":3,"layout":["..",".."]}"#;
        assert!(MapFile::from_str(json).is_err());
    }

    #[test]
    fn wrong_row_width_errors() {
        let json = r#"{"width":3,"height":1,"layout":[".."]}"#;
        assert!(MapFile::from_str(json).is_err());
    }
}
