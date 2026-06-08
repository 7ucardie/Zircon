# Phase 1 — Content Pipeline Improvements

**Goal:** Add new creatures, maps, dungeons, NPCs, and timed events
without writing or recompiling C# code. All content becomes data-driven.

**Depends on:** Phase 0 complete (stable codebase, CI passing)

---

## 1.1 — Dynamic Monster AI Registry

**Status:** [x] Complete — `ServerLibrary/Models/MonsterRegistry.cs` + `MonsterRegistrations.cs`

### Tasks
- [x] Create `MonsterRegistry` class with `Register()` and `Create()` factory methods
- [x] Register all 136 AI cases via factory lambdas in `MonsterRegistrations.RegisterAll()`
- [x] Remove the 525-line `GetMonster()` switch statement; replaced with one-liner
- [x] All 136 AI types covered, including complex SpawnList init and field overrides
- [ ] Document how to add a new monster type (update `CONTRIBUTING.md`)

### Implementation note
Used factory-function registry (`Func<MonsterInfo, MonsterObject>`) rather than type-only
registry so that complex cases (SpawnList lookups, field overrides) work without special handling.

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
- [x] Define `MonsterBehaviour` [Flags] enum in `LibraryCore/Enum.cs`
- [x] Add `MonsterBehaviour Behaviours` field to `MonsterInfo`
- [x] Implement `ProcessBehaviours()` in `MonsterObject` — dispatches to per-flag methods
- [x] `Heals` — periodic 10% burst heal every 30 s (new; existing regen is continuous 2%)
- [x] `Teleports` — one-shot random teleport when HP drops below 25%
- [x] `Enrages` — one-shot 40% AttackDelay reduction + red name when HP drops below 25%
- [ ] Refactor existing monster classes to use flags instead of duplicating logic
- [ ] Verify monster behaviour is unchanged in-game

---

## 1.3 — NPC Scripting Support

**Status:** [x] Complete — `ServerLibrary/Envir/NpcScriptEngine.cs`

Complex NPC logic (branching quests, dynamic pricing, faction checks)
requires C# code changes today. A scripting layer allows content
designers to write logic without a developer.

### Current location
`LibraryCore/SystemModels/NPCInfo.cs` — dialog tree (pages, checks, actions)
`ServerLibrary/Models/NPCObject.cs` — runtime NPC execution
`ServerLibrary/Envir/NpcScriptEngine.cs` — Lua engine, proxy types

### Implementation
Added optional `ScriptFile` to `NPCPage`. Scripts execute via MoonSharp
(`CoreModules.Preset_SoftSandbox`). `on_open` hook fires before check/action
chain; `NPCCheckType.Script` and `NPCActionType.Script` dispatch to named Lua
functions. Page navigation uses BFS over the NPCPage graph since no flat
page list exists. Scripts live in `Scripts/NPC/` relative to the server binary.

### Tasks
- [x] Evaluate MoonSharp vs pure C# `IScript` interface approach — chose MoonSharp
- [x] Add `ScriptFile` (optional) to `NPCPage`
- [x] Implement script sandbox (expose only safe player/npc API)
- [x] Implement `on_open` hook; `Script` check and action types
- [x] Add `InvalidateCache()` for script hot-reload
- [x] Security: `CoreModules.Preset_SoftSandbox` — no arbitrary file/OS access

---

## 1.4 — Cron-Style Scheduled Event Triggers

**Status:** [x] Complete — `ServerLibrary/Envir/Events/Triggers/ScheduledTime.cs`

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

### Implementation
Added `CronExpression` string property to `WorldEventTrigger` (persisted via MirDB).
Added `ScheduledTime = 4` to `WorldEventTriggerType` enum.
`ScheduledTime` trigger uses `Cronos.CronExpression.GetNextOccurrence()` to check
whether the expression fired within the last minute. Invalid expressions are logged
via `SEnvir.SaveError` and return false. Registered automatically via reflection
(same mechanism as all other triggers). Called every minute via the existing
`EventTimerTime` tick in `SEnvir.cs`.

