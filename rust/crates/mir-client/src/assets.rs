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
    pub const GAME_INTER2: u16 = 5;
    pub const MAGIC_ICON: u16 = 20;
    pub const STORE_ITEMS: u16 = 14;
    pub const GROUND: u16 = 16;
    pub const NPC: u16 = 17;
    pub const M_HUM: u16 = 31;
    pub const M_HAIR: u16 = 41;
    pub const WM_HUM: u16 = 42;
    pub const WM_HAIR: u16 = 52;
    pub const M_HUM_A: u16 = 53;
    pub const M_HAIR_A: u16 = 58;
    pub const WM_HUM_A: u16 = 59;
    pub const WM_HAIR_A: u16 = 64;
}

/// Zircon `PlayerObject.ArmourList`: armour shape / 11 -> body library.
pub fn armour_library(shape: u16, female: bool, assassin: bool) -> Option<u16> {
    let key = shape / 11;
    let base = match (assassin, female) {
        (false, false) => [Some(31), Some(32), Some(33), Some(34), Some(35)],
        (false, true) => [Some(42), Some(43), Some(44), Some(45), Some(46)],
        (true, false) => [Some(53), Some(54), Some(55), Some(56), None],
        (true, true) => [Some(59), Some(60), Some(61), Some(62), None],
    };
    match key {
        0..=4 => base[key as usize],
        10..=13 if !assassin => Some(if female { 47 } else { 36 } + (key - 10)),
        20 => Some(match (assassin, female) {
            (false, false) => 40,
            (false, true) => 51,
            (true, false) => 57,
            (true, true) => 63,
        }),
        _ => None,
    }
}

/// Zircon `PlayerObject.HelmetList`: `(shape - 1) / 10` -> helmet library.
/// Helmet shapes are 1-based; 0 means no helmet (hair shows).
pub fn helmet_library(shape: u16, female: bool, assassin: bool) -> Option<u16> {
    if shape == 0 {
        return None;
    }
    let key = (shape - 1) / 10;
    match (assassin, female) {
        (false, false) => match key {
            0..=4 => Some(139 + key),
            10..=13 => Some(144 + key - 10),
            20 => Some(148),
            _ => None,
        },
        (false, true) => match key {
            0..=4 => Some(149 + key),
            10..=13 => Some(154 + key - 10),
            20 => Some(158),
            _ => None,
        },
        (true, false) => match key {
            0..=3 => Some(159 + key),
            20 => Some(163),
            _ => None,
        },
        (true, true) => match key {
            0..=3 => Some(164 + key),
            20 => Some(168),
            _ => None,
        },
    }
}

/// Zircon `PlayerObject.ShieldList`: shape / 10 -> shield library. Assassins
/// share the normal set.
pub fn shield_library(shape: u16, female: bool) -> Option<u16> {
    match (female, shape / 10) {
        (false, 0) => Some(135),
        (false, 1) => Some(136),
        (true, 0) => Some(137),
        (true, 1) => Some(138),
        _ => None,
    }
}

/// Zircon `PlayerObject.WeaponList`: weapon shape / 10 -> weapon library.
pub fn weapon_library(shape: u16, female: bool) -> Option<u16> {
    // Upstream fix: keys 9..15 -> M_Weapon10..16, AOH 110..115, ADL 120/121/125.
    let key = shape / 10;
    let male = match key {
        0..=6 => Some(83 + key),
        9..=15 => Some(90 + (key - 9)),
        110..=115 => Some(117 + (key - 110)),
        120 => Some(111),
        121 => Some(112),
        125 => Some(113),
        _ => None,
    }?;
    // Female libraries mirror the male ones 14 entries later for the basic
    // sets and 12 later for the AOH/ADL sets.
    Some(if female {
        match male {
            83..=96 => male + 14,
            111..=122 => male + 12,
            other => other,
        }
    } else {
        male
    })
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
            let mut path = self.root.join(rel);
            if !path.exists() {
                // Asset packs differ in filename case (Storeitems.Zl) and upstream
                // renamed StoreItems.Zl to StoreItem.Zl.
                if let (Some(dir), Some(name)) = (path.parent(), path.file_name()) {
                    let wanted = name.to_string_lossy().to_lowercase();
                    let alt = wanted.replace("storeitems.zl", "storeitem.zl");
                    if let Ok(entries) = std::fs::read_dir(dir) {
                        for e in entries.flatten() {
                            let have = e.file_name().to_string_lossy().to_lowercase();
                            if have == wanted || have == alt {
                                path = e.path();
                                break;
                            }
                        }
                    }
                }
            }
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
