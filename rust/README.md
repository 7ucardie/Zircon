# Zircon in Rust

A purpose-built Rust client and server for Legend of Mir 3, reading the
original Zircon assets (`.Zl` sprite libraries, `.map` maps, `System.db`).
The C# projects in the parent directory are reference only.

## Crates

| Crate | What it is |
|---|---|
| `mir-formats` | Readers for `.Zl` (DXT1/DXT5 sprites), `.map`, and MirDB `System.db` |
| `mir-proto` | Client/server messages, postcard-encoded with a length prefix |
| `mir-server` | Game server: accounts, characters, maps, spawns, movement, melee combat, monster AI |
| `mir-client` | Native client on winit + wgpu: login, character select/create, game scene |

## Assets

Not in the repository. Extract the Zircon client package and database so that:

```
~/zircon-assets/Client/Data/*.Zl
~/zircon-assets/Client/Data/Map Data/**/*.Zl
~/zircon-assets/Client/Map/*.map
~/zircon-assets/Database/System.db
```

Override the location with `ZIRCON_ASSETS=/path/to/Client` (and `ZIRCON_DB`).

## Run

```
cd rust
cargo run -p mir-server              # listens on 0.0.0.0:7000
cargo run -p mir-client              # connects to 127.0.0.1:7000
```

The client opens on the login screen: create an account, log in, create a
character (class, gender, hair) and start the game. Accounts and characters
are stored by the server in `data/accounts.json` (Argon2 password hashes),
override the directory with `--data DIR` or `ZIRCON_DATA`.

Controls in the world: hold left mouse to walk, right mouse to run, click an
adjacent monster to attack, click an NPC to talk (you walk over), click a
ground item to walk to it and pick it up, `Tab` picks up what is under you,
`W` opens the bag, `Q` the character window, `Esc` closes windows and then
returns to the character list. In the bag, right-click uses or equips an item
(or sells it while a shop is open); in the character window right-click
unequips. Helmets and shields show on the character. The belt above the HUD
(toggle with `Z`) has ten slots on keys `1`-`9` and `0`: pick up a bag item
and click a slot (or hover the item and press the key) to link it, press the
key or right-click the slot to use it, left-click a linked slot to clear it.
Potions heal at once and every consumable starts the item cooldown shown on
the belt (Zircon's `Durability` in ms, 250 ms floor). Shop lists scroll with
the wheel. When you die a Revive button returns you to town at once; the
server forces it after ten minutes. The backquote key toggles the debug line,
`F12` saves a screenshot.

What exists in the world: real spawns with Zircon's drop tables (1 in N per
row), ground items that only the killer can take for two minutes, gold, the
125 NPCs with their data-driven dialog pages, buy/sell shops with Zircon's
prices, and every map exit from `MovementInfo` (with level requirements).

Skills: learn them from books (right-click in the bag), open the skill window
with `E`, hover a skill and press `F1`-`F11` to bind it, then press the key to
cast. Implemented: Swordsmanship, Spirit Sword and Willow Dance (passives),
Slaying (charged power attack), Thrusting and Half Moon (stances hitting extra
cells), Fire Ball, Ice Bolt, Thunder Bolt, Repulsion, Heal (heal over time),
Poison Dust (green/red poison), Flaming Daggers and Shredding, with Zircon's
power, cost, cooldown and skill-experience rules and the original effect
sprites. Other skills show in the window but cannot be cast yet.

Developer automation (also used for visual checks):
`ZIRCON_SCREENSHOT=out.png:5` saves a frame after 5 s and exits;
`ZIRCON_AUTOLOGIN=email:password` logs in, creating the account if missing;
`ZIRCON_AUTOSTART=name` enters the world with that character, creating a
warrior if needed; `ZIRCON_OPEN_CREATE=1` opens the character creation dialog;
`ZIRCON_OPEN_WINDOWS=1` opens the bag and character windows on entry;
`ZIRCON_AUTO_NPC=name` walks to that NPC and opens its dialog;
`ZIRCON_AUTOCLASS=wizard|taoist|assassin` picks the auto-created class;
`ZIRCON_OPEN_SKILLS=1` opens the skill window; `ZIRCON_AUTO_CAST=<F key>`
casts that key's spell at the nearest monster; `ZIRCON_AUTO_BELT=1` links every
consumable type in the bag to the belt and `ZIRCON_AUTO_USE=<slot>` presses
that belt key once. Server side, `ZIRCON_DEV_LEVEL=n` starts new characters at
level n, `ZIRCON_DEV_SKILLS=1` grants every class skill the level allows, and
`ZIRCON_DEV_ITEMS="Bronze Helmet;Healing Potion*5"` hands out (and wears)
the named items on entry.

`cargo run -p mir-server -- --inspect` prints the start map, player stats and
spawn table without opening a port. `--map <file>` forces a start map.

## Docker

```
docker compose -f rust/docker-compose.yml up --build
```

Only the server runs in a container; the client is a GPU application and runs
natively. Assets are mounted read-only from `ZIRCON_ASSETS_DIR`
(default `~/zircon-assets`).

## Tests

`cargo test --workspace`. Tests that need the real assets skip themselves
unless `ZIRCON_ASSETS` is set.

## Where the rules come from

Timings, formulas and formats were transcribed from the C# code:
`Client/Models/MirLibrary.cs` (`.Zl`), `Client/Scenes/Views/MapControl.cs`
(`.map` and draw order), `LibraryCore/FrameSet.cs` (animations),
`LibraryCore/MirDB/*` (`System.db`), `ServerLibrary/Models/PlayerObject*.cs`
and `MonsterObject.cs` (movement, combat, AI). `tools/gen_monster_table.py`
regenerates the monster sprite table from `Client/Models/MonsterObject.cs`.
