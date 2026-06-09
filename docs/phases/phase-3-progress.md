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

---

## Task 3.3 — Player Movement and Camera ✓

**Goal**: WASD/arrow-key grid movement with the camera following the player.

### `PlayerPlugin` (render.rs)

Added alongside `MapRenderPlugin` in the map-rendering path.

- `spawn_player`: finds the first walkable tile, spawns a blue `Sprite` at z=1
- `handle_movement`: reads WASD/arrow input; moves immediately on key-down,
  then repeats every 150ms while held; rejects moves into blocked tiles
- `camera_follow`: snaps `Camera2d` position to the player each frame
- `grid_to_world`: shared helper converting (gx, gy) to Bevy world coordinates

### Controls

| Key | Action |
|-----|--------|
| W / ↑ | Move up |
| S / ↓ | Move down |
| A / ← | Move left |
| D / → | Move right |

---

---

## Task 3.4 — Network Client ✓

**Goal**: Connect the Bevy client to the Rust server using the `zircon-protocol` crate.

### Architecture

```
Bevy systems  ← NetEvent ─ NetworkHandle::poll()
     │                           │
     └── handle.send(NetOut) ────┤
                                 │
                          background thread (Tokio rt)
                                 │
                             TCP socket
                                 │
                           Zircon server
```

### `rust/client/src/network.rs`

- `NetIn` — inbound events: `Connected`, `GoodVersion`, `Ping`, `CheckVersion`, `Disconnect`, `Error`
- `NetOut` — outbound commands: `SendVersion`, `SendPingResponse`
- `ConnectionStatus` — `Connecting` → `Connected` → `Ready` / `Disconnected`
- `NetworkHandle` resource — wraps `Mutex<UnboundedReceiver>` + `UnboundedSender` + status
- `start_network(addr)` — spawns background Tokio thread, returns handle
- `dispatch_frame(frame, tx)` — pure packet dispatch; returns reply bytes when needed:
  - `Connected` (id=1) → auto-send `Version { client_hash: [] }`
  - `Ping` (id=4) → auto-send `PingResponse { ping: 0 }`
  - `GoodVersion` (id=3) / `Disconnect` (id=2) / `CheckVersion` (id=0) → event only
- `NetworkPlugin` (windowed feature) — inserts handle, adds `poll_network` Update system

### CLI usage

```
zircon-client <map.json> [server_addr]
# e.g.
zircon-client Map/zone1.json 127.0.0.1:7000
```

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
---

## Task 3.5 — Bevy UI Layer ✓

**Goal**: HUD bars, inventory grid, NPC dialog panel, and connection status overlay.

### `rust/client/src/ui.rs` (windowed feature)

`UiPlugin` adds four systems:

| System | Trigger | Effect |
|--------|---------|--------|
| `setup_ui` | Startup | Spawns all panels |
| `update_status_hud` | Update | Reads `NetworkHandle.status` → updates top-right text |
| `update_bars` | Update (on stat change) | Adjusts HP/MP fill widths + text |
| `toggle_inventory` | Update | `I` key shows/hides inventory grid |
| `toggle_dialog` | Update | `E` key shows/hides NPC dialog |

### UI panels

- **Status overlay** (top-right): single-line connection status
- **HUD bar** (bottom-left): HP (red) and MP (blue) bars, each 160×16px with fill + text overlay
- **Inventory** (centre, hidden): 5×6 CSS-grid of 28px slots with border
- **NPC dialog** (above HUD, hidden): speaker name (gold), body text, close hint

### Pure helpers (always compiled, in `network.rs`)

`bar_pct(current, max)` and `status_label(status)` live in `network.rs` so they
compile and are tested in headless/CI builds (no Bevy required).

### Controls summary

| Key | Action |
|-----|--------|
| WASD / Arrows | Move player |
| I | Toggle inventory |
| E | Toggle NPC dialog |

---

- Network: `Connected` frame → Version reply (id=6)
- Network: `Ping` frame → PingResponse reply (id=5)
- Network: `Disconnect` frame → event with correct reason
- Network: `GoodVersion` frame → event with database_key
- Network: unknown packet silently ignored
- Network: outbound `Version` and `PingResponse` encode correctly

---

## Task 3.6 — Game Scenes ✓

**Goal**: Port Login, CharacterSelect, and Game state machine to Bevy States.

### Architecture

`game_state.rs` (always compiled — no Bevy deps):
- `GameScene` enum: `Login` (default), `CharacterSelect`, `Game`
- `#[cfg_attr(feature = "windowed", derive(bevy::prelude::States))]`
- `scene_title(scene)` pure helper — tested in CI headless builds

`scenes.rs` (windowed feature only):
- `ScenesPlugin`: registers `GameScene` state, spawns overlay panels, wires transitions
- `LoginPanel` / `CharSelectPanel` marker components for visibility toggling

### Panel layout

Both panels are full-screen `GlobalZIndex(10)` overlays with `BackgroundColor(0.04, 0.04, 0.08, 0.97)`. Initial visibility: Login=Visible, CharSelect=Hidden.

**Login panel**: title "ZIRCON", subtitle "Cross-platform Mir3 Client", hint "[Enter] Connect to server"

**CharSelect panel**: "Select Character" heading, three character slots (Warrior/Wizard/empty) in bordered boxes, navigation hint "[1/2/3] or [Enter] Play  [Esc] Back"

### Transitions

| From            | To              | Trigger                              |
|-----------------|-----------------|--------------------------------------|
| Login           | CharacterSelect | Enter / NumpadEnter key              |
| Login           | CharacterSelect | `NetworkHandle.status == Ready`      |
| CharacterSelect | Game            | Enter / 1 / 2 / 3 keys              |
| CharacterSelect | Login           | Escape key                           |
| Any (not Login) | Login           | `NetworkHandle.status == Disconnected` |

### Test coverage

4 tests in `game_state::tests` (always compiled, run in CI headless builds):
- `default_scene_is_login`
- `scene_titles_are_distinct`
- `scene_title_values`
- `game_scene_eq_and_hash`

---

## Task 3.7 — Platform Validation ✓

**Goal**: Verify the Bevy client builds (headless + windowed) and all tests pass on Windows, Linux, and macOS.

### CI changes (`.github/workflows/build.yml`)

New `build-rust-client` matrix job running on all three OS targets:

```yaml
build-rust-client:
  name: Rust Client (${{ matrix.os }})
  runs-on: ${{ matrix.os }}
  strategy:
    matrix:
      os: [ubuntu-latest, windows-latest, macos-latest]
```

Steps per platform:
1. **Install Linux system libraries** (ubuntu only) — ALSA, udev, xkbcommon, X11, Wayland headers required for Bevy's default feature set to link on Linux CI
2. **Build client (headless)** — `cargo build -p zircon-client --no-default-features`
3. **Build client (windowed)** — `cargo build -p zircon-client` — full Bevy compile validates platform ABI
4. **Test client** — `cargo test -p zircon-client --no-default-features` (41 tests)

### Platform-specific notes

| Platform | Graphics backend | System libraries required |
|----------|-----------------|--------------------------|
| Linux    | wgpu Vulkan / GLES | `libx11-dev libasound2-dev libudev-dev libxkbcommon-dev libwayland-dev` |
| Windows  | wgpu DX12 / DX11 | None (MSVC runtime included in runner) |
| macOS    | wgpu Metal       | None (Metal SDK included in Xcode toolchain) |

### Existing `build-rust` job (ubuntu-only)

Retained separately: builds server crates + client headless + all workspace tests.
The new matrix job adds the per-platform windowed compile check on top of that.
