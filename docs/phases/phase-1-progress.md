# Phase 1 — Content Pipeline Improvements

**Goal:** Add new creatures, maps, dungeons, NPCs, and timed events
without writing or recompiling C# code. All content becomes data-driven.

**Depends on:** Phase 0 complete (stable codebase, CI passing)

---

## 1.1 — Dynamic Monster AI Registry

**Status:** [ ] Not started

Currently adding a monster = write a C# class + add a case to a
400-line `GetMonster()` switch statement in `MonsterObject.cs`, then
recompile. This doesn't scale past ~120 AI types.

### Current location
`ServerLibrary/Models/MonsterObject.cs` — `GetMonster(int ai)` switch statement

### Proposed approach
Replace the switch with a registry that maps AI IDs to types at startup:

```csharp
// Registration (one line per monster type)
MonsterRegistry.Register(4,  typeof(TreeMonster));
MonsterRegistry.Register(13, typeof(LordNiJae));
MonsterRegistry.Register(22, typeof(ZumaKing));

// Factory (replaces the entire switch)
public static MonsterObject GetMonster(int ai) =>
    MonsterRegistry.Create(ai) ?? new DefaultMonster();
```

### Tasks
- [ ] Create `MonsterRegistry` class with `Register()` and `Create()` methods
- [ ] Auto-register all existing AI classes via reflection or static initializer
- [ ] Remove the `GetMonster()` switch statement
- [ ] Verify all 110+ monster types still work
- [ ] Document how to add a new monster type (update `CONTRIBUTING.md`)

---

## 1.2 — Behaviour Composition Flags

**Status:** [ ] Not started

Many monsters share abilities (poison, summons, healing, teleporting)
but each one re-implements them in its own class. This creates 110+
files with duplicated logic.

### Proposed approach
Move common behaviours into composable components that any monster
can opt into via `MonsterInfo` flags:

| Flag | Behaviour |
|---|---|
| `HasPoison` | Apply poison on melee hit |
| `Summons` | Spawn minions at HP threshold |
| `Heals` | Self-heal periodically |
| `Teleports` | Random teleport on low HP |
| `AOEAttack` | Area-of-effect melee swing |
| `RangeAttack` | Ranged projectile attack |
| `Enrages` | Speed/damage boost below 25% HP |

### Tasks
- [ ] Define `MonsterBehaviour` enum flags in `LibraryCore/Enum.cs`
- [ ] Add `MonsterBehaviour Behaviours` field to `MonsterInfo`
- [ ] Implement each behaviour in `MonsterObject` as a composable method
- [ ] Refactor at least 5 monster classes to use composition instead of
      reimplementing abilities
- [ ] Verify monster behaviour is unchanged in-game

---

## 1.3 — NPC Scripting Support

**Status:** [ ] Not started

Complex NPC logic (branching quests, dynamic pricing, faction checks)
requires C# code changes today. A scripting layer allows content
designers to write logic without a developer.

### Current location
`LibraryCore/SystemModels/NPCInfo.cs` — dialog tree (pages, checks, actions)
`ServerLibrary/Models/NPCObject.cs` — runtime NPC execution

### Proposed approach
Add an optional `ScriptFile` field to `NPCPage`. When set, execute
the script instead of (or alongside) the normal check/action chain.

Recommended: **MoonSharp** (Lua interpreter, .NET native, sandboxed,
mature, no native binaries needed).

```lua
-- Example: DynamicMerchant.lua
function on_open(player, npc)
  if player.level >= 50 then
    npc:show_page("HighLevelShop")
  else
    npc:show_page("StandardShop")
  end
end
```

### Tasks
- [ ] Evaluate MoonSharp vs pure C# `IScript` interface approach
- [ ] Add `ScriptFile` (optional) to `NPCPage`
- [ ] Implement script sandbox (expose only safe player/npc API)
- [ ] Implement `on_open`, `on_buy`, `on_sell`, `on_close` hooks
- [ ] Add script hot-reload for development
- [ ] Security: scripts run server-side only, no arbitrary file access

---

## 1.4 — Cron-Style Scheduled Event Triggers

**Status:** [ ] Not started

Today events can only trigger on time-of-day changes (dawn/day/dusk/night)
or per-minute timer ticks. There's no way to say "run the Castle Siege
event every Saturday at 8pm" without hardcoded game loop logic.

### Current location
`ServerLibrary/Envir/Events/Triggers/WorldTimeOfDay.cs`
`ServerLibrary/Envir/Events/Triggers/TimerMinute.cs`

### Proposed new trigger
Add a `ScheduledTime` trigger type with a cron-expression field:

```
// Example cron expressions
"0 20 * * 6"     — every Saturday at 8:00pm
"0 12 * * *"     — daily at noon
"0 */4 * * *"    — every 4 hours
"0 20 1 * *"     — 1st of every month at 8pm
```

Use the `Cronos` NuGet package (MIT, .NET-native, no native deps) for
expression parsing and next-fire-time calculation.

### Tasks
- [ ] Add `Cronos` package to `ServerLibrary`
- [ ] Add `ScheduledTime` trigger class with `CronExpression` string field
- [ ] Register trigger in `EventInfoHandler`
- [ ] Integrate with server game loop — check due events each minute tick
- [ ] Add at least one example scheduled event in sample data
- [ ] Document cron syntax in content authoring guide

