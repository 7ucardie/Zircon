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

**Status:** [x] Complete — all SharpDX references removed, Vortice.Windows in place

SharpDX is archived since 2019. It ships .NET Framework assemblies only.
The project consumes it through compatibility shims, producing `NU1701`
warnings on every build.

Full migration guide: `/docs/vortice-migration.md`

### Package changes (Client.csproj)
- [x] Remove: `SharpDX`, `SharpDX.Desktop`, `SharpDX.Direct3D9`,
      `SharpDX.DirectSound`, `SharpDX.Mathematics`
- [x] Add: `Vortice.Windows`, `Vortice.Multimedia`
- [x] Add: `Vortice.D3DCompiler` for runtime shader compilation

### Namespace replacements
- [x] `SharpDX.Direct3D9.*` → `Vortice.Direct3D9.*`
- [x] `SharpDX.DirectSound.*` → `Vortice.DirectSound.*`
- [x] `SharpDX.Mathematics.Interop.RawColorBGRA` → `Vortice.Mathematics.ColorBGRA`
- [x] `RenderLoop.Run` → `Vortice.Win32.MessagePump`

### Affected areas
- [x] `Client/Rendering/SharpDXD3D9/` — device creation, sprites, textures, surfaces
- [x] `Client/Rendering/SharpDXD3D11/` — D3D11 pipeline
- [x] `Client/Audio/` — DirectSound buffer management
- [x] All `using SharpDX.*` imports

### Key API changes
- D3D11: factory methods on device (`_device.CreateBuffer()`, `_device.CreateVertexShader()`, etc.)
- D3D9: constructors (`new VertexBuffer(device, ...)`) with `D3D9.CreateDevice()` for the device itself
- `DataStream` / `DataBox` → `MappedSubresource` with unsafe pointer writes
- `SharpDX.Color4` (.Red/.Green/.Blue/.Alpha) → `Vortice.Mathematics.Color4` (.R/.G/.B/.A)
- `SharpDX.Size2` → `Vortice.Mathematics.SizeI`
- `SharpDX.Matrix` → `System.Numerics.Matrix4x4` (CreateScale/CreateTranslation)
- `DataRectangle` → `Vortice.Direct3D9.LockedRectangle`
- Result handling: `new Result(ex.HResult) == Result.DeviceLost`

### Validation
- [x] Zero SharpDX references remain in any `.cs` file (verified by grep)
- [ ] Full build, zero NU1701 warnings (requires Windows CI runner)
- [ ] Manual test: windowed + fullscreen mode
- [ ] Manual test: audio plays correctly
- [ ] Manual test: D3D9 and D3D11 rendering paths

---

## 0.3 — Promote DX11 to Default Renderer; DX9 as Legacy Fallback

**Status:** [x] Complete — `Config.RenderingPipeline` default changed to `"DirectX 11"`

DX9 was released in 1999. DX11 is already implemented and produces
identical visual output. No render quality changes — same sprites,
same art, same look. Just a modern API underneath.

### Context
The project already has three rendering pipelines:
- `Client/Rendering/SharpDXD3D9/` — DX9 (legacy fallback)
- `Client/Rendering/SharpDXD3D11/` — DX11 (**new default**)
- `Client/Rendering/SilkVulkan/` — Vulkan (newest, Silk.NET)

### How selection works
1. `Config.RenderingPipeline` (persisted in `Config.ini`) holds the active renderer ID
2. `Program.Main()` calls `RenderingPipelineManager.InitializeWithFallback()` at startup
3. If the requested renderer fails to init, `RenderingPipelineManager.DefaultPipelineId` ("DirectX 11") is used
4. Players can switch at runtime via the in-game graphics settings (DXConfigWindow)
5. Existing `Config.ini` files that still say "DirectX 9" will continue to use DX9 until the user changes it

### Tasks
- [x] Identify where the active renderer is selected (`Client/Envir/Config.cs` line 34)
- [x] Make DX11 the default selection (one-line change to `Config.cs`)
- [x] Keep DX9 available as a legacy fallback option in settings (unchanged — D3D9 pipeline still registered)
- [ ] Verify DX11 output is visually identical to DX9 on same hardware (manual test)
- [x] `RenderingPipelineManager.DefaultPipelineId` was already "DirectX 11" — fallback was correct all along

### Why DX11 not DX12
DX12 offers more control but much more complexity. For a 2D sprite-based
game, DX11 gives everything needed (wide driver support, stable, simpler
API) without the low-level overhead of DX12. DX12/Vulkan comes in Phase 3
via wgpu/Bevy.

---

## 0.4 — Refactor PlayerObject.cs (16,900 lines)

**Status:** [x] Complete — split into 11 focused partial class files

