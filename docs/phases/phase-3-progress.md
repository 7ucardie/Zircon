# Phase 3 — Cross-Platform Client Progress

## Task 3.1 — Bevy Proof-of-Concept ✓

**Goal**: Load a `.map` file (JSON format from Task 1.9) and render a tile grid.

### Crate structure

```
rust/client/
├── Cargo.toml          # zircon-client, bevy optional via `windowed` feature
└── src/
    ├── main.rs         # entry point; args: <map.json> or --lib <file.zl> [idx]
    ├── map.rs          # MapFile loader + unit tests
    ├── render.rs       # Bevy MapRenderPlugin + LibRenderPlugin
    └── lib_asset.rs    # .zl decoder (see Task 3.2)
```

### Feature gates

`windowed` (default) pulls in `bevy`. CI builds with `--no-default-features` to
avoid system graphics library requirements (X11, Wayland, ALSA) on headless runners.

### CI changes (`.github/workflows/build.yml`)

```yaml
- name: Build server crates
  run: cargo build --workspace --exclude zircon-client

- name: Build client (headless)
  run: cargo build -p zircon-client --no-default-features

- name: Test
  run: cargo test --all
```

### Map tile rendering

`MapRenderPlugin` spawns one `Sprite` per tile:
- walkable (`.`) → light grey (`0.85, 0.85, 0.85`)
- blocked (`#`) → near-black (`0.15, 0.15, 0.2`)
- `TILE_PX = 16`, grid centred on origin, capped at `MAX_RENDER_DIM = 100`

---

## Task 3.2 — Sprite Rendering Pipeline ✓

**Goal**: Load Mir3 `.zl` (Library) art asset files and render decoded sprites in Bevy.

### File format (`lib_asset.rs`)

```
[4B headerSize][metadata blob][pixel data]
metadata: [4B packed: bits 0-24 = count, bits 25-31 = version]
          [26B × count: 1B enabled + 25B header]
header: position(u32), w/h/ox/oy(i16×4), shadowType(u8),
        shadowW/H/OX/OY(i16×4), overlayW/H(i16×2)
```

### Pixel encoding

| Version | Format | Bytes per aligned block |
|---------|--------|------------------------|
| 0       | BC1 / DXT1 | `aligned_w × aligned_h / 2` |
| 1       | BC3 / DXT5 | `aligned_w × aligned_h` |

`aligned = n + (4 - n%4) % 4`

### Decoders

Pure-Rust BC1 and BC3 decoders; no external image crates required.

- `decode_bc1` / `decode_bc1_block` — 4-colour opaque + 1-bit transparent mode
- `decode_bc3` / `decode_bc3_block` — 8-bit alpha block + opaque colour block
- `decode_rgb565` — 5/6/5 → R8G8B8 with high-bit replication
- `crop_rgba` — strips DXT alignment padding to true pixel dimensions

### Bevy integration (`render.rs — LibRenderPlugin`)

```rust
pub struct LibRenderPlugin {
    pub lib: LibFile,
    pub image_index: usize,
}
```

Decodes the requested image on startup, uploads as `Rgba8UnormSrgb` `Image` asset,
and spawns a `Sprite` with `offset_x / offset_y` applied.

### Test coverage

All tests pass (`cargo test -p zircon-client --no-default-features`):
- RGB565 decode (full red/green/blue/black/white)
- `align4` (already aligned, rounds up, zero)
- BC1 opaque block, BC1 transparent mode
- BC3 alpha (8-value and 6-value modes)
- File parse + image decode (solid red 4×4)
- Out-of-bounds decode returns `None`
- Map parse/validation, walkability, edge cases
- Invalid JSON, missing file, wrong dimensions
