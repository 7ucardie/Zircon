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
cast. Implemented with Zircon's power, cost, cooldown, skill-experience rules
and the original effect sprites:

- Warrior: Swordsmanship, Potion Mastery, Slaying, Thrusting, Half Moon,
  Shoulder Dash (dash that shoves weaker monsters aside), Flaming Sword,
  Dragon Rise and Blade Storm (12 s charges consumed by the next swing),
  Destructive Surge (stance hitting all eight neighbours), Defiance and Might
  (self buffs, mutually exclusive), Interchange (swap places), Beckon and
  Mass Beckon (pull and paralyse), Swift Blade (sure-hit slash), Assault
  (dash that stuns), Endurance (poison immunity), Reflect Damage, Fetter
  (slow).
- Wizard: Fire Ball, Lightning Ball, Ice Bolt, Gust Blast, Repulsion,
  Teleportation, Adamantine Fire Ball, Thunder Bolt, Ice Blades (slow),
  Cyclone, Scorched Earth, Lightning Beam, Frozen Earth, Blow Earth (lines
  of 8 cells with 30 % flanks), Fire Wall (burning cells for 20-60 s), Magic
  Shield (halves damage, drains with hits), Electric Shock (tame), Expel
  Undead, Geo Manipulation, Fire Storm, Lightning Wave, Ice Storm, Dragon
  Tornado, Greater Frozen Earth, Chain Lightning (hops), Meteor Shower,
  Renounce (MC for HP), Tempest (wind cells that repel).
- Taoist: Heal, Spirit Sword, Poison Dust, Explosive Talisman (uses a
  talisman), Evil Slayer and Greater Evil Slayer (holy talisman bonus), Magic
  Resistance and Resilience (area buffs costing talismans), Mass Heal,
  Summon Skeleton, Summon Shinsu and Summon Jin Skeleton (pets that follow,
  fight and return when recalled), Strength Of Faith (pet DC), Invisibility,
  Mass Invisibility and Transparency (monsters lose hidden targets), Trap
  Octagon (shock ring), Combat Kick, Elemental Superiority, Blood Lust,
  Resurrection, Purification, Celestial Light (survive a killing blow).
- Assassin: Willow Dance, Vine Tree Dance, Discipline, Bloody Flower (life
  steal), Poisonous Cloud (agility cloud), Full Bloom, White Lotus, Red Lotus
  and Sweetbrier (press the key to arm the next swing; each opens the next
  in the chain), Flaming Daggers, Shredding, Cloak (HP-priced stealth that
  drains every 2 s, breaks on running, swinging or casting; Pledge Of Blood
  cuts the drain, Ghost Walk rolls a deeper stealth), Calamity Of Full Moon
  and Waning Moon (self-arming power swings, out of and inside the cloak),
  Wraith Grip (roots; Touch Of The Departed adds paralysis), Hell Fire
  (fire strike plus burn), Rake (from the cloak, guaranteed slow), Summon
  Puppet (decoys that explode, then blink away cloaked), Karma (armed
  execute from the cloak costing HP; Resolution and Release scale it),
  Flame Splash (toggle: up to four extra directions per swing),
  Rejuvenation (faster regeneration).

Buff icons with their remaining time sit top-right. Other skills show in the
window but cannot be cast yet.

Developer automation (also used for visual checks):
`ZIRCON_SCREENSHOT=out.png:5` saves a frame after 5 s and exits;
`ZIRCON_AUTOLOGIN=email:password` logs in, creating the account if missing;
`ZIRCON_AUTOSTART=name` enters the world with that character, creating a
warrior if needed; `ZIRCON_OPEN_CREATE=1` opens the character creation dialog;
`ZIRCON_OPEN_WINDOWS=1` opens the bag and character windows on entry;
`ZIRCON_AUTO_NPC=name` walks to that NPC and opens its dialog;
`ZIRCON_AUTOCLASS=wizard|taoist|assassin` picks the auto-created class;
`ZIRCON_OPEN_SKILLS=1` opens the skill window; `ZIRCON_AUTO_CAST=<F key>`
casts that key's spell at the nearest monster (`m<id>` casts by magic id, e.g.
`m216` for Fire Wall; self casts such as `m415` Summon Puppet need no monster); `ZIRCON_AUTO_BELT=1` links every
consumable type in the bag to the belt and `ZIRCON_AUTO_USE=<slot>` presses
that belt key once. Server side, `ZIRCON_DEV_LEVEL=n` starts new characters at
level n, `ZIRCON_DEV_SKILLS=1` grants every class skill the level allows, and
`ZIRCON_DEV_ITEMS="Bronze Helmet;Healing Potion*5"` hands out (and wears)
the named items on entry.