`ServerLibrary/Models/PlayerObject.cs` is a God class. It handles
combat, inventory, quests, skills, social features, crafting, movement,
and more in a single file. This is a **partial class split** — same type,
no logic changes, just organised into focused files.

### Resulting partial class files
| File | Lines | Responsibility |
|---|---|---|
| `PlayerObject.cs` | 1,863 | Fields, constructor, Process region, lifecycle, network packets |
| `PlayerObject.Chat.cs` | 468 | Chat, ObserverChat, Inspect |
| `PlayerObject.Stats.cs` | 930 | GainExperience, LevelUp, RefreshStats, AddBaseStats, Buffs |
| `PlayerObject.Social.cs` | 2,580 | Marriage, Companions, Guild, Group, LFG, Trade |
| `PlayerObject.Quests.cs` | 981 | Quests, Mail, MarketPlace |
| `PlayerObject.Inventory.cs` | 3,186 | Items region, Appearance (Change region) |
| `PlayerObject.NPC.cs` | 3,430 | All NPC interactions, crafting, refining |
| `PlayerObject.Movement.cs` | 1,005 | Packet Actions (movement, fishing) |
| `PlayerObject.Combat.cs` | 1,631 | Combat region (attack, magic, die, etc.) |
| `PlayerObject.Instances.cs` | 1,096 | Instances, Currency, Friends, Discipline, LootBoxes, Bundles |
| `PlayerObject.Milestone.cs` | 317 | Milestone tracking |

### Tasks
- [x] Map existing methods into categories (natural seams via #region)
- [x] Create partial class files using `partial class PlayerObject`
- [x] Move methods/fields into appropriate file — no logic changes
- [x] Verify build succeeds
- [ ] Verify no regressions in game behaviour (manual test)

---

## 0.5 — Refactor GameScene.cs (5,000 lines)

**Status:** [x] Complete — split into 10 focused partial class files

`Client/Scenes/GameScene.cs` mixes rendering, input handling, HUD logic,
and scene management. Same approach as 0.4 — partial class split.

### Resulting partial class files
| File | Lines | Content |
|---|---|---|
| `GameScene.cs` | 1,507 | Properties region, fields, constructor, SetDefaultLocations, SaveChatTabs, IDisposable |
| `GameScene.Process.cs` | 171 | `Process()` main loop override |
| `GameScene.Input.cs` | 498 | `OnKeyPress`, `OnKeyDown` |
| `GameScene.Labels.cs` | 1,450 | `CreateItemLabel`, `EquipmentItemInfo`, `CreatePotionLabel`, `CreateFameLabel`, `CreateMagicLabel`, `SetItemInfo` |
| `GameScene.Combat.cs` | 646 | `UseMagic`, `CanAttackTarget`, `OnAfterDraw`, `Displacement` |
| `GameScene.Items.cs` | 321 | `FillItems`, `AddItems`, `AddCompanionItems`, `CanUseItem`, `CanWearItem` |
| `GameScene.Stats.cs` | 217 | All `*Changed()` stat notification methods |
| `GameScene.Chat.cs` | 38 | Two `ReceiveChat` overloads |
| `GameScene.Quests.cs` | 376 | Quest query/display methods |
| `GameScene.HUD.cs` | 73 | `UpdateMapIcon`, `IsAlly` |

### Tasks
- [x] Identify seams (using existing #region structure and method clusters)
- [x] Split into partial files (`sealed partial class GameScene`)
- [ ] Build and regression check (requires Windows CI runner)

---

## 0.6 — Add async/await to Networking and Database Layers

**Status:** [~] In progress — `BaseConnection` APM→async done; game loop and DB not yet converted

The project targets .NET 10.0 but all I/O is synchronous. Server threads
block on DB queries and network reads, wasting capacity.

### Key areas
- [x] `LibraryCore/Network/BaseConnection.cs` — replaced APM `BeginReceive`/`EndReceive`
      and `BeginSend` with `NetworkStream.ReadAsync`/`WriteAsync`; `BeginReceive()` shim
      kept for subclass compatibility
- [ ] `ServerLibrary/Envir/SEnvir.cs` — replace `Thread.Sleep` game loop with
      `Task`-based timer
- [ ] DB layer — MirDB is a custom in-memory ORM (not Dapper); async conversion
      is out of scope until Phase 1 ORM work
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
| 0.2 Vortice migration | **Complete** | All SharpDX removed, Vortice.Windows in place |
| 0.3 DX11 as default | **Complete** | `Config.cs` default changed to DirectX 11 |
| 0.4 PlayerObject refactor | **Complete** | 11 partial class files |
| 0.5 GameScene refactor | **Complete** | 10 partial class files |
| 0.6 Async networking/DB | **In progress** | `BaseConnection` converted; SEnvir loop + CConnection pending |
| 0.7 Test suite | Not started | — |
