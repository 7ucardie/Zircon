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

Chat follows Zircon's prefixes: `Enter` opens the chat bar, `Enter` sends
and `Esc` closes it. Plain text talks to players in view range (with a
bubble over your head), `/name text` whispers, `!text` shouts to the whole
map (level 2, once per 10 s), `!@text` shouts to every player (level 33,
once per 30 s) and `!!text` goes to your group. Lines are coloured by kind
(white talk, yellow shout, green whispers, cyan group, orange global, red
system); `ZIRCON_AUTO_CHAT=text` says something two seconds after entry.

Groups follow Zircon's `PlayerObject.Social`: `P` opens the group window
with an "Allow group" toggle (invites are refused while it is off), a name
box with Invite, Kick buttons for the leader and Leave. The invitee gets a
prompt (Accept / Decline); the first member leads, groups hold 15 and a
group left with one member dissolves. Members on the same map within 18
cells share kills: the living ones split the experience (+6% per member,
weighted by level) and each rolls the drops with the chance and gold
divided by the member count, owning what they roll. `!!text` is group
chat. `ZIRCON_AUTO_GROUP=name` invites that player after entry and
`ZIRCON_AUTO_GROUP_ACCEPT=1` accepts any invite.

Storage (`B`, or `ZIRCON_OPEN_STORAGE=1`) is the account's 100-slot bank,
shared by its characters and kept in `accounts.json`: pick up a bag or
equipment item and click a storage slot to store it, or the reverse to take
it out. As in Zircon it only works inside a safe zone (`SafeZoneInfo`
regions), items flagged `CanStore = false` are refused and withdrawing
never checks weight. Trading (`T`) asks the player standing in the cell in
front of you who faces you; they get an Accept / Decline prompt. In the
trade window right-click bag items to offer them (15 at most, `CanTrade`
only), type a gold amount and press Set gold (it can only be raised), then
both press Confirm; any change to an offer clears both confirmations, a
side without enough free bag slots is unlocked to make room, and a step,
turn, death or disconnect closes the trade.

PvP uses Zircon's attack modes, cycled with `H` (Peaceful, Group, Guild,
War/Red/Brown, All) and shown in the status line: outside safe zones your
swings and spells that go through the hostility check hit other players
according to the mode. Hitting an innocent player turns your name brown
for a minute; killing one adds 50 PK points (yellow name at 50, red at
200, one point fades per minute) and red names are attacked by guards.
See `docs/research/pvp.md` for what the C# does beyond that.

Guilds (`G`) follow Zircon's `PlayerObject.Social`: founding one costs
7,500,000 gold plus 1,000,000 per member slot (the window buys ten) and
needs a unique 2-15 character alphanumeric name; the founder is "Guild
Leader" with every permission, joiners get the default rank "New Member".
Members with the AddMember permission invite by name (the invitee gets a
prompt), leaders kick, set ranks and permissions, the tax (a cut of every
gold pickup that feeds the guild funds) and buy member slots from the
funds; anyone with EditNotice sets the notice. `!~text` is guild chat,
the guild and rank show under the player's name, and the Guild attack
mode spares guild mates. Guilds persist in `guilds.json` under the data
directory; see `docs/research/guilds.md` for the rest of the C# rules.

Mail (`M`) works like Zircon's: compose to a character name (offline is
fine) with a subject, message, gold and up to five bag items (right-click
them while composing; items need a safe zone), one mail per ten seconds.
Mail arrives live or waits in the account's box in `mail.json`; unread
mails show yellow, attachments are taken one by one in a safe zone with
bag room, and a mail can be deleted once it is empty. Recipients' boxes
hold fifty attachments.

