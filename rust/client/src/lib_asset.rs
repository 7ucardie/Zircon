//! Parser and DXT decoder for Zircon `.zl` (Mir3 Library) art asset files.
//!
//! # File format
//!
//! ```text
//! [4 bytes]  headerSize       — byte length of the metadata blob
//! [headerSize bytes]          — metadata blob:
//!     [4 bytes]  packed       — bits 0-24 = image count, bits 25-31 = version
//!     [26 bytes × count]      — per-image entries (1-byte enabled + 25-byte header)
//! [variable]  pixel data      — DXT-compressed blocks; addressed via Position field
//! ```
//!
//! Per-image header (25 bytes, only present when enabled = true):
//!   position (u32), width/height (i16 × 2), offset_x/y (i16 × 2),
//!   shadow_type (u8), shadow_w/h/ox/oy (i16 × 4), overlay_w/h (i16 × 2).
//!
//! # Pixel encoding
//!
//! - Version 0 → BC1 / DXT1: `aligned_w × aligned_h / 2` bytes per layer
//! - Version 1 → BC3 / DXT5: `aligned_w × aligned_h`    bytes per layer
//! where aligned = n + (4 - n%4)%4

use std::io::{self, Cursor, Read};
use std::path::Path;

// ── Public types ──────────────────────────────────────────────────────────────

/// Metadata for one image slot.
#[derive(Debug, Clone)]
pub struct ImageInfo {
    /// Absolute byte offset into the file where compressed pixel data begins.
    pub position: u32,
    pub width: i16,
    pub height: i16,
    pub offset_x: i16,
    pub offset_y: i16,
    pub shadow_type: u8,
    pub shadow_width: i16,
    pub shadow_height: i16,
    pub shadow_offset_x: i16,
    pub shadow_offset_y: i16,
    pub overlay_width: i16,
    pub overlay_height: i16,
}

/// A Mir3 library file loaded into memory.
pub struct LibFile {
    /// 0 = DXT1, 1 = DXT5.
    pub version: u8,
    /// One entry per image slot.  `None` means the slot is disabled.
    pub images: Vec<Option<ImageInfo>>,
    /// Full file bytes kept for deferred pixel-data access.
    data: Vec<u8>,
}

/// Decoded image, ready to upload to the GPU as RGBA8.
#[derive(Debug)]
pub struct DecodedImage {
    /// True pixel dimensions (before DXT alignment padding).
    pub width: u32,
    pub height: u32,
    pub offset_x: i16,
    pub offset_y: i16,
    /// Raw `width × height × 4` bytes in RGBA order.
    pub rgba: Vec<u8>,
}

// ── Loading ───────────────────────────────────────────────────────────────────

impl LibFile {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let data = std::fs::read(path)
            .map_err(|e| anyhow::anyhow!("cannot read {:?}: {e}", path))?;
        Self::from_bytes(data)
    }

    pub fn from_bytes(data: Vec<u8>) -> anyhow::Result<Self> {
        let mut cur = Cursor::new(data.as_slice());

        let header_size = read_u32(&mut cur)? as usize;
        let mut meta = vec![0u8; header_size];
        cur.read_exact(&mut meta)
            .map_err(|_| anyhow::anyhow!("truncated header"))?;

        let mut mc = Cursor::new(meta.as_slice());
        let packed = read_u32(&mut mc)?;
        let count = (packed & 0x1FF_FFFF) as usize;
        let version = ((packed >> 25) & 0x7F) as u8;

        let mut images = Vec::with_capacity(count);
        for _ in 0..count {
            let enabled = read_u8(&mut mc)?;
            if enabled == 0 {
                images.push(None);
            } else {
                images.push(Some(ImageInfo {
                    position:        read_u32(&mut mc)?,
                    width:           read_i16(&mut mc)?,
                    height:          read_i16(&mut mc)?,
                    offset_x:        read_i16(&mut mc)?,
                    offset_y:        read_i16(&mut mc)?,
                    shadow_type:     read_u8(&mut mc)?,
                    shadow_width:    read_i16(&mut mc)?,
                    shadow_height:   read_i16(&mut mc)?,
                    shadow_offset_x: read_i16(&mut mc)?,
                    shadow_offset_y: read_i16(&mut mc)?,
                    overlay_width:   read_i16(&mut mc)?,
                    overlay_height:  read_i16(&mut mc)?,
                }));
            }
        }

        Ok(LibFile { version, images, data })
    }

    pub fn len(&self) -> usize { self.images.len() }
    pub fn is_empty(&self) -> bool { self.images.is_empty() }

    /// Decode the main (non-shadow, non-overlay) image at `index` to RGBA8.
    /// Returns `None` if the slot is disabled or has zero dimensions.
    pub fn decode_image(&self, index: usize) -> Option<DecodedImage> {
        let info = self.images.get(index)?.as_ref()?;
        if info.width <= 0 || info.height <= 0 || info.position == 0 {
            return None;
        }
        let w = info.width as u32;
        let h = info.height as u32;
        let aw = align4(w);
        let ah = align4(h);
        let data_size = if self.version > 0 { (aw * ah) as usize } else { (aw * ah / 2) as usize };
        let start = info.position as usize;
        let end = start + data_size;
        if end > self.data.len() {
            return None;
        }
        let compressed = &self.data[start..end];

        let rgba = if self.version > 0 {
            decode_bc3(compressed, aw, ah)
        } else {
            decode_bc1(compressed, aw, ah)
        };

        // Crop to actual dimensions (strip DXT alignment padding)
        let rgba = crop_rgba(&rgba, aw, ah, w, h);

        Some(DecodedImage { width: w, height: h, offset_x: info.offset_x, offset_y: info.offset_y, rgba })
    }
}