### Tasks
- [x] Add `Cronos` 0.8.4 package to `ServerLibrary`
- [x] Add `ScheduledTime` trigger class with `CronExpression` string field on `WorldEventTrigger`
- [x] Register trigger in `EventInfoHandler` (automatic via `[EventTriggerType]` attribute)
- [x] Integrate with server game loop — `EventHandler.Process("SCHEDULEDTIME")` each minute tick
- [ ] Add at least one example scheduled event in sample data
- [ ] Document cron syntax in content authoring guide

---

## 1.5 — Event Chaining

**Status:** [x] Complete — `ServerLibrary/Envir/Events/Actions/FireEvent.cs`

Events can now trigger other events via the `FireEvent` action type.

### Implementation
`FireEvent = 40` added to `EventActionType` enum.
`WorldEventInfoList` added to `SEnvir` so events are findable by name.
`EventInfoHandler.FireWorldEvent(WorldEventInfo)` executes all actions
of a target event unconditionally (depth-first, no TriggerValue gating).
`FireEvent` action: reads `StringParameter1` as target event Description,
guards circular chains via `[ThreadStatic] int _depth` (max 10), logs
unknown events and depth violations via `SEnvir.SaveError`.

### Tasks
- [x] Add `FireEvent` to event action enum in `LibraryCore`
- [x] Add `WorldEventInfoList` to `SEnvir` for name-based lookup
- [x] Add `FireWorldEvent()` method to `EventInfoHandler`
- [x] Add `FireEvent` action class in `ServerLibrary/Envir/Events/Actions/`
- [x] Guard against circular event chains (max depth = 10)
- [ ] Write integration test: event A fires event B on trigger

---

## 1.6 — Persistent EventLog (survives restarts)

**Status:** [x] Complete — `EventLogEntry.cs` + `EventInfoHandler.PersistEventLog()`

`EventLog` is persisted via a `[UserObject]` MirDB model, surviving server restarts.

### Implementation
- `ServerLibrary/DBModels/EventLogEntry.cs` — MirDB persistent mirror of in-memory `EventLog`
  - Fields: `Key`, `WorldEvent`, `PlayerEvent`, `MonsterEvent`, `PlayerIndex`, `InstanceInfo`, `InstanceSequence`, `CurrentValue`, `TriggerCountData`
  - `TriggerCountData` serializes trigger-count dictionaries as `"W{index}:{count}|P{index}:{count}|M{index}:{count}"`
  - All properties follow the MirDB backing-field + `OnChanged()` pattern
- `SEnvir.EventLogEntryList` — `DBCollection<EventLogEntry>` wired into the session
- `SEnvir.LoadEventLogs()` — called at startup; rebuilds in-memory `EventLogs` list from DB
- `EventInfoHandler.PersistEventLog()` — called after every trigger fire (World, Player, Monster)
- `SEnvir` instance cleanup (line 4400) — deletes `EventLogEntry` rows on instance expiry

### Tasks
- [x] Add `EventLog` DB table via MirDB
- [x] Persist event state changes to DB on update
- [x] Load event state from DB on server startup
- [x] Prune expired event logs on cleanup cycle
- [x] Ensure instance-scoped events are cleaned up when instance expires

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
| 1.1 Monster AI registry | **Complete** | `MonsterRegistry.cs` + `MonsterRegistrations.cs`; 525-line switch removed |
| 1.2 Behaviour composition | **Partial** | Enum + field added; Heals/Teleports/Enrages implemented; class refactor pending |
| 1.3 NPC scripting | **Complete** | `NpcScriptEngine.cs`; MoonSharp Lua, sandboxed, `on_open`/Script check+action |
| 1.4 Scheduled events | **Complete** | `ScheduledTime.cs`; Cronos 0.8.4; `CronExpression` on `WorldEventTrigger` |
| 1.5 Event chaining | **Complete** | `FireEvent.cs`; `WorldEventInfoList` in SEnvir; depth guard via `[ThreadStatic]` |
| 1.6 Persistent EventLog | **Complete** | `EventLogEntry.cs`; `PersistEventLog()`; `LoadEventLogs()`; instance cleanup |
| 1.7 Dungeon phases | Not started | |
| 1.8 Dungeon difficulty | Not started | Depends on 1.7 |
| 1.9 JSON map format | Not started | |
| 1.10 Hot reload | Not started | Depends on 1.9 |
