# Phase 3 — Cross-Platform Client

**Goal:** Replace the Windows-only DirectX client with a Bevy (Rust) client that
runs on Windows, Linux, and macOS. Same visual quality and assets; different
runtime foundation.

**Branch:** `phase/3-client-modernization` / `claude/codebase-review-rust-migration-SiWCK`

---

## 3.1 — Bevy Proof-of-Concept

**Status:** [x] Complete — `rust/client/` crate

### Tasks
- [x] Scaffold `rust/client/` Bevy crate in the Rust workspace
- [x] Implement `MapFile` — deserializes `.map.json` files (JSON format from Tasks 1.9/1.10)
- [x] Implement `MapRenderPlugin` — spawns one `Sprite` per tile, centred on origin
- [x] Window title shows map dimensions; camera auto-positioned
- [x] Feature-gated: `default = ["windowed"]`; CI builds with `--no-default-features`
  (Bevy optional dep — no system-graphics libs needed in CI)
- [x] 8 unit tests for `MapFile` (parsing, walkability, bounds, error cases)
- [x] CI updated: separate headless build step for client crate

### Crate structure

```
rust/client/
├── Cargo.toml          — bevy = "0.15" optional, windowed feature gate
└── src/
    ├── main.rs         — App entry: loads map path from argv, runs Bevy
    ├── map.rs          — MapFile struct (Deserialize) + walkability helpers + tests
    └── render.rs       — MapRenderPlugin: Camera2d + tile Sprite spawning
```

### Usage

```bash
# Developer: full windowed build (requires system graphics libs)
cargo run -p zircon-client -- Map/my_map.map.json

# CI / headless: no graphics libs needed
cargo build -p zircon-client --no-default-features
cargo test  -p zircon-client --no-default-features
```

### Rendering design

- Tile size: 16 × 16 px (constant `TILE_PX`)
- Walkable cell: `Color::srgb(0.85, 0.85, 0.85)` (light grey)
- Blocked cell: `Color::srgb(0.15, 0.15, 0.2)` (near-black)
- Rendered area capped at 100 × 100 tiles (`MAX_RENDER_DIM`) for PoC performance
- Grid centred on world origin; camera at origin looks at the grid

---

## 3.2 — Sprite Rendering Pipeline

**Status:** [ ] Not started

Load `.lib` art assets in Bevy and render them in place of the coloured
placeholder tiles.

---

## 3.3 — Player Movement and Camera

**Status:** [ ] Not started

---

## 3.4 — Network Client in Rust

**Status:** [ ] Not started

Connect Bevy client to the Rust server using the `zircon-protocol` crate.

---

## 3.5 — Port UI Layer

**Status:** [ ] Not started

---

## 3.6 — Port Game Scenes

**Status:** [ ] Not started

---

## 3.7 — Platform Validation

**Status:** [ ] Not started

---

## 3.8 — Tauri Admin Tools

**Status:** [ ] Not started