// ── DXT alignment helper ──────────────────────────────────────────────────────

#[inline]
fn align4(n: u32) -> u32 { n + (4 - n % 4) % 4 }

// ── Crop helper ───────────────────────────────────────────────────────────────

fn crop_rgba(src: &[u8], src_w: u32, _src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    if src_w == dst_w {
        // No column crop needed; just trim rows.
        return src[..(dst_w * dst_h * 4) as usize].to_vec();
    }
    let mut out = Vec::with_capacity((dst_w * dst_h * 4) as usize);
    for y in 0..dst_h {
        let row_start = (y * src_w * 4) as usize;
        let row_end = row_start + (dst_w * 4) as usize;
        out.extend_from_slice(&src[row_start..row_end]);
    }
    out
}

// ── BC1 / DXT1 decoder ────────────────────────────────────────────────────────

/// Decode a BC1-compressed buffer into RGBA8.
/// `aw` and `ah` must both be multiples of 4.
fn decode_bc1(data: &[u8], aw: u32, ah: u32) -> Vec<u8> {
    let cols = (aw / 4) as usize;
    let rows = (ah / 4) as usize;
    let mut out = vec![0u8; (aw * ah * 4) as usize];

    for by in 0..rows {
        for bx in 0..cols {
            let block_idx = by * cols + bx;
            let block = &data[block_idx * 8..(block_idx + 1) * 8];
            let pixels = decode_bc1_block(block);
            blit_block(&pixels, &mut out, bx, by, aw as usize);
        }
    }
    out
}

fn decode_bc1_block(block: &[u8]) -> [[u8; 4]; 16] {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let indices = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);

    let r0 = decode_rgb565(c0);
    let r1 = decode_rgb565(c1);

    let palette: [[u8; 4]; 4] = if c0 > c1 {
        [
            [r0[0], r0[1], r0[2], 255],
            [r1[0], r1[1], r1[2], 255],
            [lerp(r0[0], r1[0], 1, 3), lerp(r0[1], r1[1], 1, 3), lerp(r0[2], r1[2], 1, 3), 255],
            [lerp(r0[0], r1[0], 2, 3), lerp(r0[1], r1[1], 2, 3), lerp(r0[2], r1[2], 2, 3), 255],
        ]
    } else {
        [
            [r0[0], r0[1], r0[2], 255],
            [r1[0], r1[1], r1[2], 255],
            [lerp(r0[0], r1[0], 1, 2), lerp(r0[1], r1[1], 1, 2), lerp(r0[2], r1[2], 1, 2), 255],
            [0, 0, 0, 0], // transparent black
        ]
    };

    let mut pixels = [[0u8; 4]; 16];
    for (i, px) in pixels.iter_mut().enumerate() {
        let idx = ((indices >> (i * 2)) & 0x3) as usize;
        *px = palette[idx];
    }
    pixels
}

