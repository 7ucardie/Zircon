# Source-specific Wood perimeter implementation

Status: closed single-room and connected two-room Wood generation are implemented
and visually checked. Final server traversal proof and consolidated verification
are being recorded.
This document describes the delivered recipes and remaining verification; it
is not a claim of arbitrary room-outline or doorway synthesis.

## Checked source roles

D1008 supplies four repeatable diagonal wall bodies and four convex corner
groups. Each retains complete source cells, collision, ground backing and wall
art. Source fingerprints and all-frame bounds protect captures, including
artwork anchored outside the selection. Neither sprites nor indices are rotated
or mirrored. Ground variants can differ between repeated bodies; the tests
separately establish exact source capture and matching repeated collision.

The checked body and corner coordinates are recorded in
[WALL_ASSEMBLY.md](WALL_ASSEMBLY.md), with implementation in
`wood_wall_bodies.rs` and `wood_perimeter_kit.rs`. The earlier decorated D1006
north/south kits remain reusable authored groups; their nonrepeatable approaches
are not used as generic perimeter bodies.

## Geometry and atomic composition

`wood_perimeter.rs` places the west corner, NW bodies, north corner, NE bodies,
east corner, SE bodies, south corner and SW bodies in clockwise order. NW and
NE counts each accept 3–10. Measured corner approaches require SE=NW+2 and
SW=NE. Accumulated even-grid translations must return exactly to the first seam.
All 64 supported count combinations pass source-backed generation checks.

`piece_composition.rs` accepts fixed placements and explicit seam pairs. It
validates canvas bounds, complete asset frames, even placement, unique ownership,
unique seam endpoints, profiles, widths and exact world alignment. Every paired
endpoint is consumed simultaneously, permitting cycles without weakening
`Piece::join`'s unused-port checks. The seam graph and walkable floor must be
connected; remaining ports must face unowned space. A failed composition does
not publish a partial map. A successful composition uses one document stamp.

Limits are 64 placements, 512 seams and an even canvas of 2–256 cells per side.
Covered cells cannot overlap; transparent coverage padding can. The resulting
Document can exceed the reusable Piece format's 64×64 size limit without
changing that format. Port profile equality is a geometry declaration; the
built-in recipe additionally depends on reviewed body/corner artwork seams.

## Interior, enclosure and persisted metadata

The generator starts from blocked empty space and reserves each group's complete
2×2 owned blocks. Flooding unowned blocks from outside identifies the exterior;
only unowned blocks enclosed by the ring receive undecorated source floor from
`generate::paint_floor`. Perimeter collision and artwork are never overwritten
by the fill. Exterior blocks must remain nonwalkable. Final layout validation
requires at least 64 walkable cells, one connected floor component and blocked
map-border cells.

The editor exposes **Wood wall perimeter** separately from **Enclosed chamber**,
which copies a whole authored source room. `Layout.wall_repeats` records the
counts; `Layout.wall_rooms` distinguishes one or two generated rooms and
restores the mode on reload. Older layouts default to no repeat/room metadata. Existing navigation, export, private database bundles and Test Walk
operate on the resulting map. `world_perimeter` exposes the same recipe through
[MCP authoring](MCP_AUTHORING.md).

Whole 48×54 and 58×64 rooms passed native editor/client inspection. Evidence is
under `/tmp/zircon-perimeter-first`, with individual body and corner fixtures
under `/tmp/zircon-wall-bodies-verified` and `/tmp/zircon-perimeter-kit-verified`.
Final consolidated validation totals belong in [RECOVERY.md](RECOVERY.md).

## Physical passage extension

The game's teleport triggers do not create physical openings in a wall. The
single-room recipe currently closes every wall and uses existing portal metadata.
A physical entry needs complete wall ends plus a bounded landing, another room
or a terminating corridor. An unexplained walkable breach to the canvas border
is not a completed room connection.

Two authored entry groups are available. The D10032 16×38 entry retains a whole
room/corridor junction and uses its own profiles. The D1008 20×32 masked group in
`wood_wall_entry.rs` retains a real passage between two complete wall turns and
has four body-compatible seams: west NE, recessed east NW, east SE and east NE.
Its exact source capture, four attached body seams and native source/isolated
views pass. All four wall ends require surrounding geometry; it cannot replace
one straight segment of the single-room recipe.

Two clean D1008 concave turns also have passing source/body collision tests and
native inspections: a 6×10 valley transitions NE to NW, and a 6×10 notch
transitions SE to SW. `wood_connected.rs` now uses these with six convex corners
and repeated bodies to form one closed outline around two rooms. Both NW and NE
counts accept 3–6, giving 16 checked combinations.

The concave origins must be separated by exactly 14 rows. A 6×4 rectangle of
walkable floor between their complete collars forms the physical passage.
Tests verify every floor cell is reachable, each side contains a substantial
room, and blocking the passage column separates the two sides. The composer and
interior fill retain the same ownership and enclosure guarantees as the single
room. The separately captured four-seam junction is not used by this recipe.

The UI mode is **Wood connected rooms**, selected by
`ZIRCON_CAVE_MODE=wood-connected`; metadata uses `wall_rooms: 2` with the two
repeat counts. MCP `world_perimeter` accepts `rooms: 2` and defaults to one room.
Native editor/client inspection passed for the bundle under
`/tmp/zircon-connected-mcp/connected`; the maximum 6/6 case passed under
`/tmp/zircon-connected-final/{editor-6-6,game-6-6}.png`. The real-process MCP smoke run passed 41
requests covering all twelve tools. A real actor and observer client verified
16 authoritative walking steps through the passage with teleport routes disabled.
The bounded Wood perimeter/physical-entry phase is complete. Consolidated checks
are recorded in RECOVERY.md; this does not mark the whole editor roadmap complete.

Arbitrary BSP and organic outlines need additional authored geometry recipes.
Original sprite creation is a separate planned workflow in
[SPRITE_AUTHORING_PLAN.md](SPRITE_AUTHORING_PLAN.md); renderer changes are not
required by the completed wall recipe.
