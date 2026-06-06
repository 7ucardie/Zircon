# Zircon — Development Roadmap

This roadmap tracks the modernisation of Zircon across four phases:
stabilising the existing C# codebase, improving the content pipeline,
migrating the server to Rust, and modernising the client for
cross-platform support.

Visual/render quality is intentionally unchanged. The goal is better
foundations, not different-looking output.

---

## Phase 0 — Stabilise the C# Codebase
**Branch:** `phase/0-stabilize`

Foundation work. Nothing in later phases is safe without this.

| # | Task | Status |
|---|---|---|
| 0.1 | Add GitHub Actions CI (build all projects on every push) | [ ] |
| 0.2 | Complete Vortice.Windows migration — replace dead SharpDX | [ ] |
| 0.3 | Promote DX11 to default renderer; demote DX9 to legacy/fallback | [ ] |
| 0.4 | Refactor `PlayerObject.cs` (16,900 lines) into partial class files | [ ] |
| 0.5 | Refactor `GameScene.cs` (5,000 lines) into partial class files | [ ] |
| 0.6 | Add async/await to networking and database layers | [ ] |
| 0.7 | Establish first automated test suite (unit tests for LibraryCore) | [ ] |

---

## Phase 1 — Content Pipeline Improvements
**Branch:** `phase/1-content-pipeline`

Make it possible to add creatures, maps, dungeons, NPCs, and timed
events without touching or recompiling C# code.

| # | Task | Status |
|---|---|---|
| 1.1 | Dynamic monster AI registry — replace 400-line `GetMonster()` switch | [ ] |
| 1.2 | Behavior composition flags — `HasPoison`, `Summons`, `Heals`, `Teleports` | [ ] |
| 1.3 | NPC scripting support — Lua or C# scripts for custom NPC logic | [ ] |
| 1.4 | Cron-style scheduled event triggers — `"0 20 * * 6"` = every Saturday 8pm | [ ] |
| 1.5 | Event chaining — actions can fire other events (multi-stage world events) | [ ] |
| 1.6 | Persistent EventLog — survives server restarts (store to DB) | [ ] |
| 1.7 | Dungeon phases — kill boss → unlock next area → spawn loot room | [ ] |
| 1.8 | Dungeon difficulty scaling — Normal/Hard/Nightmare with stat scaling | [ ] |
| 1.9 | JSON/YAML map definitions — replace binary `.map` with human-readable format | [ ] |
| 1.10 | Hot reload for map and content data during development | [ ] |

---

## Phase 2 — Rust Server Migration
**Branch:** `phase/2-rust-server`

Port the server and tooling to Rust. The C# client continues to work
unchanged during the entire migration (wire-compatible protocol).

| # | Task | Status |
|---|---|---|
| 2.1 | Scaffold Rust workspace — `server/`, `protocol/`, `db/` crates | [ ] |
| 2.2 | Port `LibraryCore/Network/` protocol to Rust (`bytes` + `tokio-codec`) | [ ] |
| 2.3 | Port database layer to `sqlx` (async, compile-time checked queries) | [ ] |
| 2.4 | Port `ServerCore` game loop to Rust with Tokio async runtime | [ ] |
| 2.5 | Port `ServerLibrary` — player, monster, map logic to Rust | [ ] |
| 2.6 | Port `Patcher` and `PatchManager` to Rust CLI tools | [ ] |
| 2.7 | Integration test: C# client connects to Rust server successfully | [ ] |
| 2.8 | Performance baseline: benchmark Rust server vs C# server under load | [ ] |

---

## Phase 3 — Cross-Platform Client (Long-term)
**Branch:** `phase/3-client-modernization`

Bevy (Rust game engine) as the client target. Same visual quality,
same assets — adds Linux and macOS support alongside Windows.

| # | Task | Status |
|---|---|---|
| 3.1 | Bevy proof-of-concept — load a `.map` file and render a single map tile | [ ] |
| 3.2 | Sprite rendering pipeline — load `.lib` art assets in Bevy | [ ] |
| 3.3 | Player movement and camera | [ ] |
| 3.4 | Network client in Rust — connect to Rust server from Bevy client | [ ] |
| 3.5 | Port UI layer (HUD, inventory, NPC dialogs) to Bevy UI | [ ] |
| 3.6 | Port game scenes — Login, CharacterSelect, GameScene | [ ] |
| 3.7 | Platform validation — Windows, Linux, macOS | [ ] |
| 3.8 | Replace WinForms admin tools with Tauri (Rust backend, web frontend) | [ ] |

---

## Graphics / API Stack

| Now | Phase 0 | Phase 3 |
|---|---|---|
| DX9 (primary, SharpDX — dead) | DX11 (primary, Vortice) | wgpu (Vulkan / Metal / DX12 / WebGPU) |
| DX11 (secondary, SharpDX) | Vulkan (Silk.NET, secondary) | Bevy wgpu renderer |
| Vulkan (Silk.NET, new) | DX9 (legacy fallback) | — |

Visual output is identical across all stages. API modernisation only.

---

## Platform Support

| Platform | Now | Phase 0 | Phase 3 |
|---|---|---|---|
| Windows | Yes | Yes | Yes |
| Linux | No | No | Yes (Bevy) |
| macOS | No | No | Yes (Bevy) |
| Browser (WASM) | No | No | Possible (Bevy) |
| Android / iOS | No | No | Possible (Bevy) |