Horses come from NPC pages with Zircon's `ChangeHorse` action (the horse
dealers): owning one adds bag weight and, from the white horse up,
attack and defence. `R` mounts and dismounts on maps that allow horses;
riding runs three cells a step but you cannot attack, cast, toggle
stances or use items from the saddle, and a push, death or a map that
forbids horses dismounts you. The horse draws under the rider with the
real horse libraries, horse armour picks the skin. Marriage runs through
the NPCs: the `Marriage` action proposes to the player facing you (both
level 22 with 500,000 gold, paid on acceptance), the wedding-ring page
turns a bag ring into the wedding ring (right-click it), the character
window shows the partner and a "To partner" button that teleports you
next to them once every two minutes while the ring is on, and `Divorce`
ends it. See `docs/research/mounts-marriage.md`.
Mining and fishing follow Zircon's `PlayerObject.Movement`: with a Pick
Axe equipped (level 20) click an adjacent wall on a `CanMine` map
(Deserted Mine, Quartz Mine, Dragon Abyss) to swing at it; every
`MineInfo` row rolls its 1-in-N for an ore, the pickaxe loses 4
durability and rubble piles up under you. Fishing casts at a fishing zone
within throw range with a rod and robe on: click the water, the float
sits, and once a fish nibbles click to reel; each reel adds points, each
miss removes some, 50 lands a catch from the zone's table and 0 loses it.
This asset pack ships no fishing zones, rods, robes or bait, so
`ZIRCON_DEV_FISHING=<item index>` (server side; set it on the client too
to click without a rod) makes every unwalkable cell fishable for that
item. See `docs/research/fishing-mining.md`.
Weapon refining follows Zircon's `NPCRefine`: at an NPC page of the Refine
dialog type a window opens under the dialog; pick the refine type (DC,
durability, spell power or an element) and a quality (Rush to Precise,
waiting one minute to one day), right-click black iron ore (up to five),
common jewellery (three) and a refine special (one) in the bag to add them,
and press Refine to pay 50,000 gold and send the equipped weapon into the
furnace. The chance is 60% minus 5% per previous refine, plus 1% per 2,000
ore durability and per 6 jewellery levels and 25% per superior item, under
a ceiling of 90% adjusted by quality. A RefineRetrieve page lists the
weapons and hands them back once ready: a success adds the stat (shown in
the tooltip as refined) and a level, a failure returns the weapon as it
was. Pending refines persist with the character.

Companions come from a CompanionManage page (`docs/research/refining-
companions.md`): adopt one of the `CompanionInfo` looks for its price with
a name (locked looks need their unlock item), then bring it out, store it
or release it there. A companion follows you, teleports after you when it
falls behind, and picks up your own drops within eight cells into its bag
(gold goes straight to you); its bag size and weight come from
`CompanionLevelInfo` (level one carries nothing), it gains experience once
a minute and levels up, and it loses one hunger a minute outside safe
zones, refusing to gather when starving until fed with a consumable that
has the companion hunger stat. `N` opens the companion window with its
level, hunger and bag; Take moves an item into yours.

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
- Wave four (all classes): passives Defensive Mastery, Physical Immunity,
  Magic Immunity, Advent Of Demon, Advent Of Devil, Vitality, Last Stand,
  Fatal Blow, Dual Weapon Skills, Massacre; self bursts Seismic Slam,
  Taecheon Sword, Fire Sword, Thunder Strike, Ice Breaker, Frozen Dragon,
  Heavenly Sky, Poison Cloud, Four Wheels, Crescent Moon, Flash Of Light;
  targets Ice Dragon, Searing Light, Hemorrhage, Abyss, Containment,
  Neutralize, Parasite; cell areas Ice Rain, Asteroid, Life Steal; self
  buffs Invincibility, Evasion, Raging Wind, Concentration, The New
  Beginning, Judgement Of Heaven, Superior Magic Shield, Dark Conversion,
  Spiritualism.