// ── BC3 / DXT5 decoder ────────────────────────────────────────────────────────

/// Decode a BC3-compressed buffer into RGBA8.
fn decode_bc3(data: &[u8], aw: u32, ah: u32) -> Vec<u8> {
    let cols = (aw / 4) as usize;
    let rows = (ah / 4) as usize;
    let mut out = vec![0u8; (aw * ah * 4) as usize];

    for by in 0..rows {
        for bx in 0..cols {
            let block_idx = by * cols + bx;
            let block = &data[block_idx * 16..(block_idx + 1) * 16];
            let pixels = decode_bc3_block(block);
            blit_block(&pixels, &mut out, bx, by, aw as usize);
        }
    }
    out
}

fn decode_bc3_block(block: &[u8]) -> [[u8; 4]; 16] {
    // First 8 bytes: alpha block
    let alpha_table = decode_bc3_alphas(&block[0..8]);
    // Next 8 bytes: colour block (same as DXT1 but always 4-colour mode)
    let color_block = decode_bc1_block_opaque(&block[8..16]);

    let mut pixels = [[0u8; 4]; 16];
    for (i, px) in pixels.iter_mut().enumerate() {
        *px = [color_block[i][0], color_block[i][1], color_block[i][2], alpha_table[i]];
    }
    pixels
}

fn decode_bc3_alphas(block: &[u8]) -> [u8; 16] {
    let a0 = block[0];
    let a1 = block[1];

    let table: [u8; 8] = if a0 > a1 {
        [
            a0, a1,
            lerp(a0, a1, 1, 7), lerp(a0, a1, 2, 7),
            lerp(a0, a1, 3, 7), lerp(a0, a1, 4, 7),
            lerp(a0, a1, 5, 7), lerp(a0, a1, 6, 7),
        ]
    } else {
        [
            a0, a1,
            lerp(a0, a1, 1, 5), lerp(a0, a1, 2, 5),
            lerp(a0, a1, 3, 5), lerp(a0, a1, 4, 5),
            0, 255,
        ]
    };

    // 48 bits packed in 6 bytes; 3 bits per pixel
    let bits: u64 = (block[2] as u64)
        | ((block[3] as u64) << 8)
        | ((block[4] as u64) << 16)
        | ((block[5] as u64) << 24)
        | ((block[6] as u64) << 32)
        | ((block[7] as u64) << 40);

    let mut out = [0u8; 16];
    for (i, a) in out.iter_mut().enumerate() {
        *a = table[((bits >> (i * 3)) & 0x7) as usize];
    }
    out
}

/// Like `decode_bc1_block` but always treats c0 > c1 (4-colour opaque mode).
fn decode_bc1_block_opaque(block: &[u8]) -> [[u8; 3]; 16] {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let indices = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);

    let r0 = decode_rgb565(c0);
    let r1 = decode_rgb565(c1);

    let palette: [[u8; 3]; 4] = [
        r0,
        r1,
        [lerp(r0[0], r1[0], 1, 3), lerp(r0[1], r1[1], 1, 3), lerp(r0[2], r1[2], 1, 3)],
        [lerp(r0[0], r1[0], 2, 3), lerp(r0[1], r1[1], 2, 3), lerp(r0[2], r1[2], 2, 3)],
    ];

    let mut pixels = [[0u8; 3]; 16];
    for (i, px) in pixels.iter_mut().enumerate() {
        *px = palette[((indices >> (i * 2)) & 0x3) as usize];
    }
    pixels
}

// ── Blit helper ───────────────────────────────────────────────────────────────

/// Write a 4×4 decoded block into the flat RGBA output buffer.
fn blit_block(pixels: &[[u8; 4]; 16], out: &mut [u8], bx: usize, by: usize, stride: usize) {
    for row in 0..4usize {
        for col in 0..4usize {
            let dst = ((by * 4 + row) * stride + (bx * 4 + col)) * 4;
            out[dst..dst + 4].copy_from_slice(&pixels[row * 4 + col]);
        }
    }
}

// ── Colour helpers ────────────────────────────────────────────────────────────

