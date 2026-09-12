# Editor and content-generation plan

Status: proposal (2026-09-12). Nothing below exists yet in `rust/`.

## What exists today

| Tool | Where | What it edits |
|---|---|---|
| Server data views | C# `Server/Views/*View.cs` (WinForms grids over `System.db`) | items, item stats, monsters, monster stats, drops, maps, regions, respawns, safe zones, movements, NPCs and pages, magics, base stats, sets, stores, quests, events... |
| Map viewer | C# `Server/Views/MapViewer.cs` | view a map with regions/spawns (no tile painting) |
| Library editor | C# `LibraryEditor/` | import/export images in `.Zl` sprite libraries (DXT1/DXT5), offsets, shadows |
| Rust | `mir-formats` | read-only parsers for `.Zl`, `.map`, `System.db`; `dbdump` example |

There is no Rust editor, no `.map` tile editor anywhere, and no writer for any of
the three formats in Rust. The C# server views remain usable to edit `System.db`
(the Rust server reads the same file), so editing content is possible today,
just not from the Rust side.

## Goal

A single desktop tool, `mir-editor`, that edits everything the Rust game
consumes: `System.db` records, `.map` tiles/flags, `.Zl` sprites, and can
generate new content (maps, monsters, items) with procedural generators and
AI assistance, always validating against the game's rules before saving.

## Phase 1: writers (foundation)

1. `mir-formats::mirdb` write support. `System.db` is self-describing (type
   mapping, then per-collection `next_index`, `count`, and raw records), so a
   round-trip writer only needs to re-encode `Value`s. Acceptance: load and
   save the real database byte-identical; unit test on every collection.
2. `mir-formats::map` writer: header, back layer (`(w/2)*(h/2)*3` bytes),
   14-byte cells. Acceptance: round-trip all shipped maps.
3. `mir-formats::zl` writer: DXT1/DXT5 encode (crate `texpresso` or
   `intel_tex_2`), 25-byte image headers, shadow/overlay surfaces.
   Acceptance: re-encode `StoreItems.Zl`, decode, compare with a tolerance.
4. Backups: every save writes `<file>.bak-<timestamp>` first.

## Phase 2: data editor (`mir-editor` crate, egui/eframe)

- Generic table editor built from the MirDB mapping (like the C# grids): sort,
  filter, add, delete, edit cells with typed widgets; foreign keys resolved by
  name (Item -> Monster drops, Map -> Regions -> Spawns/Safe zones).
- Specialised panels:
  - Items: icon preview from `StoreItems.Zl`, stats sub-table, class/level
    requirements, price and weight, book -> magic link.
  - Monsters: sprite preview (all directions, animation loop), stats, AI type,
    drop table with expected value per kill, respawn groups.
  - Magics: powers/costs per level, icon preview, need levels.
  - NPCs: pages, checks, actions, goods with prices; dialog preview text.
  - Maps: list with region overlay, spawn and safe zone placement.
- Live check: the tool runs the same `GameData::load` as the server and
  reports errors (dangling references, unwalkable spawn cells).
- Hot reload: `mir-server --reload` (SIGHUP or admin command) re-reads
  `System.db` so edits show up without restarting.

## Phase 3: map editor

- Tile painting with the 14 map libraries (`Data/Map Data/*.Zl`): back layer,
  middle, front, object animation, light, door and walk/fly flags.
- Brushes: single tile, rectangle fill, stamp (copy a region), autotile sets
  built by clustering tiles that appear adjacent in shipped maps.
- Walkability preview using the exact rule the server uses.
- Region tools: paint bit regions for spawns, safe zones and movements; place
  NPCs; test-walk from inside the editor by launching the client on the map.

## Phase 4: content generation

### Maps (procedural first, AI for intent)

1. Generators that only use tile ids seen in shipped maps of the chosen
   biome (forest, sand, snow, wood, cave): cellular-automata caves, room and
   corridor dungeons, and wave-function-collapse outdoors trained on the
   shipped maps' 3x3 tile neighbourhoods so walls, cliffs and roofs stay
   consistent.
2. Validation: connectivity by BFS from every exit, no unreachable spawn
   regions, minimum free area, exits paired with `MovementInfo` rows.
3. AI layer: a prompt ("a snowy mountain pass, 200x120, two exits, mid-level
   monsters") is turned into generator parameters, spawn tables and NPC
   placement by an LLM through a JSON schema; the generator does the geometry,
   the LLM never writes tiles directly. Names, descriptions and quest hooks
   also come from the model.

### Monsters

1. Stats from a level curve fitted to the shipped `MonsterInfo` rows (HP, AC,
   DC, experience, speed per level) with a "boss" and "passive animal" profile,
   so a level-30 generated monster sits between shipped level-28 and 32 ones.
2. Drops from a template per tier plus the biome's material items.
3. Sprites: animated 8-direction sets are the hard part. Plan in order of
   reliability: (a) recolour and rescale variants of existing sprite sets
   (palette shift in `.Zl`, new `MonsterImage` id), (b) mix parts from sets
   that share a skeleton (same frame layout), (c) generated sheets from an
   image model with a sprite-sheet control net, reviewed frame by frame in the
   editor before import. Ship (a) and (b) first; treat (c) as research.

### Items

1. LLM-drafted items validated against the equipment stat budget per level
   (derived from shipped items: stat points per required level and slot).
2. Icons: recoloured shipped icons or generated 36x36 icons via the same
   review-and-import path; ground sprites likewise.

## Milestones

| # | Deliverable | Visible result |
|---|---|---|
| 1 | Writers + round-trip tests | `cargo test` proves every shipped file survives load/save |
| 2 | Data editor (items, monsters, drops, magics) | edit a potion's heal and see it in-game after reload |
| 3 | Map/NPC/spawn editing | move a spawn and see it in-game |
| 4 | Map editor with painting | a new hand-made map reachable from Bichon |
| 5 | Procedural map generator + validator | a generated cave with spawns, exits and safe zone |
| 6 | Monster/item generation with AI drafts | a new monster family (recoloured) with drops |
| 7 | AI map intent + sprite-sheet import | prompt-to-map end to end |

Each milestone lands as its own commit with CI green and a screenshot.

## Decisions to confirm

- Editor UI: egui/eframe desktop app (single binary, no web stack) is the
  proposal; the alternative is a web UI served by the server.
- Where AI calls run: from the editor via an API key in the environment, never
  from the game server.
- Generated content is written to a separate `Generated.db` merged at load
  time, so shipped data stays untouched and a bad generation is one file
  delete away.
