//! CPU decoders for DXT1 (BC1) and DXT5 (BC3) block-compressed images.
//!
//! The `.Zl` libraries store every sprite as raw DXT blocks. Decoding on the CPU
//! keeps the GPU side format-agnostic (plain RGBA8 textures everywhere) and the
//! sprites are small enough that this is far from a bottleneck.

/// Decode a 5:6:5 packed colour into (r, g, b) 8-bit components.
#[inline]
fn rgb565(c: u16) -> [u8; 3] {
    let r = ((c >> 11) & 0x1F) as u32;
    let g = ((c >> 5) & 0x3F) as u32;
    let b = (c & 0x1F) as u32;
    [
        ((r * 255 + 15) / 31) as u8,
        ((g * 255 + 31) / 63) as u8,
        ((b * 255 + 15) / 31) as u8,
    ]
}

/// Decode one 8-byte DXT1 colour block into 16 RGBA pixels (row-major 4x4).
fn decode_bc1_block(block: &[u8], out: &mut [[u8; 4]; 16]) {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let p0 = rgb565(c0);
    let p1 = rgb565(c1);
    let mut palette = [[0u8; 4]; 4];
    palette[0] = [p0[0], p0[1], p0[2], 255];
    palette[1] = [p1[0], p1[1], p1[2], 255];
    if c0 > c1 {
        for i in 0..3 {
            palette[2][i] = ((2 * p0[i] as u32 + p1[i] as u32) / 3) as u8;
            palette[3][i] = ((p0[i] as u32 + 2 * p1[i] as u32) / 3) as u8;
        }
        palette[2][3] = 255;
        palette[3][3] = 255;
    } else {
        for i in 0..3 {
            palette[2][i] = ((p0[i] as u32 + p1[i] as u32) / 2) as u8;
        }
        palette[2][3] = 255;
        palette[3] = [0, 0, 0, 0]; // 1-bit transparency
    }
    let indices = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
    for (i, px) in out.iter_mut().enumerate() {
        let idx = ((indices >> (2 * i)) & 0x3) as usize;
        *px = palette[idx];
    }
}

/// Decode one 16-byte DXT5 block (8 bytes alpha + 8 bytes colour).
fn decode_bc3_block(block: &[u8], out: &mut [[u8; 4]; 16]) {
    decode_bc1_block(&block[8..16], out);
    // Colour palette in BC3 is always the 4-colour mode regardless of c0/c1 order.
    // Re-derive colours in 4-colour mode to be exact.
    let c0 = u16::from_le_bytes([block[8], block[9]]);
    let c1 = u16::from_le_bytes([block[10], block[11]]);
    if c0 <= c1 {
        let p0 = rgb565(c0);
        let p1 = rgb565(c1);
        let mut palette = [[0u8; 3]; 4];
        palette[0] = p0;
        palette[1] = p1;
        for i in 0..3 {
            palette[2][i] = ((2 * p0[i] as u32 + p1[i] as u32) / 3) as u8;
            palette[3][i] = ((p0[i] as u32 + 2 * p1[i] as u32) / 3) as u8;
        }
        let indices = u32::from_le_bytes([block[12], block[13], block[14], block[15]]);
        for (i, px) in out.iter_mut().enumerate() {
            let idx = ((indices >> (2 * i)) & 0x3) as usize;
            px[0] = palette[idx][0];
            px[1] = palette[idx][1];
            px[2] = palette[idx][2];
        }
    }

    let a0 = block[0] as u32;
    let a1 = block[1] as u32;
    let mut alphas = [0u8; 8];
    alphas[0] = a0 as u8;
    alphas[1] = a1 as u8;
    if a0 > a1 {
        for i in 1..7 {
            alphas[i + 1] = (((7 - i as u32) * a0 + i as u32 * a1) / 7) as u8;
        }
    } else {
        for i in 1..5 {
            alphas[i + 1] = (((5 - i as u32) * a0 + i as u32 * a1) / 5) as u8;
        }
        alphas[6] = 0;
        alphas[7] = 255;
    }
    // 48 bits of 3-bit indices.
    let mut bits: u64 = 0;
    for i in 0..6 {
        bits |= (block[2 + i] as u64) << (8 * i);
    }
    for (i, px) in out.iter_mut().enumerate() {
        let idx = ((bits >> (3 * i)) & 0x7) as usize;
        px[3] = alphas[idx];
    }
}

/// Decode a DXT1 or DXT5 image of `width` x `height` pixels (any size; the
/// caller must pass data covering the 4-aligned dimensions) into tightly packed
/// RGBA8. Pixels are in straight (non-premultiplied) alpha.
pub fn decode(data: &[u8], width: u32, height: u32, dxt5: bool) -> Vec<u8> {
    let bw = width.div_ceil(4);
    let bh = height.div_ceil(4);
    let block_size = if dxt5 { 16 } else { 8 };
    let mut out = vec![0u8; (width * height * 4) as usize];
    let mut pixels = [[0u8; 4]; 16];
    for by in 0..bh {
        for bx in 0..bw {
            let offset = ((by * bw + bx) * block_size) as usize;
            if offset + block_size as usize > data.len() {
                return out;
            }
            let block = &data[offset..offset + block_size as usize];
            if dxt5 {
                decode_bc3_block(block, &mut pixels);
            } else {
                decode_bc1_block(block, &mut pixels);
            }
            for py in 0..4 {
                let y = by * 4 + py;
                if y >= height {
                    break;
                }
                for px in 0..4 {
                    let x = bx * 4 + px;
                    if x >= width {
                        break;
                    }
                    let o = ((y * width + x) * 4) as usize;
                    out[o..o + 4].copy_from_slice(&pixels[(py * 4 + px) as usize]);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bc1_solid_block() {
        // c0 = white (0xFFFF), c1 = black, all indices 0 -> white opaque.
        let block = [0xFF, 0xFF, 0x00, 0x00, 0, 0, 0, 0];
        let px = decode(&block, 4, 4, false);
        assert_eq!(&px[0..4], &[255, 255, 255, 255]);
    }

    #[test]
    fn bc1_transparent_index3() {
        // c0 <= c1 => index 3 is transparent.
        let block = [0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let px = decode(&block, 4, 4, false);
        assert_eq!(px[3], 0);
    }

    #[test]
    fn bc3_alpha_block() {
        // alpha0 = 255, alpha1 = 0, all alpha indices 1 -> alpha 0
        let mut block = [0u8; 16];
        block[0] = 255;
        block[1] = 0;
        // indices: 3 bits each all = 1 -> pattern 0b001001001... = 0x49 0x92 0x24 repeating
        block[2] = 0x49;
        block[3] = 0x92;
        block[4] = 0x24;
        block[5] = 0x49;
        block[6] = 0x92;
        block[7] = 0x24;
        block[8] = 0xFF;
        block[9] = 0xFF;
        let px = decode(&block, 4, 4, true);
        assert_eq!(px[3], 0);
        assert_eq!(&px[0..3], &[255, 255, 255]);
    }
}