- Wave five: Defensive Blow and Offensive Blow (12 s charges; the latter
  shoves, paralyses and silences), Crushing Wave (12-cell line with
  flanks), Fire Bounce and Lightning Strike (bounce to nearby monsters),
  Ice Aura (paralysing field on the line), Burning Fire (mines), Dark Soul
  Prison (dark field), Summon Demonic Creature, Demon Explosion and Demonic
  Recovery, Thunder Kick (push then strike), Dance Of Swallow (blink
  behind and strike), Hundred Fist (straight dash and shove), Dragon
  Repulse (6 s repelling channel paid in HP and MP), Elemental Swords
  (five auto-firing swords), Binding Talisman and Brain Storm, Improved
  Explosive Talisman; augments Augment Defiance, Augment Reflect Damage,
  Augment Destructive Surge, Advanced Potion Mastery, Empowered Healing,
  Stealth, Art Of Shadows, Dragon Wave.
- Wave six: Elemental Hurricane (channelled 8-cell beam with flanks, ticking
  every half second until you move, turn, are struck or run dry), Mirror
  Image (a Dark Stone decoy), Frost Bite (banks the damage you take and
  bursts it on monsters within 3 when it ends), Tornado (a 10 s summon),
  Cursed Doll (a doll that forwards what it suffers to its victim), Soul
  Resonance (linked group members die together), Corpse Exploder and
  Summon Dead (target a corpse; click one), Dragon Blood (poisoned blades),
  Chain (tethers monsters within 2 of the target to it) with Chain Of
  Fire, and the passives Shuriken (thrown attacks with a shuriken weapon),
  Burning, Shocked, Augment Poison Dust, Infection and Magic Combustion.
  Zircon's ~174 player magics are now all present; see
  `docs/research/skills-remaining.md` for the simplifications.

Buff icons with their remaining time sit top-right. Other skills show in the
window but cannot be cast yet.

Developer automation (also used for visual checks):
`ZIRCON_SCREENSHOT=out.png:5` saves a frame after 5 s and exits;
`ZIRCON_HEADLESS=1` renders to an offscreen texture instead of the window
(screenshots then work even with the screen locked, where macOS stalls the
swapchain);
`ZIRCON_AUTOLOGIN=email:password` logs in, creating the account if missing;
`ZIRCON_AUTOSTART=name` enters the world with that character, creating a
warrior if needed; `ZIRCON_OPEN_CREATE=1` opens the character creation dialog;
`ZIRCON_OPEN_WINDOWS=1` opens the bag and character windows on entry;
`ZIRCON_OPEN=storage,group,guild,mail,quests,inventory,character,skills`
opens any set of windows; server side `ZIRCON_DAY_TIME=0.1` pins the
daylight (night screenshots);
`ZIRCON_AUTO_NPC=name` walks to that NPC and opens its dialog;
`ZIRCON_AUTOCLASS=wizard|taoist|assassin` picks the auto-created class;
`ZIRCON_OPEN_SKILLS=1` opens the skill window; `ZIRCON_AUTO_CAST=<F key>`
casts that key's spell at the nearest monster (`m<id>` casts by magic id, e.g.
`m216` for Fire Wall; self casts such as `m415` Summon Puppet need no monster); `ZIRCON_AUTO_BELT=1` links every
consumable type in the bag to the belt and `ZIRCON_AUTO_USE=<slot>` presses
that belt key once. Server side, `ZIRCON_DEV_LEVEL=n` starts new characters at
level n, `ZIRCON_DEV_SKILLS=1` grants every class skill the level allows, and
`ZIRCON_DEV_ITEMS="Bronze Helmet;Healing Potion*5"` hands out (and wears)
the named items on entry, `ZIRCON_DEV_HORSE=2` gives a horse (Zircon
`HorseType`), `ZIRCON_DEV_START=2:150,182` starts new characters on that
map file and cell; client side `ZIRCON_AUTO_MOUNT=1` mounts after entry.

`cargo run -p mir-server -- --inspect` prints the start map, player stats and
spawn table without opening a port. `--map <file>` forces a start map.
`ZIRCON_SEED=n` seeds the server's random rolls (the tests use a fixed seed and
ordered object maps, so a failing roll reproduces).

