//! Asset resolution: Zircon `LibraryFile` ids -> `.Zl` files on disk, plus a
//! cache of open libraries.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mir_formats::zl::{ImageInfo, SurfaceKind, ZlLibrary};

use crate::monster_table::library_path;

pub mod lib {
    pub const INTERFACE: u16 = 3;
    pub const GAME_INTER: u16 = 4;
    pub const M_HUM: u16 = 31;
    pub const M_HAIR: u16 = 41;
    pub const WM_HUM: u16 = 42;
    pub const WM_HAIR: u16 = 52;
}

/// Map-file library byte (`Libraries.KROrder`) -> `LibraryFile` value.
pub fn kr_library(file: u8) -> Option<u16> {
    // Base set, in LibraryFile order 254..=267.
    const BASE: [(&str, u16); 14] = [
        ("Animationsc", 254),
        ("Cliffsc", 255),
        ("Dungeonsc", 256),
        ("Furnituresc", 257),
        ("Housesc", 258),
        ("Innersc", 259),
        ("Object1c", 260),
        ("Object2c", 261),
        ("SmObjectsc", 262),
        ("SmTilesc", 263),
        ("Tiles5c", 264),
        ("Tiles30c", 265),
        ("Tilesc", 266),
        ("Wallsc", 267),
    ];
    // Themed sets (Forest 268, Sand 280, Snow 292, Wood 304) in this order.
    const THEMED: [&str; 12] = [
        "Animationsc",
        "Cliffsc",
        "Dungeonsc",
        "Furnituresc",
        "Housesc",
        "Innersc",
        "SmObjectsc",
        "SmTilesc",
        "Tiles5c",
        "Tiles30c",
        "Tilesc",
        "Wallsc",
    ];
    let (theme, name) = mir_formats::map::kr_order(file)?;
    if theme.is_empty() {
        return BASE.iter().find(|(n, _)| *n == name).map(|(_, v)| *v);
    }
    let base = match theme {
        "Forest" => 268,
        "Sand" => 280,
        "Snow" => 292,
        "Wood" => 304,
        _ => return None,
    };
    THEMED
        .iter()
        .position(|n| *n == name)
        .map(|i| base + i as u16)
}

pub struct Assets {
    root: PathBuf,
    libs: HashMap<u16, Option<Arc<ZlLibrary>>>,
}

impl Assets {
    pub fn new(root: impl AsRef<Path>) -> Assets {
        Assets {
            root: root.as_ref().to_path_buf(),
            libs: HashMap::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn library(&mut self, id: u16) -> Option<Arc<ZlLibrary>> {
        if let Some(l) = self.libs.get(&id) {
            return l.clone();
        }
        let loaded = library_path(id).and_then(|rel| {
            let path = self.root.join(rel);
            match ZlLibrary::open(&path) {
                Ok(l) => {
                    tracing::info!(id, path = %path.display(), images = l.len(), version = l.version(), "library opened");
                    Some(Arc::new(l))
                }
                Err(e) => {
                    tracing::warn!(id, path = %path.display(), "cannot open library: {e}");
                    None
                }
            }
        });
        self.libs.insert(id, loaded.clone());
        loaded
    }

    pub fn info(&mut self, id: u16, index: u32) -> Option<ImageInfo> {
        self.library(id)?.info(index as usize).copied()
    }

    /// Decode a surface to (padded width, padded height, rgba).
    pub fn decode(
        &mut self,
        id: u16,
        index: u32,
        kind: SurfaceKind,
    ) -> Option<(u32, u32, Vec<u8>)> {
        let lib = self.library(id)?;
        match lib.decode(index as usize, kind) {
            Ok(Some(s)) => Some((s.width, s.height, s.rgba)),
            Ok(None) => None,
            Err(e) => {
                tracing::warn!(id, index, "decode failed: {e}");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kr_library_values() {
        assert_eq!(kr_library(0), Some(266)); // Tilesc
        assert_eq!(kr_library(11), Some(254)); // Animationsc
        assert_eq!(kr_library(12), Some(260)); // Object1c
        assert_eq!(kr_library(60), Some(278)); // Forest_Tilesc
        assert_eq!(kr_library(15), Some(314)); // Wood_Tilesc
        assert_eq!(kr_library(14), None);
    }

    #[test]
    fn library_paths_match_generated_table() {
        assert_eq!(library_path(266), Some("Data/Map Data/Tilesc.Zl"));
        assert_eq!(library_path(278), Some("Data/Map Data/Forest/Tilesc.Zl"));
        assert_eq!(library_path(314), Some("Data/Map Data/Wood/Tilesc.Zl"));
        assert_eq!(library_path(lib::M_HUM), Some("Data/M-Hum.Zl"));
        assert_eq!(library_path(lib::GAME_INTER), Some("Data/GameInter.Zl"));
    }
}
