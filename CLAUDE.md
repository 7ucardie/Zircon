# Zircon — Claude Code Guidelines

## CI Must Always Pass

**Before pushing any commit, CI must be green. This is non-negotiable.**

- After every push, check the GitHub Actions "Build" workflow result.
- If the build fails, diagnose and fix the errors before continuing to the next task.
- Do not stack new feature commits on top of a failing build.
- Use `mcp__github__actions_list` + `mcp__github__get_job_logs` to retrieve failure details.

## Project Layout

| Directory | Purpose |
|---|---|
| `LibraryCore/` | Shared models, enums, system models (cross-platform, no Windows deps) |
| `ServerLibrary/` | Server-side game logic (monsters, NPCs, events, envir) |
| `Client/` | Windows game client (DirectX, DirectSound via Vortice) |
| `Server/` | Server entry point |
| `docs/phases/` | Phase progress tracking docs |

## Development Branches

| Branch | Purpose |
|---|---|
| `master` | Upstream baseline — never push directly |
| `phase/0-stabilize` | Vortice 3.8.3 API compatibility fixes |
| `phase/1-content-pipeline` | Content pipeline improvements (tasks 1.1–1.10) |
| `claude/codebase-review-rust-migration-SiWCK` | Active development branch |

Always develop on `claude/codebase-review-rust-migration-SiWCK` and push there.

## Key Constraints

- **No visual/render quality changes.** The image rendering is correct as-is.
- **No `IsDisposed` on Vortice COM objects** — it is `protected`. Use `NativePointer == IntPtr.Zero` externally.
- **MirDB ORM pattern**: All persisted properties must use the backing-field + `OnChanged(oldValue, value, "PropertyName")` boilerplate.
- **`using System.Linq;`** must be present in any file that calls `.First()`, `.Where()`, etc. on `IList<T>`.

## Build

```
dotnet build "Zircon Server.sln"
```

CI runs 4 jobs: Windows Debug, Windows Release, Linux Debug, Linux Release.
Linux jobs only build cross-platform projects (LibraryCore, ServerLibrary, etc.) — not the Windows Client.