---

## 1.5 — Event Chaining

**Status:** [ ] Not started

Events cannot currently trigger other events. This means multi-stage
world events (kill boss → start phase 2 → reward all players) need
bespoke code.

### Proposed approach
Add a `FireEvent` action type. When executed, it looks up the named
event and fires it as if it had been triggered normally.

```
// Example: boss death triggers reward event
MonsterDie trigger: BossMonster
  Action: FireEvent("WorldBossRewardEvent")
  Action: BroadcastMessage("The boss has fallen!")
```

### Tasks
- [ ] Add `FireEvent` action class in `ServerLibrary/Envir/Events/Actions/`
- [ ] Add event lookup by name in `EventInfoHandler`
- [ ] Guard against circular event chains (max depth = 10)
- [ ] Add `FireEvent` to event action enum in `LibraryCore`
- [ ] Write integration test: event A fires event B on trigger

---

## 1.6 — Persistent EventLog (survives restarts)

**Status:** [ ] Not started

`EventLog` is in-memory. A server restart resets all event state —
timed events lose their position, player event progress is wiped.

### Tasks
- [ ] Add `EventLog` DB table via MirDB
- [ ] Persist event state changes to DB on update
- [ ] Load event state from DB on server startup
- [ ] Prune expired event logs on cleanup cycle
- [ ] Ensure instance-scoped events are cleaned up when instance expires

---

## 1.7 — Dungeon Phases / Progression

**Status:** [ ] Not started

Dungeons are instances (`InstanceInfo`) but have no internal progression.
You enter, kill everything, and exit. There's no "kill the first boss to
unlock the next wing" mechanic.

### Proposed approach
Add an `InstancePhase` list to `InstanceInfo`. Each phase has:
- Entry condition (all monsters in region dead / timer elapsed / item used)
- Actions on phase start (unlock doors, spawn next group, broadcast message)
- Completion rewards per phase

### Tasks
- [ ] Add `InstancePhase` model to `LibraryCore/SystemModels/InstanceInfo.cs`
- [ ] Add phase tracking to server-side instance state
- [ ] Wire phase entry conditions to existing trigger system (region clear,
      monster death)
- [ ] Add `OnPhaseComplete` action list (spawns, teleports, broadcasts)
- [ ] Test with a 2-phase dungeon end-to-end

---

## 1.8 — Dungeon Difficulty Scaling

**Status:** [ ] Not started

Every player gets the same instance regardless of level or group size.

### Proposed approach
Add `DifficultyMode` enum (`Normal`, `Hard`, `Nightmare`) to
`InstanceInfo`. Each difficulty level has stat multipliers and
optionally a different `RespawnIndex`.

### Tasks
- [ ] Add `DifficultyMode` enum to `LibraryCore/Enum.cs`
- [ ] Add `Difficulties` config list to `InstanceInfo`
- [ ] Apply stat multipliers to spawned monsters on instance creation
- [ ] Expose difficulty selection in dungeon finder UI
- [ ] Scale drop rates and experience per difficulty

---

## 1.9 — JSON Map Definitions

**Status:** [ ] Not started

Map walkable cells are stored in binary `.map` files — not human-readable,
not version-controlled meaningfully, hard to diff, impossible to hand-edit.

### Proposed approach
Provide a converter and a JSON schema. Existing binary maps convert once;
new maps can be authored in JSON directly.

```json
{
  "name": "BichonWall",
  "width": 600,
  "height": 600,
  "cells": "base64-encoded-bitfield-or-RLE",
  "regions": [...]
}
```

### Tasks
- [ ] Define JSON schema for map format
- [ ] Write `MapConverter` tool (binary `.map` → JSON)
- [ ] Update map loader in `ServerLibrary/Models/Map.cs` to accept JSON
- [ ] Keep binary loader as fallback for existing assets
- [ ] Document map format for content authors

---

## 1.10 — Hot Reload for Map and Content Data

**Status:** [ ] Not started (depends on 1.9)

Every content change today requires a server restart. Hot reload
dramatically speeds up map design and balance iteration.

### Tasks
- [ ] Add file watcher on `Maps/` and `Data/` directories (development mode only)
- [ ] On change: reload affected `MapInfo` / `MonsterInfo` / `NPCInfo` without
      restarting server
- [ ] Gate hot reload behind a `--dev` flag so production servers are unaffected
- [ ] Broadcast "map reloaded" notice to GMs on hot reload

---

## Progress Summary

| Task | Status | Notes |
|---|---|---|
| 1.1 Monster AI registry | Not started | Unblocks new creature creation without code |
| 1.2 Behaviour composition | Not started | Depends on 1.1 |
| 1.3 NPC scripting | Not started | |
| 1.4 Scheduled events | Not started | |
| 1.5 Event chaining | Not started | |
| 1.6 Persistent EventLog | Not started | |
| 1.7 Dungeon phases | Not started | |
| 1.8 Dungeon difficulty | Not started | Depends on 1.7 |
| 1.9 JSON map format | Not started | |
| 1.10 Hot reload | Not started | Depends on 1.9 |