## Light and sound

The map is lit like Zircon's light layer: an offscreen target is cleared to
the map's darkness (`MapInfo.Light`: day cycle from the server's daylight,
night, twilight or full light; blood red while dead) and additive light
blobs are added for lit objects (torch light from equipment, a minimum
glow around your own character, NPCs, fire walls and other fields), then
the target is multiplied over the world before names and UI are drawn.
The server broadcasts the daylight (`ZIRCON_DAY_CYCLE` game days per real
day, default 12).

Sound follows Zircon's `SoundIndex` tables, generated from the C# client by
`tools/gen_sound_table.py` into `mir-client/src/sound_table.rs` (index to
file and channel, monster attack/struck/die by `MonsterImage`, magic
cast/travel/impact sounds, attack-skill sounds). The client plays login and
character-select music, each map's music on entry, footsteps on walk and
run frames of your own character, weapon swings by weapon shape, player
and monster struck and death sounds, spell casts with their projectile and
impact sounds, teleport and lotus effects, the fire wall and tempest hums
while one is within 20 cells, item cell sounds by item type, gold gained,
quest accepted and completed, and NPC dialog links. Files are looked up
case-insensitively under `Sound/`; missing files are silent (21 indices of
this asset pack have no file, see `docs/research/sound.md`). Channels
(system, music, magic, monster, player) have their own volume;
`ZIRCON_VOLUME=0.5` scales all of them and `ZIRCON_MUTE=1` silences
everything (also used by the screenshot runs).

## NPC scripts

Dialog pages run Zircon's checks and actions: levels, class, gold, items,
random rolls, currencies by name (`CurrencyInfo`), named data lists and
values (`GameNPCList`, persisted in `npc_lists.json` under `--data`) and
rebirth at level 86 + rebirths, weapon level/element/added stats from
refining, horses and marriage. `NPCRequirement` rows hide an NPC from
players who do not meet them. Fame and Lua script actions have no system
behind them yet.

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

Wave two adds the timed class behaviours the first pass skipped: Voracious
Ghosts revive up to three times with half their HP each time and only drop
loot at the last death, Crimson Necromancers strip the magic resistance of
everything within three cells every ten seconds, Jinchon Devils lay death
clouds around their targets that burst after a few seconds, and the
Netherworld and Jinam gates stand invulnerable for twenty minutes, sweeping
players in view to the region named by `ZIRCON_MYSTERY_SHIP_REGION` /
`ZIRCON_LAIR_REGION` (Zircon's `Config` region indices; unset = no
teleport). Wave three covers the odd bosses: Shinsu fights only inside ten
second windows and is unseen between them, Terracotta warriors rush in
invisible and show themselves two cells from their target (the sub boss
fades out again when far), and Doom Claw stands still while its claws,
maw and wave strike whoever stands in each part's reach with class-reduced
damage and pushes, ignoring attackers more than ten cells away.
`cargo test -p mir-server report_monster_ai -- --ignored --nocapture`
prints which AI ids still fall back to the plain melee profile.

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

## Multiplayer hardening

The server follows Zircon's `SConnection` limits: a client that sends more
than 50 messages in one tick or 200 frames in a second is dropped, frames
over 64 KiB are refused before they are read, strings and lists in every
message are size-checked (oversized ones disconnect), a connection that
stays silent for 20 s (the client pings every 5 s) or never says hello
within 10 s is closed, outbound queues are bounded so a stalled client is
dropped instead of buffered forever, and there are caps of 1000
connections and 32 per address. `SIGTERM`/`Ctrl-C` saves every character
before exit and `SIGHUP` reloads `System.db`. `cargo test -p mir-server
--test load` (needs `ZIRCON_ASSETS`) starts the real binary, logs in a
dozen bots that walk and chat at once, checks that a flooder and an
oversized frame are dropped without disturbing them, and that `SIGTERM`
saves everything.

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
