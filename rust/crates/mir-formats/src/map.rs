//! `.map` reader (Zircon `MapControl.LoadMap` / server `Map.Load`).
//!
//! ```text
//! offset 22: i16 width, i16 height
//! offset 28: back layer, (width/2)*(height/2) records of 3 bytes, x-major:
//!            u8 back_file, u16 back_image           -> stored at cell (x*2, y*2)
//! then:      width*height records of 14 bytes, x-major:
//!            u8 flag, u8 middle_anim, u8 front_anim, u8 front_file, u8 middle_file,
//!            u16 middle_image, u16 front_image, 3 skipped, u8 light, 1 skipped
//! ```
//! A cell is walkable only when both bits 0 and 1 of `flag` are set.

use std::path::Path;

use crate::{Cursor, FormatError, Result};

#[derive(Debug, Clone, Copy, Default)]
pub struct Cell {
    pub flag: u8,
    /// Zircon `KROrder` key of the back tile library (only meaningful on even x/y cells).
    pub back_file: u8,
    /// Raw back image index (used verbatim).
    pub back_image: u16,
    pub middle_file: u8,
    /// Raw middle image index (Zircon stores +1 and subtracts at draw time).
    pub middle_image: u16,
    pub front_file: u8,
    pub front_image: u16,
    /// Raw animation byte for the middle tile.
    pub middle_anim: u8,
    /// Front animation byte masked with 0x8F (255 -> 0).
    pub front_anim: u8,
    /// Light level, 0..=30.
    pub light: u8,
}

impl Cell {
    pub fn is_walkable(&self) -> bool {
        (self.flag & 0x01) == 1 && (self.flag & 0x02) == 2
    }
    pub fn middle_anim_count(&self) -> u8 {
        self.middle_anim & 0x0F
    }
    pub fn middle_anim_blend(&self) -> bool {
        self.middle_anim & 0x80 != 0
    }
    /// Zircon animates a middle tile only when `1 < anim < 255`.
    pub fn middle_animated(&self) -> bool {
        self.middle_anim > 1 && self.middle_anim < 255
    }
    pub fn front_anim_count(&self) -> u8 {
        self.front_anim & 0x0F
    }
    pub fn front_anim_blend(&self) -> bool {
        self.front_anim & 0x80 != 0
    }
    pub fn front_animated(&self) -> bool {
        self.front_anim > 1 && self.front_anim < 255
    }
}

#[derive(Debug, Clone)]
pub struct MapFile {
    pub width: u16,
    pub height: u16,
    cells: Vec<Cell>,
}

impl MapFile {
    pub fn load(path: impl AsRef<Path>) -> Result<MapFile> {
        let data = std::fs::read(path)?;
        MapFile::parse(&data)
    }

    pub fn parse(data: &[u8]) -> Result<MapFile> {
        let mut c = Cursor::at(data, 22);
        let width = c.i16()?;
        let height = c.i16()?;
        if width <= 0 || height <= 0 {
            return Err(FormatError::Invalid("map dimensions"));
        }
        let (w, h) = (width as usize, height as usize);
        let mut cells = vec![Cell::default(); w * h];
        c.seek(28);
        for x in 0..w / 2 {
            for y in 0..h / 2 {
                let back_file = c.u8()?;
                let back_image = c.u16()?;
                let cell = &mut cells[(y * 2) * w + x * 2];
                cell.back_file = back_file;
                cell.back_image = back_image;
            }
        }
        for x in 0..w {
            for y in 0..h {
                let cell = &mut cells[y * w + x];
                cell.flag = c.u8()?;
                cell.middle_anim = c.u8()?;
                let v = c.u8()?;
                cell.front_anim = (if v == 255 { 0 } else { v }) & 0x8F;
                cell.front_file = c.u8()?;
                cell.middle_file = c.u8()?;
                cell.middle_image = c.u16()?;
                cell.front_image = c.u16()?;
                c.skip(3)?;
                cell.light = (c.u8()? & 0x0F) * 2;
                c.skip(1)?;
            }
        }
        Ok(MapFile {
            width: width as u16,
            height: height as u16,
            cells,
        })
    }

    pub fn cell(&self, x: i32, y: i32) -> Option<&Cell> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        Some(&self.cells[y as usize * self.width as usize + x as usize])
    }

    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        self.cell(x, y).map(|c| c.is_walkable()).unwrap_or(false)
    }

    pub fn walkable_count(&self) -> usize {
        self.cells.iter().filter(|c| c.is_walkable()).count()
    }
}

/// Zircon `Libraries.KROrder`: map-file library byte -> library name (file stem
/// under `Data/Map Data/`, with an optional theme subfolder).
pub fn kr_order(file: u8) -> Option<(&'static str, &'static str)> {
    const BASE: [&str; 12] = [
        "Tilesc",
        "Tiles30c",
        "Tiles5c",
        "SmTilesc",
        "Housesc",
        "Cliffsc",
        "Dungeonsc",
        "Innersc",
        "Furnituresc",
        "Wallsc",
        "SmObjectsc",
        "Animationsc",
    ];
    match file {
        0..=11 => Some(("", BASE[file as usize])),
        12 => Some(("", "Object1c")),
        13 => Some(("", "Object2c")),
        15..=26 => Some(("Wood", BASE[(file - 15) as usize])),
        30..=41 => Some(("Sand", BASE[(file - 30) as usize])),
        45..=56 => Some(("Snow", BASE[(file - 45) as usize])),
        60..=71 => Some(("Forest", BASE[(file - 60) as usize])),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kr_order_table() {
        assert_eq!(kr_order(0), Some(("", "Tilesc")));
        assert_eq!(kr_order(13), Some(("", "Object2c")));
        assert_eq!(kr_order(14), None);
        assert_eq!(kr_order(15), Some(("Wood", "Tilesc")));
        assert_eq!(kr_order(41), Some(("Sand", "Animationsc")));
        assert_eq!(kr_order(60), Some(("Forest", "Tilesc")));
        assert_eq!(kr_order(71), Some(("Forest", "Animationsc")));
        assert_eq!(kr_order(72), None);
    }

    #[test]
    fn parses_map_zero() {
        let Some(assets) = std::env::var_os("ZIRCON_ASSETS") else {
            eprintln!("ZIRCON_ASSETS not set; skipping");
            return;
        };
        let map = MapFile::load(Path::new(&assets).join("Map/0.map")).unwrap();
        assert_eq!((map.width, map.height), (350, 350));
        assert!(map.walkable_count() > 1000);
    }
}
