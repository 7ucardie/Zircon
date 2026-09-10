//! `.Zl` sprite library reader (Zircon's `MirLibrary` / `Mir3Library`).
//!
//! Layout (all little-endian):
//! ```text
//! i32 header_size
//! header block:
//!     i32 value      count = version == 0 ? value : value & 0x01FF_FFFF
//!                    version = (value >> 25) & 0x7F
//!     count times:   u8 present; if present: 25-byte image header
//! payload:           per-image DXT blobs at absolute `position`
//! ```
//! Image header: `i32 position, i16 width, i16 height, i16 offset_x, i16 offset_y,
//! u8 shadow_type, i16 shadow_width, i16 shadow_height, i16 shadow_offset_x,
//! i16 shadow_offset_y, i16 overlay_width, i16 overlay_height`.
//!
//! Payload per image is three concatenated DXT surfaces: image, shadow, overlay.
//! Version 0 libraries are DXT1, everything else DXT5. Surface dimensions are
//! rounded up to multiples of 4 for sizing.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::{dxt, Cursor, FormatError, Result};

#[derive(Debug, Clone, Copy, Default)]
pub struct ImageInfo {
    pub position: u32,
    pub width: u16,
    pub height: u16,
    pub offset_x: i16,
    pub offset_y: i16,
    pub shadow_type: u8,
    pub shadow_width: u16,
    pub shadow_height: u16,
    pub shadow_offset_x: i16,
    pub shadow_offset_y: i16,
    pub overlay_width: u16,
    pub overlay_height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceKind {
    Image,
    Shadow,
    Overlay,
}

/// Decoded RGBA8 pixels of one surface.
pub struct DecodedSurface {
    /// Padded (multiple of 4) width actually decoded.
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A lazily-read sprite library. The header is parsed once; pixel data is read
/// from disk on demand (the libraries total several GB).
pub struct ZlLibrary {
    path: PathBuf,
    version: u8,
    images: Vec<Option<ImageInfo>>,
    file: Mutex<File>,
}

impl std::fmt::Debug for ZlLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZlLibrary")
            .field("path", &self.path)
            .field("version", &self.version)
            .field("images", &self.images.len())
            .finish()
    }
}

fn padded(v: u16) -> u32 {
    let v = v as u32;
    v + (4 - v % 4) % 4
}

impl ImageInfo {
    fn surface_size(&self, kind: SurfaceKind, version: u8) -> usize {
        let (w, h) = self.surface_dims(kind);
        if w == 0 || h == 0 {
            return 0;
        }
        let (w, h) = (padded(w) as usize, padded(h) as usize);
        if version > 0 {
            w * h
        } else {
            w * h / 2
        }
    }

    pub fn surface_dims(&self, kind: SurfaceKind) -> (u16, u16) {
        match kind {
            SurfaceKind::Image => (self.width, self.height),
            SurfaceKind::Shadow => (self.shadow_width, self.shadow_height),
            SurfaceKind::Overlay => (self.overlay_width, self.overlay_height),
        }
    }

    pub fn has_surface(&self, kind: SurfaceKind) -> bool {
        let (w, h) = self.surface_dims(kind);
        self.position != 0 && w > 0 && h > 0
    }
}

impl ZlLibrary {
    pub fn open(path: impl AsRef<Path>) -> Result<ZlLibrary> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)?;
        let mut len_buf = [0u8; 4];
        file.read_exact(&mut len_buf)?;
        let header_size = i32::from_le_bytes(len_buf);
        if header_size < 4 {
            return Err(FormatError::Invalid("zl header size"));
        }
        let mut header = vec![0u8; header_size as usize];
        file.read_exact(&mut header)?;
        let mut c = Cursor::new(&header);
        let value = c.i32()?;
        let version = ((value >> 25) & 0x7F) as u8;
        let count = if version == 0 {
            value
        } else {
            value & 0x01FF_FFFF
        };
        if count < 0 {
            return Err(FormatError::Invalid("zl image count"));
        }
        let mut images = Vec::with_capacity(count as usize);
        for _ in 0..count {
            if !c.bool()? {
                images.push(None);
                continue;
            }
            let info = ImageInfo {
                position: c.i32()? as u32,
                width: c.i16()? as u16,
                height: c.i16()? as u16,
                offset_x: c.i16()?,
                offset_y: c.i16()?,
                shadow_type: c.u8()?,
                shadow_width: c.i16()? as u16,
                shadow_height: c.i16()? as u16,
                shadow_offset_x: c.i16()?,
                shadow_offset_y: c.i16()?,
                overlay_width: c.i16()? as u16,
                overlay_height: c.i16()? as u16,
            };
            images.push(Some(info));
        }
        Ok(ZlLibrary {
            path,
            version,
            images,
            file: Mutex::new(file),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn version(&self) -> u8 {
        self.version
    }

    pub fn is_dxt5(&self) -> bool {
        self.version > 0
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }

    pub fn info(&self, index: usize) -> Option<&ImageInfo> {
        self.images.get(index).and_then(|i| i.as_ref())
    }

    /// Read and decode one surface of one image to RGBA8. Returns `None` when
    /// the image or surface does not exist.
    pub fn decode(&self, index: usize, kind: SurfaceKind) -> Result<Option<DecodedSurface>> {
        let Some(info) = self.info(index) else {
            return Ok(None);
        };
        if !info.has_surface(kind) {
            return Ok(None);
        }
        let image_size = info.surface_size(SurfaceKind::Image, self.version);
        let shadow_size = info.surface_size(SurfaceKind::Shadow, self.version);
        let (offset, size) = match kind {
            SurfaceKind::Image => (0, image_size),
            SurfaceKind::Shadow => (image_size, shadow_size),
            SurfaceKind::Overlay => (
                image_size + shadow_size,
                info.surface_size(SurfaceKind::Overlay, self.version),
            ),
        };
        let mut raw = vec![0u8; size];
        {
            let mut file = self.file.lock().unwrap();
            file.seek(SeekFrom::Start(info.position as u64 + offset as u64))?;
            file.read_exact(&mut raw)?;
        }
        let (w, h) = info.surface_dims(kind);
        let (pw, ph) = (padded(w), padded(h));
        let rgba = dxt::decode(&raw, pw, ph, self.is_dxt5());
        Ok(Some(DecodedSurface {
            width: pw,
            height: ph,
            rgba,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assets() -> Option<PathBuf> {
        let p = std::env::var_os("ZIRCON_ASSETS").map(PathBuf::from)?;
        p.join("Data").exists().then_some(p)
    }

    #[test]
    fn reads_ground_library() {
        let Some(assets) = assets() else {
            eprintln!("ZIRCON_ASSETS not set; skipping");
            return;
        };
        let lib = ZlLibrary::open(assets.join("Data/Ground.Zl")).unwrap();
        assert!(!lib.is_empty());
        let first = (0..lib.len()).find(|&i| lib.info(i).is_some()).unwrap();
        let img = lib.decode(first, SurfaceKind::Image).unwrap().unwrap();
        assert_eq!(img.rgba.len(), (img.width * img.height * 4) as usize);
    }
}
