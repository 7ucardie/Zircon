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

## Branching

`master` is the upstream baseline. Do not push to it directly; create a feature branch and open a pull request.
Active Rust work happens on `rust/prototype`.

## Rust Rewrite (`rust/`)

The game is being rebuilt in Rust as a purpose-built client + server (see `rust/README.md`).

- **Never edit the C# projects.** They are the reference specification for formats and rules only.
- Assets are not in the repo: `~/zircon-assets/Client` (Data, Map, Sound) and `~/zircon-assets/Database/System.db`. Set `ZIRCON_ASSETS` to run asset-backed tests.
- Every milestone must end with something runnable and visible; use `ZIRCON_SCREENSHOT=out.png:5 cargo run -p mir-client` to check rendering.
- CI for Rust is `.github/workflows/rust.yml`: fmt, clippy `-D warnings`, build and test on Linux/macOS/Windows, plus the server Docker image. Keep it green like the C# build.
- Build/test: `cd rust && cargo build --workspace && cargo test --workspace`.

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
