# Phase 2 — Rust Server Migration

**Goal:** Port the server and tooling to Rust. The C# client continues to work
unchanged during the entire migration (wire-compatible protocol).

**Branch:** `claude/codebase-review-rust-migration-SiWCK`

---

## 2.1 — Scaffold Rust Workspace

**Status:** [x] Complete — `rust/` workspace with `protocol`, `db`, `server`, `patchmgr` crates

### Crates

| Crate | Path | Purpose |
|---|---|---|
| `zircon-protocol` | `rust/protocol/` | Wire frame encoding/decoding; packet ID registry |
| `zircon-db` | `rust/db/` | MirDB binary format reader; game data structs |
| `zircon-server` | `rust/server/` | Tokio game loop, network layer, world state |
| `zircon-patchmgr` | `rust/patchmgr/` | Patch manager and patcher CLIs |

### `rust/Cargo.toml` workspace config
- `edition = "2021"`, `version = "0.1.0"`, `license = "MIT"`
- Shared dev-dependency: `criterion = { version = "0.5", default-features = false, features = ["cargo_bench_support"] }`

---

## 2.2 — Port `LibraryCore/Network/` Protocol to Rust

**Status:** [x] Complete — `zircon-protocol` crate

### Wire format
```
[4-byte LE total-length][2-byte LE packet-ID][payload]
```
Total-length includes the 2-byte packet-ID but not itself.

### API
```rust
pub struct RawFrame { pub packet_id: u16, pub payload: Vec<u8> }
impl RawFrame {
    pub fn parse(buf: &mut BytesMut) -> Result<Option<Self>, ProtocolError>
    pub fn encode(&self) -> Bytes
}
```

### Packet registry
- 355 packet IDs ported from C# `Packets.cs`
- `PacketId` enum with `#[repr(u16)]` and `TryFrom<u16>`

---

## 2.3 — Port Database Layer

**Status:** [x] Complete — `zircon-db` crate (MirDB binary format, not SQLite)

### Format
MirDB is a custom binary ORM (not a SQL database). Two databases:
- `System.db` — static game data (MonsterInfo, NPCInfo, MapInfo, ItemInfo, …)
- `Users.db` — player accounts and characters

### Structs ported
- `MonsterInfo`, `NPCInfo`, `MapInfo`, `ItemInfo`
- `UserInfo`, `CharacterInfo`

### API
```rust
pub struct GameData { pub maps: Vec<MapInfo>, pub monsters: Vec<MonsterInfo>, ... }
impl GameData {
    pub fn load(db_path: &Path) -> anyhow::Result<Self>
}
```

---

## 2.4 — Port `ServerCore` Game Loop to Rust

**Status:** [x] Complete — `zircon-server` Tokio async runtime

### Architecture
- Single async task owns all game state — no shared-state concurrency
- mpsc channels for new connections and inbound frames
- 50 ms tick interval (`TICK_MS = 50`)
- `Config` loaded from `config.toml` (TOML via `serde`/`toml`)

### Key files
- `rust/server/src/main.rs` — entry point, config loading
- `rust/server/src/envir/mod.rs` — `Envir` struct, game loop
- `rust/server/src/network/mod.rs` — `bind_listener` / `run_listener` / `start_listener`
- `rust/server/src/config.rs` — `Config` struct

---

## 2.5 — Port `ServerLibrary` — Player, Monster, Map Logic

**Status:** [x] Complete — `objects/`, `map/`, `envir/world.rs`

### Map loading
Binary `.map` loader + JSON `.map.json` loader, matching C# `Map.Load()` exactly.

Cell layout:
- Offset: `28 + width * height / 4 * 3` (skips embedded mini-map thumbnail)
- 14 bytes/cell; walkable = `(flag & 0x03) == 0x03`
- Flat column-major array: `index = x * height + y`

### Object model
```rust
pub enum MapObject { Player(PlayerObject), Monster(MonsterObject), Npc(NpcObject) }
```
All variants implement `object_id()`, `map_index()`, `x()`, `y()`, `next_tick()`, `process()`.

### World state
```rust
pub struct World {
    pub maps: HashMap<i32, GameMap>,
    pub objects: HashMap<ObjectId, MapObject>,
    pub active_queue: Vec<ObjectId>,
}
```

