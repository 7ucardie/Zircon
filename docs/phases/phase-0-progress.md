# Phase 0 — Stabilise the C# Codebase

**Goal:** Make the codebase safe to refactor. No behaviour changes, no new features.
Before this phase is done, no phase-1/2/3 work should merge.

---

## 0.1 — Add GitHub Actions CI

**Status:** [x] Complete — `.github/workflows/build.yml`

Every push to every branch should build all projects. Without this,
broken commits silently accumulate.

### Tasks
- [x] Create `.github/workflows/build.yml`
- [x] Runner: `windows-latest` (required — solution targets `net10.0-windows`)
- [x] Build Debug + Release, AnyCPU (matrix strategy)
- [x] Run `dotnet build "Zircon Server.sln"`
- [x] Upload Release artifacts (7-day retention)
- [ ] Add build badge to README or ROADMAP (once first run confirms green)

### Notes
- NU1701 warnings (SharpDX .NET Framework compat shims) are suppressed
  with `-p:NoWarn=NU1701` until the Vortice migration (task 0.2) removes them.
- Output paths use relative Windows paths (`..\..\Debug\Client\`) — these
  work on `windows-latest` since it's a real Windows environment.

---

## 0.2 — Complete Vortice.Windows Migration (replace SharpDX)

**Status:** [ ] Not started

SharpDX is archived since 2019. It ships .NET Framework assemblies only.
The project consumes it through compatibility shims, producing `NU1701`
warnings on every build.

Full migration guide: `/docs/vortice-migration.md`

### Package changes (Client.csproj)
- [ ] Remove: `SharpDX`, `SharpDX.Desktop`, `SharpDX.Direct3D9`,
      `SharpDX.DirectSound`, `SharpDX.Mathematics`
- [ ] Add: `Vortice.Windows`, `Vortice.Multimedia`
- [ ] Add: `Vortice.D3DCompiler` if runtime shader compilation is needed

### Namespace replacements
- [ ] `SharpDX.Direct3D9.*` → `Vortice.Direct3D9.*`
- [ ] `SharpDX.DirectSound.*` → `Vortice.DirectSound.*`
- [ ] `SharpDX.Mathematics.Interop.RawColorBGRA` → `Vortice.Mathematics.ColorBGRA`
- [ ] `RenderLoop.Run` → `Vortice.Win32.MessagePump`

### Affected areas
- [ ] `Client/Rendering/SharpDXD3D9/` — device creation, sprites, textures, surfaces
- [ ] `Client/Rendering/SharpDXD3D11/` — D3D11 pipeline
- [ ] `Client/Audio/` — DirectSound buffer management
- [ ] All `using SharpDX.*` imports

### Validation
- [ ] Full build, zero NU1701 warnings
- [ ] Manual test: windowed + fullscreen mode
- [ ] Manual test: audio plays correctly
- [ ] Manual test: D3D9 and D3D11 rendering paths

---

## 0.3 — Promote DX11 to Default Renderer; DX9 as Legacy Fallback

**Status:** [ ] Not started (depends on 0.2)

DX9 was released in 1999. DX11 is already implemented and produces
identical visual output. No render quality changes — same sprites,
same art, same look. Just a modern API underneath.

### Context
The project already has three rendering pipelines:
- `Client/Rendering/SharpDXD3D9/` — DX9 (current default)
- `Client/Rendering/SharpDXD3D11/` — DX11 (already implemented)
- `Client/Rendering/SilkVulkan/` — Vulkan (newest, Silk.NET)

### Tasks
- [ ] Identify where the active renderer is selected (likely `CEnvir.cs` or config)
- [ ] Make DX11 the default selection
- [ ] Keep DX9 available as a legacy fallback option in settings
- [ ] Verify DX11 output is visually identical to DX9 on same hardware
- [ ] Update default config / documentation to reflect new default

### Why DX11 not DX12
DX12 offers more control but much more complexity. For a 2D sprite-based
game, DX11 gives everything needed (wide driver support, stable, simpler
API) without the low-level overhead of DX12. DX12/Vulkan comes in Phase 3
via wgpu/Bevy.

---

## 0.4 — Refactor PlayerObject.cs (16,900 lines)

**Status:** [ ] Not started

`ServerLibrary/Models/PlayerObject.cs` is a God class. It handles
combat, inventory, quests, skills, social features, crafting, movement,
and more in a single file. This is a **partial class split** — same type,
no logic changes, just organised into focused files.

### Proposed partial class files
| File | Responsibility |
|---|---|
| `PlayerObject.Core.cs` | Fields, constructor, lifecycle (Update/Die/etc.) |
| `PlayerObject.Combat.cs` | Attack, damage calculation, PvP |
| `PlayerObject.Skills.cs` | Skill usage, cooldowns, magic spells |
| `PlayerObject.Inventory.cs` | Item management, equipment, storage, drop |
| `PlayerObject.Quests.cs` | Quest tracking, completion, rewards |
| `PlayerObject.Stats.cs` | Stat calculation, buffs, debuffs |
| `PlayerObject.Social.cs` | Guild, party, marriage, relationships |
| `PlayerObject.Movement.cs` | Pathfinding, teleport, map transitions |
| `PlayerObject.Crafting.cs` | Crafting, refining, upgrading |

### Tasks
- [ ] Map existing methods into categories (search for natural seam points)
- [ ] Create partial class files using `partial class PlayerObject`
- [ ] Move methods/fields into appropriate file — no logic changes
- [ ] Verify build succeeds
- [ ] Verify no regressions in game behaviour

---

## 0.5 — Refactor GameScene.cs (5,000 lines)

**Status:** [ ] Not started

`Client/Scenes/GameScene.cs` mixes rendering, input handling, HUD logic,
and scene management. Same approach as 0.4 — partial class split.

### Proposed partial class files
- `GameScene.Core.cs` — scene lifecycle, main loop
- `GameScene.Rendering.cs` — draw calls, sprite batching
- `GameScene.Input.cs` — mouse and keyboard handling
- `GameScene.HUD.cs` — UI overlays, minimap, status bars
- `GameScene.Network.cs` — incoming packet processing

### Tasks
- [ ] Identify seams (rendering vs input vs network)
- [ ] Split into partial files
- [ ] Build and regression check

---

## 0.6 — Add async/await to Networking and Database Layers

**Status:** [ ] Not started

The project targets .NET 10.0 but all I/O is synchronous. Server threads
block on DB queries and network reads, wasting capacity.

### Key areas
- [ ] `LibraryCore/Network/` — replace sync socket reads with `ReadAsync`
- [ ] `ServerLibrary/Envir/SEnvir.cs` — replace `Thread.Sleep` game loop with
      `Task`-based timer
- [ ] All Dapper `Query<T>` calls → `QueryAsync<T>` throughout
- [ ] `Client/Envir/CConnection.cs` — async client socket handling
- [ ] Audit for `.Result` / `.Wait()` deadlock patterns after conversion

---

## 0.7 — Establish First Automated Test Suite

**Status:** [ ] Not started

There are zero automated tests today. Start with `LibraryCore` since it
has no Windows/UI dependencies — it can run in CI.

### Tasks
- [ ] Add `LibraryCore.Tests` xUnit project to solution
- [ ] Tests for `Functions.cs` utility methods
- [ ] Tests for network packet serialisation/deserialisation
- [ ] Tests for `ConfigReader` reflection-based initialisation
- [ ] Tests for `Stat` calculation logic
- [ ] Wire test run into CI workflow (step after build)

---

## Progress Summary

| Task | Status | Branch/PR |
|---|---|---|
| 0.1 CI/CD | **Complete** | `.github/workflows/build.yml` |
| 0.2 Vortice migration | Not started | — |
| 0.3 DX11 as default | Not started | — |
| 0.4 PlayerObject refactor | Not started | — |
| 0.5 GameScene refactor | Not started | — |
| 0.6 Async networking/DB | Not started | — |
| 0.7 Test suite | Not started | — |