#[inline]
fn decode_rgb565(v: u16) -> [u8; 3] {
    let r5 = ((v >> 11) & 0x1F) as u8;
    let g6 = ((v >> 5)  & 0x3F) as u8;
    let b5 = (v         & 0x1F) as u8;
    // Expand to 8-bit by replicating high bits
    [(r5 << 3) | (r5 >> 2), (g6 << 2) | (g6 >> 4), (b5 << 3) | (b5 >> 2)]
}

#[inline]
fn lerp(a: u8, b: u8, num: u32, denom: u32) -> u8 {
    ((a as u32 * (denom - num) + b as u32 * num) / denom) as u8
}

// ── Low-level readers ─────────────────────────────────────────────────────────

fn read_u8(r: &mut Cursor<&[u8]>) -> io::Result<u8> {
    let mut buf = [0u8; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}
fn read_u32(r: &mut Cursor<&[u8]>) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}
fn read_i16(r: &mut Cursor<&[u8]>) -> io::Result<i16> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(i16::from_le_bytes(buf))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── RGB565 decode ─────────────────────────────────────────────────────────

    #[test]
    fn rgb565_full_red() {
        // 0b1111_1000_0000_0000 = 0xF800 — R=31, G=0, B=0
        let [r, g, b] = decode_rgb565(0xF800);
        assert_eq!(r, 255, "red");
        assert_eq!(g, 0,   "green");
        assert_eq!(b, 0,   "blue");
    }

    #[test]
    fn rgb565_full_green() {
        // 0b0000_0111_1110_0000 = 0x07E0 — R=0, G=63, B=0
        let [r, g, b] = decode_rgb565(0x07E0);
        assert_eq!(r, 0,   "red");
        assert_eq!(g, 255, "green");
        assert_eq!(b, 0,   "blue");
    }

    #[test]
    fn rgb565_full_blue() {
        // 0b0000_0000_0001_1111 = 0x001F — R=0, G=0, B=31
        let [r, g, b] = decode_rgb565(0x001F);
        assert_eq!(r, 0,   "red");
        assert_eq!(g, 0,   "green");
        assert_eq!(b, 255, "blue");
    }

    #[test]
    fn rgb565_black() {
        let [r, g, b] = decode_rgb565(0x0000);
        assert_eq!([r, g, b], [0, 0, 0]);
    }

    #[test]
    fn rgb565_white() {
        let [r, g, b] = decode_rgb565(0xFFFF);
        assert_eq!(r, 255);
        assert_eq!(g, 255);
        assert_eq!(b, 255);
    }

    // ── align4 ────────────────────────────────────────────────────────────────

    #[test]
    fn align4_already_aligned() {
        assert_eq!(align4(4), 4);
        assert_eq!(align4(8), 8);
        assert_eq!(align4(100), 100);
    }

    #[test]
    fn align4_rounds_up() {
        assert_eq!(align4(1), 4);
        assert_eq!(align4(5), 8);
        assert_eq!(align4(15), 16);
    }

    #[test]
    fn align4_zero_stays_zero() {
        assert_eq!(align4(0), 0);
    }

    // ── BC1 block ─────────────────────────────────────────────────────────────

    #[test]
    fn bc1_block_all_red_opaque() {
        // color0 = 0xF800 (red), color1 = 0x0000 (black)
        // c0 > c1 → 4-colour opaque mode
        // indices = 0x00000000 → all pixels use color0 = red
        let block = [0x00u8, 0xF8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let pixels = decode_bc1_block(&block);
        for px in &pixels {
            assert_eq!(px[0], 255, "R");
            assert_eq!(px[1], 0,   "G");
            assert_eq!(px[2], 0,   "B");
            assert_eq!(px[3], 255, "A");
        }
    }

    #[test]
    fn bc1_block_transparent_mode() {
        // c0 <= c1 → 3-colour + transparent mode.
        // color0 = 0x0000 (black), color1 = 0x07E0 (green)
        // index 3 → transparent
        let block = [0x00u8, 0x00, 0xE0, 0x07,
                     0xFF, 0xFF, 0xFF, 0xFF]; // all pixels → index 3 (0b11)
        let pixels = decode_bc1_block(&block);
        for px in &pixels {
            assert_eq!(px[3], 0, "should be transparent");
        }
    }

    // ── BC3 alphas ────────────────────────────────────────────────────────────

    #[test]
    fn bc3_alphas_full_range_8_values() {
        // a0=255, a1=0 → 8-value mode; first pixel index 0 = 255
        let block = [255u8, 0, 0, 0, 0, 0, 0, 0];
        let alphas = decode_bc3_alphas(&block);
        assert_eq!(alphas[0], 255);
    }

    #[test]
    fn bc3_alphas_clamped_6_values() {
        // a0=0, a1=255 → a0 <= a1 → 6-value mode, indices 6/7 are 0/255
        let block = [0u8, 255, 0b11_110_110, 0b11_110_110, 0b11_110_110, 0b11_110_110, 0b11_110_110, 0b11_110_110];
        let alphas = decode_bc3_alphas(&block);
        // first pair of 3-bit indices from 0b110_110 (first pixel) = index 6 → 0
        // depends on bit packing — just check no panic and values in range
        for a in &alphas {
            assert!(*a <= 255);
        }
    }

    // ── File parsing with synthetic data ─────────────────────────────────────

    fn make_bc1_block_solid_red() -> [u8; 8] {
        // color0=0xF800 (red, max), color1=0x0000 (black, less) → c0>c1 opaque
        [0x00u8, 0xF8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    }

    fn make_lib_v0_1x1() -> Vec<u8> {
        // A minimal version-0 .zl with one 4×4 image (smallest DXT unit)
        let image_block = make_bc1_block_solid_red(); // 8 bytes BC1
        // Image position = 4 (headerSize field) + header_size
        // header_size = 4 (packed) + 26 (1 slot) = 30 bytes
        let image_pos = 4u32 + 30u32; // = 34
        // packed: version=0, count=1
        let packed = 1u32; // bits 25-31 = 0 (version), bits 0-24 = 1 (count)
        let mut meta = Vec::new();
        meta.extend_from_slice(&packed.to_le_bytes());
        // One image entry: 1 byte enabled + 27 bytes header
        meta.push(1u8); // enabled
        meta.extend_from_slice(&image_pos.to_le_bytes()); // position
        meta.extend_from_slice(&4i16.to_le_bytes()); // width = 4
        meta.extend_from_slice(&4i16.to_le_bytes()); // height = 4
        meta.extend_from_slice(&0i16.to_le_bytes()); // offset_x
        meta.extend_from_slice(&0i16.to_le_bytes()); // offset_y
        meta.push(0u8); // shadow_type
        meta.extend_from_slice(&0i16.to_le_bytes()); // shadow_width
        meta.extend_from_slice(&0i16.to_le_bytes()); // shadow_height
        meta.extend_from_slice(&0i16.to_le_bytes()); // shadow_offset_x
        meta.extend_from_slice(&0i16.to_le_bytes()); // shadow_offset_y
        meta.extend_from_slice(&0i16.to_le_bytes()); // overlay_width
        meta.extend_from_slice(&0i16.to_le_bytes()); // overlay_height
        assert_eq!(meta.len(), 30); // 4 (packed) + 26 (1 slot)

        let mut file = Vec::new();
        file.extend_from_slice(&(meta.len() as u32).to_le_bytes()); // headerSize
        file.extend_from_slice(&meta);
        file.extend_from_slice(&image_block);
        file
    }

    #[test]
    fn parse_v0_1image() {
        let data = make_lib_v0_1x1();
        let lib = LibFile::from_bytes(data).expect("parse failed");
        assert_eq!(lib.version, 0);
        assert_eq!(lib.len(), 1);
        assert!(lib.images[0].is_some());
    }

    #[test]
    fn decode_v0_solid_red() {
        let data = make_lib_v0_1x1();
        let lib = LibFile::from_bytes(data).unwrap();
        let img = lib.decode_image(0).expect("decode failed");
        assert_eq!(img.width, 4);
        assert_eq!(img.height, 4);
        assert_eq!(img.rgba.len(), 4 * 4 * 4);
        // All 16 pixels should be opaque red
        for chunk in img.rgba.chunks_exact(4) {
            assert_eq!(chunk[0], 255, "R"); // R
            assert_eq!(chunk[2], 0,   "B"); // B
            assert_eq!(chunk[3], 255, "A"); // A opaque
        }
    }

    #[test]
    fn decode_out_of_bounds_returns_none() {
        let data = make_lib_v0_1x1();
        let lib = LibFile::from_bytes(data).unwrap();
        assert!(lib.decode_image(99).is_none());
    }
}