`cargo run -p mir-server -- --inspect` prints the start map, player stats and
spawn table without opening a port. `--map <file>` forces a start map.
`ZIRCON_SEED=n` seeds the server's random rolls (the tests use a fixed seed and
ordered object maps, so a failing roll reproduces).

## NPC scripts

Dialog pages run Zircon's checks and actions: levels, class, gold, items,
random rolls, currencies by name (`CurrencyInfo`), named data lists and
values (`GameNPCList`, persisted in `npc_lists.json` under `--data`) and
rebirth at level 86 + rebirths. `NPCRequirement` rows hide an NPC from
players who do not meet them. Weapon refining, horses, marriage, fame and
Lua script actions wait for their systems.

## Quests

NPC dialogs list the quests an NPC starts (`[Accept]`) or finishes
(`[Complete]`, or "in progress"); the quest log opens with `L` and shows
the progress text with Zircon's name tags filled in, each task's count and
the class-filtered rewards. Requirements (level, class, other quests
completed or not), kill and gather tasks (`QuestTaskMonsterDetails` with map
filter and chance roll, capped at the task amount), region tasks, choice
rewards, experience and currency rewards, daily/weekly/repeatable resets
and abandon follow the C# rules; gather tasks credit on the kill instead of
dropping a quest item to pick up.

## Monster AI

`MonsterInfo.AI` selects a behaviour profile (`world/ai_profile.rs`, ported
from Zircon's `MonsterRegistrations`): town guards (immobile, untouchable,
kill wild monsters on sight), passive farm animals and trees, ranged
spitters and archers (with kiting and self-scare), ray-limited line
attacks, splash and self-area hits, caster classes (Thunder Bolt, Fire Ball,
mass lightning, beams, storms and the Sama guardian kits), poison-on-hit
tables (green, red, paralysis, silence, abyss), blink-strikers with a
one-time panic teleport, burrowers and ambushers that stay invisible until
prey is close, suicide larvae, corpse spawners and boss summon phases.
Elemental monster hits ignore dodge and go through MR; monsters that lose
their target re-search every half second like Zircon does every tick.

## Editing content

`cargo run -p mir-editor` opens the data editor (egui): every `System.db`
collection as a sortable, filterable table with typed cells; a detail form for
the selected row (complex values as text: stats `2:10;8:3`, arrays `1,2,3`,
points `x,y`); item, magic, NPC and monster sprite previews; links from
foreign keys (Item, Monster, Map, Region, Page...) and a list of every record
that points back at the selected one (drops, stats, shop goods). Add,
Duplicate, Delete, Undo (Cmd+Z), Save (Cmd+S). Validate runs the server's
own loader plus consistency checks (dangling monster/item/region references,
spawn regions with no walkable cell, books without a magic) on the unsaved
data and links each finding to its row. Saving writes a
`System.db.bak-<timestamp>` copy first; a running `mir-server` notices the
changed file within 2 s and reloads it (SIGHUP forces a reload on unix), so
an edited potion heals for the new amount on its next use without a restart.
`--db FILE` edits another database; `ZIRCON_SCREENSHOT`,
`ZIRCON_EDITOR_COLLECTION`, `ZIRCON_EDITOR_FILTER`, `ZIRCON_EDITOR_ROW` and
`ZIRCON_EDITOR_VALIDATE` automate a visual check.

Underneath, `mir-formats` writes as well as reads: `MirDb::save`,
`MapFile::save` and `ZlBuilder::save` re-encode `System.db`, `.map` and `.Zl`
files (the shipped files round-trip byte for byte in the tests) and leave a
`.bak-<timestamp>` copy of the previous file. New sprites go in through
`ZlBuilder::push_rgba` (DXT1/DXT5 encoding). The rest of the editor plan is in
`docs/EDITOR_PLAN.md`.

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
