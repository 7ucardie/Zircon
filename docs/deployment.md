# Zircon — Deployment Guide

## Prerequisites

| Requirement | Version | Notes |
|---|---|---|
| .NET SDK | 10.0 or later | Server and client both target `net10.0` |
| Windows OS | Windows 10 / Server 2019+ | Client and Server require `net10.0-windows8.0` |
| Visual Studio or .NET CLI | Any | `dotnet build` is sufficient for CI/CD |
| SQL Server or SQLite | — | MirDB ORM handles schema; see Data directory |
| DirectX 11 runtime | Installed by Windows | Required by the game client |

> **Linux note:** `LibraryCore` and `ServerLibrary` compile on Linux. The `Client` project and `Server` entry point are Windows-only (`net10.0-windows8.0`).

---

## Repository layout

```
Zircon/
├── LibraryCore/          # Shared models and enums (cross-platform)
├── ServerLibrary/        # Game logic: monsters, NPCs, events, maps
├── Server/               # Server entry point (Windows)
├── Client/               # Windows game client (DirectX 11 / DirectX 9)
├── Launcher/             # Patcher / launcher GUI
├── Scripts/NPC/          # Lua NPC scripts (hot-reloaded by NpcScriptEngine)
├── Data/                 # Game data files (MirDB database, palettes, etc.)
└── docs/                 # Developer documentation
```

---

## Building

### Full solution (Windows)

```bash
dotnet build "Zircon Server.sln" -c Release
```

### Server only (Windows or Linux)

```bash
dotnet build Server/Server.csproj -c Release
```

### Client only (Windows)

```bash
dotnet build Client/Client.csproj -c Release
```

Output binaries land in `Server/bin/Release/` and `Client/bin/Release/` respectively (no framework subdirectory — `AppendTargetFrameworkToOutputPath` is disabled).

---

## Server configuration

Edit `Server/bin/Release/ServerConfig.json` (created on first run if absent):

| Key | Default | Description |
|---|---|---|
| `Port` | `7000` | TCP port clients connect to |
| `DatabasePath` | `./Data/` | Path to the MirDB data directory |
| `MaxPlayers` | `500` | Hard cap on simultaneous connections |
| `LogPath` | `./Logs/` | Directory for error and event logs |

---

## Running the server

```bash
cd Server/bin/Release
./Server.exe           # Windows
```

The server writes errors to `Logs/Errors/` and events to `Logs/Events/`. Check `Logs/Errors/` first if the server does not start.

### Scheduled events (cron)

World events using the `ScheduledTime` trigger type take a standard 5-field cron expression stored in `WorldEventTrigger.CronExpression`. The server evaluates these once per minute. Examples:

```
"0 20 * * 6"    — every Saturday at 20:00
"0 12 * * *"    — daily at noon
"0 */4 * * *"   — every 4 hours
```

---

## Running the client

```bash
cd Client/bin/Release
./Zircon.exe
```

The client reads `Config.json` in the same directory. The most important settings:

| Key | Description |
|---|---|
| `ServerIP` | IP or hostname of the game server |
| `ServerPort` | Must match server `Port` (default `7000`) |
| `FullScreen` | `true` / `false` |
| `GameSize` | Resolution, e.g. `"1920x1080"` |
| `Renderer` | `"D3D11"` (default) or `"D3D9"` |

---

## NPC scripts

Lua scripts live in `Scripts/NPC/` relative to the server binary. The engine uses MoonSharp (`CoreModules.Preset_SoftSandbox`) — no file system or OS access is available to scripts.

```lua
-- Scripts/NPC/Blacksmith.lua
function on_open(player, npc)
    npc:say("Welcome, " .. player.name .. "!")
end
```

Call `SEnvir.EventHandler.InvalidateCache()` (or restart the server) to reload scripts. Hot-reload via `InvalidateCache()` is supported at runtime.

---

## CI / CD

GitHub Actions runs four jobs on every push:

| Job | Platform | Projects built |
|---|---|---|
| Windows Debug | `windows-latest` | Full solution |
| Windows Release | `windows-latest` | Full solution |
| Linux Debug | `ubuntu-latest` | LibraryCore, ServerLibrary, Server, Tests |
| Linux Release | `ubuntu-latest` | LibraryCore, ServerLibrary, Server, Tests |

**The build must be green before merging any branch.** Check `Actions` → `Build` on GitHub. Retrieve failure logs with:

```bash
# Using GitHub CLI (gh)
gh run view --log-failed
```

Active development branch: `claude/codebase-review-rust-migration-SiWCK`

---

## Adding a new monster AI type

1. Create a class inheriting `MonsterObject` in `ServerLibrary/Models/Monsters/`.
2. Register it in `ServerLibrary/Models/MonsterRegistrations.cs`:
   ```csharp
   MonsterRegistry.Register(MonsterAI.MyNewMonster, info => new MyNewMonster(info));
   ```
3. Add the enum value to `MonsterAI` in `LibraryCore/Enum.cs`.
4. No C# recompilation needed to assign the AI to a mob — set `MonsterInfo.AI` in the DB editor.

---

## Adding a new timed world event

1. In the DB editor, create a `WorldEventInfo` record with the desired actions.
2. Add a `WorldEventTrigger` of type `ScheduledTime`.
3. Set `CronExpression` on the trigger to the desired schedule (5-field cron).
4. The server picks it up on the next minute tick — no restart needed.

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Server exits immediately | MirDB schema mismatch | Delete `Data/*.db` and let server recreate |
| Client black screen | DirectX device lost | Verify DirectX 11 is installed; try `"Renderer": "D3D9"` |
| `[FireEvent] Event not found` in error log | Wrong `StringParameter1` on action | Check event `Description` field matches exactly |
| NPC script not updating | Script cache not invalidated | Restart server or call `InvalidateCache()` via GM command |
| CI fails Windows build only | Vortice API regression | Check `docs/vortice-migration.md` for Vortice 3.x breaking changes |