---

## 2.6 — Port `Patcher` and `PatchManager` to Rust CLI Tools

**Status:** [x] Complete — `zircon-patchmgr` and `zircon-patcher` binaries

### `zircon-patchmgr build`
```
zircon-patchmgr build <client_dir> [--existing <PList.Bin>] [--patch-dir Patch] [--out PList.Bin]
```
- Parallel MD5 scan via rayon + walkdir
- Diff against existing PList.Bin — skip unchanged files (checksum match)
- Compress changed files with flate2 GzEncoder (level 6, matching C# default)
- Write new PList.Bin

### PList.Bin format
- No leading count; read until EOF
- Per entry: 7-bit BinaryWriter string (C# compat) + i64 LE compressed_len + i32 LE checksum_len + 16 MD5 bytes

### `zircon-patcher`
Self-update binary: sleep 2s → remove old binary → rename new → spawn new process.

---

## 2.7 — Integration Tests: C# Client Protocol Compatibility

**Status:** [x] Complete — `rust/server/src/tests.rs`

### Test suite (8 tests)
- `wire_encode_decode_empty` — empty payload round-trip
- `wire_encode_decode_with_payload` — 4-byte payload round-trip
- `server_accepts_connection` — TCP connect succeeds
- `server_handles_multiple_connections` — 3 concurrent connections
- `server_echo_unknown_packet` — unrecognized packet ID survives gracefully
- `server_handles_disconnect` — clean disconnect without panic
- `server_handles_large_packet` — 1 KiB payload
- `server_handles_fragmented_data` — byte-by-byte stream write

### Helper utilities
```rust
fn encode_packet(id: u16, payload: &[u8]) -> Vec<u8>  // builds Zircon wire frame
async fn start_test_server() -> (JoinHandle<()>, SocketAddr)  // binds at port 0
```
`bind_listener` / `run_listener` split allows port-0 binding for test isolation.

---

## 2.8 — Performance Baseline: Benchmarks

**Status:** [x] Complete — Criterion benchmarks for protocol and patchmgr

### `rust/protocol/benches/framing.rs`
| Benchmark | What it measures |
|---|---|
| `frame_encode/0B` | `RawFrame::encode` empty payload |
| `frame_encode/16B` | `RawFrame::encode` 16-byte payload |
| `frame_encode/256B` | `RawFrame::encode` 256-byte payload |
| `frame_encode/1024B` | `RawFrame::encode` 1 KiB payload |
| `frame_parse_burst` | Parse 1000 frames; setup (clone) excluded via `iter_batched` |
| `frame_parse_single` | Single-frame parse latency |

### `rust/patchmgr/benches/plist.rs`
| Benchmark | What it measures |
|---|---|
| `plist_encode/100` | `to_bytes` for 100 entries |
| `plist_encode/500` | `to_bytes` for 500 entries |
| `plist_encode/2000` | `to_bytes` for 2000 entries |
| `plist_decode/100` | `from_bytes` for 100 entries |
| `plist_decode/500` | `from_bytes` for 500 entries |
| `plist_decode/2000` | `from_bytes` for 2000 entries |
| `plist_roundtrip_1k` | Encode + decode 1000-entry cycle |

Run with: `cargo bench --package zircon-protocol` or `cargo bench --package zircon-patchmgr`

---

## Progress Summary

| Task | Status | Key Files |
|---|---|---|
| 2.1 Scaffold workspace | **Complete** | `rust/Cargo.toml`, 4 crates |
| 2.2 Protocol port | **Complete** | `rust/protocol/src/` |
| 2.3 DB layer port | **Complete** | `rust/db/src/` |
| 2.4 Game loop port | **Complete** | `rust/server/src/envir/`, `network/` |
| 2.5 ServerLibrary port | **Complete** | `rust/server/src/objects/`, `map/`, `envir/world.rs` |
| 2.6 Patcher/PatchMgr port | **Complete** | `rust/patchmgr/src/` |
| 2.7 Integration tests | **Complete** | `rust/server/src/tests.rs` — 8 tests |
| 2.8 Benchmarks | **Complete** | `rust/protocol/benches/framing.rs`, `rust/patchmgr/benches/plist.rs` |
