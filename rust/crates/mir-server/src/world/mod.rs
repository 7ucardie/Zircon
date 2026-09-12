//! The game world: maps, objects, movement, combat, monster AI and spawning.
//!
//! Rules and timings follow Zircon's server (`ServerLibrary/Models/*`): action
//! cooldowns rather than a fixed tick, 600 ms per walk/run, 300 ms turns,
//! attack delay `max(800, 1500 - AttackSpeed * 47)`, monster AI with 3 s search,
//! 2 s roam, greedy chase, 1-cell melee. Time is milliseconds since server start.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use mir_formats::mirdb::stat;
use mir_formats::MapFile;
use mir_proto::{
    buff_type, effect, element, item_type, magic_type, parse_dialog, slot, spell_effect,
    Appearance, BeltLink, BuffSummary, Class, Direction, Gender, Good, Grid, MapDescriptor,
    ObjectId, ObjectState, PlayerStats, Point, ServerMessage, Weights, CAST_TIME, MAGIC_DELAY,
    MAGIC_RANGE, MAX_BELT,
};
use rand::Rng;

use crate::accounts::CharacterRecord;
use crate::data::{DropDef, GameData, MonsterDef, RespawnDef};
use crate::items::{can_use, default_slot, sell_price, Bag, Changed, UserItem};
use crate::magic::{UserMagic, SKILL_EXP};

pub use mir_proto::rules::{attack_delay, ATTACK_TIME, MOVE_TIME, TURN_TIME};
pub const MAX_VIEW_RANGE: i32 = 18;
pub const SEARCH_DELAY: u64 = 3000;
pub const ROAM_DELAY: u64 = 2000;
pub const DEAD_DURATION: u64 = 60_000;
pub const REGEN_DELAY: u64 = 10_000;
/// Zircon `Config.AutoReviveDelay`: forced town revive after 10 minutes;
/// the player can return to town at any time while dead.
pub const REVIVE_DELAY: u64 = 600_000;

/// Zircon `PoisonType` bits used by the prototype.
pub mod poison_kind {
    pub const GREEN: u16 = 1;
    pub const RED: u16 = 2;
    pub const SLOW: u16 = 4;
    pub const PARALYSIS: u16 = 8;
    /// Stops movement (Zircon `WraithGrip`).
    pub const WRAITH_GRIP: u16 = 16;
    /// Tick damage with no colour (Zircon `HellFire`).
    pub const HELL_FIRE: u16 = 32;
    /// Stops a monster completely.
    pub const SILENCED: u16 = 64;
    /// Shrinks a monster's view range to 2 and halves its hit chance.
    pub const ABYSS: u16 = 128;
    /// Monsters stop attacking and run.
    pub const FEAR: u16 = 1024;
    /// Roots (tick damage).
    pub const CONTAINMENT: u16 = 4096;
    /// Tick damage that stops regeneration.
    pub const HEMORRHAGE: u16 = 16384;
    pub const BINDING: u16 = 32768;
}

/// Paralysed objects take no actions.
fn paralysed(o: &Object) -> bool {
    o.poisons.iter().any(|p| p.kind == poison_kind::PARALYSIS)
}

/// Zircon slow poison: `Value * 100` ms added to every action delay.
fn slow_ms(o: &Object) -> u64 {
    o.poisons
        .iter()
        .filter(|p| p.kind == 4)
        .map(|p| p.value.max(0) as u64 * 100)
        .max()
        .unwrap_or(0)
}

/// Belt links from the character record, one entry per slot, with stale
/// item links pruned (Zircon `GetStartInformation`).
fn load_belt(stored: &[BeltLink], bag: &Bag) -> Vec<BeltLink> {
    (0..MAX_BELT as u8)
        .map(|slot| {
            let mut l = stored
                .iter()
                .find(|l| l.slot == slot)
                .copied()
                .unwrap_or(BeltLink::empty(slot));
            l.slot = slot;
            if let Some(item) = l.item {
                let present = bag
                    .inventory
                    .iter()
                    .any(|s| s.as_ref().map(|it| it.id == item).unwrap_or(false));
                if !present {
                    l.item = None;
                }
            }
            l
        })
        .collect()
}
pub const CELL_GRACE: u64 = 300;
/// Ground items vanish after this (Zircon `Config.DropDuration`, 60 min).
pub const DROP_DURATION: u64 = 60 * 60_000;
/// Other players may take a drop after this (Zircon group rule: 2 min).
pub const DROP_SHARE_AFTER: u64 = 120_000;
pub const DROP_DISTANCE: i32 = 5;
pub const PICKUP_RADIUS: i32 = 1;

pub type ConnId = u64;

/// Zircon: `max(800, AttackDelay - AttackSpeed * ASpeedRate)` milliseconds.
#[derive(Debug, Clone, Copy)]
pub struct CombatStats {
    pub accuracy: i32,
    pub agility: i32,
    pub min_ac: i32,
    pub max_ac: i32,
    pub min_dc: i32,
    pub max_dc: i32,
    pub min_mr: i32,
    pub max_mr: i32,
    pub min_mc: i32,
    pub max_mc: i32,
    pub min_sc: i32,
    pub max_sc: i32,
}

impl CombatStats {
    pub const ZERO: CombatStats = CombatStats {
        accuracy: 0,
        agility: 0,
        min_ac: 0,
        max_ac: 0,
        min_dc: 0,
        max_dc: 0,
        min_mr: 0,
        max_mr: 0,
        min_mc: 0,
        max_mc: 0,
        min_sc: 0,
        max_sc: 0,
    };
}

/// A poison instance (Zircon `Poison`).
#[derive(Debug, Clone)]
pub struct Poison {
    /// Zircon `PoisonType` bit: 1 Green, 2 Red.
    pub kind: u16,
    pub value: i32,
    pub ticks_left: i32,
    pub next_tick: u64,
    pub owner: Option<ObjectId>,
}

/// Stat changes a buff carries (Zircon `BuffInfo.Stats`, the subset used).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuffStats {
    pub dc_pct: i32,
    pub phys_def_pct: i32,
    pub mag_def_pct: i32,
    pub max_ac: i32,
    pub max_mr: i32,
    pub agility: i32,
    /// Percent of damage absorbed (Zircon `Stat.MagicShield`).
    pub magic_shield: i32,
    /// Percent of melee damage returned to monsters (Zircon `Stat.ReflectDamage`).
    pub reflect: i32,
    pub hp_pct: i32,
    pub mc_pct: i32,
    /// Zircon `Stat.PetDCPercent`.
    pub pet_dc_pct: i32,
    pub max_dc: i32,
    pub max_mc: i32,
    pub max_sc: i32,
    /// Percent of max HP restored instead of dying (Zircon `Stat.CelestialLight`).
    pub celestial: i32,
    /// HP drained every 2 s while cloaked (Zircon `Stat.CloakDamage`).
    pub cloak_damage: i32,
}

/// A timed buff on a player (Zircon `BuffInfo`).
#[derive(Debug, Clone)]
pub struct Buff {
    pub kind: u16,
    /// Server time when it ends; `u64::MAX` never.
    pub expires: u64,
    pub stats: BuffStats,
    /// Next tick for ticking buffs (Cloak drain).
    pub tick_at: u64,
}

/// Shoulder Dash in progress.
#[derive(Debug, Clone, Copy)]
pub struct Dash {
    pub remaining: i32,
    pub travelled: i32,
    pub direction: Direction,
    pub next_step: u64,
}

/// A spell object on a cell (Zircon `SpellObject`).
#[derive(Debug)]
pub struct SpellData {
    pub effect: u8,
    pub tick_count: i32,
    pub tick_frequency: u64,
    pub tick_time: u64,
    pub owner: ObjectId,
    pub magic: u16,
    /// Trap Octagon: the monsters held; the ring vanishes when none is left.
    pub targets: Vec<ObjectId>,
}

/// Zircon `BuffType.Heal`: heals `cap` per second until the pool is empty.
#[derive(Debug, Clone)]
pub struct HealBuff {
    pub pool: i32,
    pub cap: i32,
    pub next_tick: u64,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct PlayerData {
    pub conn: ConnId,
    pub account: u32,
    pub name: String,
    pub class: Class,
    pub gender: Gender,
    pub hair: u8,
    /// Account character id, for persistence.
    pub character: u32,
    pub level: i32,
    pub experience: u64,
    pub max_mp: i32,
    pub mp: i32,
    pub revive_time: u64,
    pub bind_region: i32,
    pub regen_time: u64,
    pub bag: Bag,
    pub next_item_id: u32,
    pub attack_speed: i64,
    pub max_bag: i32,
    pub max_wear: i32,
    pub max_hand: i32,
    /// Open NPC dialog: (npc object, page index).
    pub npc: Option<(ObjectId, i32)>,
    pub magics: Vec<UserMagic>,
    pub magic_time: u64,
    /// Slaying's charged power attack (Zircon `CanPowerAttack`).
    pub slaying_charged: bool,
    pub thrusting_on: bool,
    pub half_moon_on: bool,
    /// Belt links (Zircon `CharacterBeltLink`), one per slot.
    pub belt: Vec<BeltLink>,
    /// Zircon `UseItemTime`: no consumable can be used before this.
    pub use_item_time: u64,
    pub buffs: Vec<Buff>,
    /// A charged warrior power attack and when the charge expires.
    pub charge: Option<(u16, u64)>,
    /// Destructive Surge stance (Zircon `CanDestructiveSurge`).
    pub surge_on: bool,
    pub dash: Option<Dash>,
    /// Zircon `Stat.LifeSteal` percent from passives.
    pub life_steal: i32,
    pub pets: Vec<ObjectId>,
    /// Moon charges (Zircon `CanAttack` on Calamity / Waning Moon).
    pub full_moon_ready: bool,
    pub waning_moon_ready: bool,
    pub flame_splash_on: bool,
    /// Zircon `UserCurrency` amounts by `CurrencyInfo` index (gold is in the bag).
    pub currencies: HashMap<i32, i64>,
    pub rebirth: i32,
    /// Zircon `NPCVals["ROLLRESULT"]`.
    pub npc_roll: Option<i32>,
}

/// Zircon `GameNPCList` rows: named lists/values NPC scripts read and write,
/// persisted as JSON next to the accounts.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct NpcStore {
    /// category -> type value -> IntValue1.
    pub lists: HashMap<String, HashMap<String, i64>>,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl NpcStore {
    fn load(path: PathBuf) -> NpcStore {
        let mut store: NpcStore = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        store.path = Some(path);
        store
    }

    fn save(&self) {
        if let Some(p) = &self.path {
            if let Ok(json) = serde_json::to_vec_pretty(self) {
                let _ = std::fs::write(p, json);
            }
        }
    }
}

#[derive(Debug)]
pub struct MonsterData {
    pub def: i32,
    pub spawn: Option<(i32, usize)>,
    pub target: Option<ObjectId>,
    pub search_time: u64,
    pub roam_time: u64,
    pub dead_time: u64,
    pub struck_time: u64,
    pub regen_time: u64,
    pub attack_delay: u64,
    pub move_delay: u64,
    pub experience: f64,
    pub exp_owner: Option<ObjectId>,
    /// Player this monster fights for (tamed or summoned pets).
    pub owner: Option<ObjectId>,
    /// Zircon `ShockTime`: cannot move until then; any damage clears it.
    pub shock_until: u64,
    /// Zircon `SummonLevel`: +10 % stats per point.
    pub summon_level: i32,
    /// Pets return to the wild after this (`TameTime`).
    pub tame_until: u64,
    /// Puppets explode at this time (or sooner when targeted or struck).
    pub explode_at: Option<u64>,
    /// Burrowed / dormant: invisible and untouchable until something comes close.
    pub hidden: bool,
    pub hide_check: u64,
    /// Ranged attackers: earliest time for the next ranged shot.
    pub range_time: u64,
    /// Archers stop shooting until then after a scare.
    pub fear_time: u64,
    pub blink_time: u64,
    /// Next hit deals double (blink-in strike).
    pub bonus: bool,
    pub panic_used: bool,
    pub spell_time: u64,
    /// Boss summon phase (-1 = not started).
    pub stage: i32,
    pub minions: Vec<ObjectId>,
    pub master: Option<ObjectId>,
    /// Town guard (AI -1): fights wild monsters, cannot be hurt.
    pub guard: bool,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct NpcData {
    pub info: i32,
    pub entry_page: i32,
}

#[derive(Debug)]
pub struct ItemData {
    pub item: UserItem,
    /// Owning account while the drop is protected.
    pub owner: Option<u32>,
    pub spawn_time: u64,
    pub expire: u64,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Kind {
    Player(PlayerData),
    Monster(MonsterData),
    Npc(NpcData),
    Item(ItemData),
    Spell(SpellData),
}

#[derive(Debug)]
pub struct Object {
    pub id: ObjectId,
    pub kind: Kind,
    pub map: i32,
    pub location: Point,
    pub direction: Direction,
    pub hp: i32,
    pub max_hp: i32,
    pub dead: bool,
    pub stats: CombatStats,
    pub action_time: u64,
    pub move_time: u64,
    pub attack_time: u64,
    pub cell_time: u64,
    pub appearance: Appearance,
    /// Objects this one currently sees (players only).
    pub visible: HashSet<ObjectId>,
    pub poisons: Vec<Poison>,
    pub heal: Option<HealBuff>,
}

impl Object {
    pub fn is_player(&self) -> bool {
        matches!(self.kind, Kind::Player(_))
    }
    pub fn player(&self) -> Option<&PlayerData> {
        match &self.kind {
            Kind::Player(p) => Some(p),
            _ => None,
        }
    }
    pub fn player_mut(&mut self) -> Option<&mut PlayerData> {
        match &mut self.kind {
            Kind::Player(p) => Some(p),
            _ => None,
        }
    }
    pub fn monster_mut(&mut self) -> Option<&mut MonsterData> {
        match &mut self.kind {
            Kind::Monster(m) => Some(m),
            _ => None,
        }
    }
    pub fn is_monster(&self) -> bool {
        matches!(self.kind, Kind::Monster(_))
    }
    pub fn is_item(&self) -> bool {
        matches!(self.kind, Kind::Item(_))
    }
    pub fn is_spell(&self) -> bool {
        matches!(self.kind, Kind::Spell(_))
    }
    pub fn blocking(&self) -> bool {
        !self.dead && !self.is_item() && !self.is_spell()
    }
    /// The player behind an object: itself for players, the owner for pets.
    pub fn side(&self) -> Option<ObjectId> {
        match &self.kind {
            Kind::Player(_) => Some(self.id),
            Kind::Monster(m) => m.owner,
            _ => None,
        }
    }
    /// Zircon `CanAttackTarget` for the prototype (no PvP): players and
    /// their pets fight wild monsters, wild monsters fight players and pets.
    pub fn hostile_to(&self, other: &Object) -> bool {
        if other.dead || other.is_item() || other.is_spell() || matches!(other.kind, Kind::Npc(_)) {
            return false;
        }
        // Guards police wild monsters; nobody else fights their own kind.
        if let (Kind::Monster(me), Kind::Monster(them)) = (&self.kind, &other.kind) {
            if me.guard {
                return them.owner.is_none() && !them.guard;
            }
            if them.guard {
                return false;
            }
        }
        match (self.side(), other.side()) {
            (Some(_), None) => other.is_monster(),
            (None, Some(_)) => true,
            _ => false,
        }
    }
    pub fn has_buff(&self, kind: u16) -> bool {
        self.player()
            .map(|p| p.buffs.iter().any(|b| b.kind == kind))
            .unwrap_or(false)
    }
    pub fn state(&self) -> ObjectState {
        ObjectState {
            id: self.id,
            appearance: self.appearance.clone(),
            location: self.location,
            direction: self.direction,
            hp: self.hp,
            max_hp: self.max_hp,
            dead: self.dead,
        }
    }
}

#[derive(Debug)]
pub struct SpawnGroup {
    pub def: RespawnDef,
    pub points: Vec<Point>,
    pub alive: i32,
    pub next_spawn: u64,
}

#[derive(Debug)]
pub struct MapState {
    pub index: i32,
    pub file: MapFile,
    pub descriptor: MapDescriptor,
    pub spawns: Vec<SpawnGroup>,
    pub objects: Vec<ObjectId>,
    cells: HashMap<(i32, i32), Vec<ObjectId>>,
    /// Cells that trigger travel: indices into `GameData::movements`.
    movements: HashMap<(i32, i32), Vec<usize>>,
}

impl MapState {
    pub fn objects_at(&self, p: Point) -> &[ObjectId] {
        self.cells
            .get(&(p.x, p.y))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
    fn remove_from_cell(&mut self, id: ObjectId, p: Point) {
        if let Some(v) = self.cells.get_mut(&(p.x, p.y)) {
            v.retain(|o| *o != id);
        }
    }
    fn add_to_cell(&mut self, id: ObjectId, p: Point) {
        self.cells.entry((p.x, p.y)).or_default().push(id);
    }
}

/// A delayed melee hit (Zircon `DelayedAction`).
#[derive(Debug)]
struct PendingHit {
    time: u64,
    attacker: ObjectId,
    target_cell: (i32, Point),
    power: i32,
    target: Option<ObjectId>,
    /// Attack skills riding on this swing (Zircon `magics` list).
    magics: Vec<u16>,
    primary: bool,
    /// Blade Storm's delayed half: damage already computed.
    raw: bool,
    /// Elemental hits skip the dodge roll and go through MR.
    element: u8,
    /// Ranged hits do not require adjacency when they land.
    ranged: bool,
}

/// A monster spell landing after its delay: elemental damage on targets
/// and/or on whoever stands in `cells` (with a percent scale each).
#[derive(Debug)]
struct PendingMonsterSpell {
    time: u64,
    caster: ObjectId,
    targets: Vec<ObjectId>,
    cells: Vec<(Point, i32)>,
    power: i32,
    element: u8,
    map: i32,
}

/// A spell whose effect lands after its travel/cast delay (Zircon `DelayMagic`).
#[derive(Debug)]
struct PendingMagic {
    time: u64,
    caster: ObjectId,
    magic: u16,
    target: Option<ObjectId>,
    location: Point,
    /// Repulsion: direction to push.
    direction: Option<Direction>,
    /// Line spells: centre cells hit for full power, flanks for 30 %.
    primary: bool,
    /// Chain Lightning: (power divisor, cells already struck).
    chain: Option<(i32, Vec<Point>)>,
}

/// Outgoing message with routing info.
#[derive(Debug)]
pub enum Outgoing {
    To(ConnId, ServerMessage),
}

pub struct World {
    pub data: GameData,
    map_dir: PathBuf,
    /// Ordered maps so ticks iterate deterministically (seeded tests).
    pub maps: BTreeMap<i32, MapState>,
    pub objects: BTreeMap<ObjectId, Object>,
    /// Ids of every player object, kept in sync by `insert_object` /
    /// `remove_object`; ordered like `objects` so scans stay deterministic.
    players: BTreeSet<ObjectId>,
    next_id: u32,
    pub now: u64,
    /// Seeded from `ZIRCON_SEED` when set (tests), otherwise from the OS.
    rng: rand::rngs::StdRng,
    pending_hits: Vec<PendingHit>,
    pub npc_store: NpcStore,
    pending_monster_spells: Vec<PendingMonsterSpell>,
    ai_profiles: HashMap<i32, ai_profile::AiProfile>,
    pending_magics: Vec<PendingMagic>,
    /// Events raised this tick: (subject object, message).
    events: Vec<(ObjectId, ServerMessage)>,
    pub outgoing: Vec<Outgoing>,
    last_spawn_check: u64,
    force_map: Option<String>,
    drops_by_monster: HashMap<i32, Vec<DropDef>>,
}

mod ai_profile;
mod buffs;
mod combat;
mod inventory;
mod magic;
mod maps;
mod monster_ai;
mod movement;
mod npc;
mod player;
mod skills;
mod spawn;
mod spells;
mod test_api;
mod visibility;

impl World {
    /// Replace the RNG with a seeded one so a test run is reproducible.
    #[cfg(test)]
    pub fn with_seed(mut self, seed: u64) -> World {
        self.rng = rand::SeedableRng::seed_from_u64(seed);
        self
    }

    fn drops_by_monster(data: &GameData) -> HashMap<i32, Vec<DropDef>> {
        let mut drops_by_monster: HashMap<i32, Vec<DropDef>> = HashMap::new();
        for d in &data.drops {
            drops_by_monster
                .entry(d.monster)
                .or_default()
                .push(d.clone());
        }
        drops_by_monster
    }

    /// Hot reload: swap in freshly loaded game data. Live objects keep their
    /// state; new definitions apply to the next spawn, stat refresh, drop or
    /// purchase. Spawn groups of loaded maps are refreshed in place.
    pub fn reload_data(&mut self, data: GameData) {
        self.drops_by_monster = World::drops_by_monster(&data);
        self.data = data;
        for map in self.maps.values_mut() {
            for g in &mut map.spawns {
                if let Some(def) = self.data.respawns.iter().find(|r| r.index == g.def.index) {
                    g.def = def.clone();
                }
            }
        }
        let players: Vec<ObjectId> = self.players().map(|o| o.id).collect();
        for id in players {
            self.refresh_stats(id, false);
            self.send_player_stats(id);
        }
    }

    pub fn new(data: GameData, map_dir: impl AsRef<Path>, force_map: Option<String>) -> World {
        let drops_by_monster = World::drops_by_monster(&data);
        World {
            drops_by_monster,
            data,
            map_dir: map_dir.as_ref().to_path_buf(),
            maps: BTreeMap::new(),
            objects: BTreeMap::new(),
            players: BTreeSet::new(),
            next_id: 1,
            now: 0,
            rng: match std::env::var("ZIRCON_SEED")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
            {
                Some(seed) => rand::SeedableRng::seed_from_u64(seed),
                None => rand::SeedableRng::from_os_rng(),
            },
            pending_hits: Vec::new(),
            npc_store: NpcStore::default(),
            pending_monster_spells: Vec::new(),
            ai_profiles: HashMap::new(),
            pending_magics: Vec::new(),
            events: Vec::new(),
            outgoing: Vec::new(),
            last_spawn_check: 0,
            force_map,
        }
    }

    /// Persist NPC lists/values under `dir` (`npc_lists.json`).
    pub fn set_store_dir(&mut self, dir: &Path) {
        self.npc_store = NpcStore::load(dir.join("npc_lists.json"));
    }

    /// Objects on a map, in insertion order (empty when the map is not loaded).
    pub fn on_map(&self, map: i32) -> impl Iterator<Item = &Object> + '_ {
        self.maps
            .get(&map)
            .into_iter()
            .flat_map(|m| m.objects.iter())
            .map(|id| &self.objects[id])
    }

    /// Every player object, in id order.
    pub fn players(&self) -> impl Iterator<Item = &Object> + '_ {
        self.players.iter().map(|id| &self.objects[id])
    }

    fn alloc_id(&mut self) -> ObjectId {
        let id = ObjectId(self.next_id);
        self.next_id += 1;
        id
    }

    // ---- tick ----------------------------------------------------------------

    pub fn tick(&mut self, now: u64) {
        self.now = now;

        // Player timers: regen and revive.
        let players: Vec<ObjectId> = self.players().map(|o| o.id).collect();
        for id in &players {
            let (dead, revive_time, regen_due) = {
                let o = &self.objects[id];
                let p = o.player().unwrap();
                (o.dead, p.revive_time, now >= p.regen_time)
            };
            if dead {
                if now >= revive_time {
                    if let Err(e) = self.revive_player(*id) {
                        tracing::warn!("revive failed: {e}");
                    }
                }
                continue;
            }
            if regen_due {
                let o = self.objects.get_mut(id).unwrap();
                let p = o.player_mut().unwrap();
                p.regen_time = now + REGEN_DELAY;
                let mut rate = if p.class == Class::Wizard { 0.03 } else { 0.02 };
                // Rejuvenation: +0.5 % per level above the base.
                if let Some(m) = p
                    .magics
                    .iter()
                    .find(|m| m.magic == magic_type::REJUVENATION)
                {
                    rate += (0.5 + m.level as f32 * 0.5) / 100.0;
                }
                if p.mp < p.max_mp {
                    p.mp = (p.mp + (p.max_mp as f32 * rate).max(1.0) as i32).min(p.max_mp);
                    let stats = self.player_stats(&self.objects[id]);
                    self.send_to(*id, ServerMessage::StatsChanged(stats));
                }
                let o = self.objects.get_mut(id).unwrap();
                if o.hp < o.max_hp {
                    o.hp = (o.hp + (o.max_hp as f32 * 0.02).max(1.0) as i32).min(o.max_hp);
                    let (hp, max_hp) = (o.hp, o.max_hp);
                    self.events.push((
                        *id,
                        ServerMessage::HealthChanged {
                            id: *id,
                            hp,
                            max_hp,
                        },
                    ));
                    let stats = self.player_stats(&self.objects[id]);
                    self.send_to(*id, ServerMessage::StatsChanged(stats));
                }
            }
        }

        // Monsters near players are active (Zircon only processes objects with NearByPlayers).
        let player_positions: Vec<(i32, Point)> = players
            .iter()
            .map(|id| (self.objects[id].map, self.objects[id].location))
            .collect();
        let expired: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| matches!(&o.kind, Kind::Item(i) if now >= i.expire))
            .map(|o| o.id)
            .collect();
        for id in expired {
            self.remove_object(id);
        }
        let active: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| o.is_monster())
            .filter(|o| {
                o.dead
                    || player_positions
                        .iter()
                        .any(|(m, p)| *m == o.map && p.distance(o.location) <= MAX_VIEW_RANGE + 2)
            })
            .map(|o| o.id)
            .collect();
        for id in active {
            self.process_monster(id);
        }

        self.resolve_hits();
        self.resolve_magics();
        self.process_poisons();
        self.process_heals();
        self.process_buffs();
        self.process_charges();
        self.process_dashes();
        self.process_spells();
        self.process_monster_spells();

        if now >= self.last_spawn_check + 1000 {
            self.last_spawn_check = now;
            let maps: Vec<i32> = self.maps.keys().copied().collect();
            for m in maps {
                self.do_spawns(m);
            }
        }

        self.update_visibility();
        self.flush_events();
    }
}

impl Object {
    #[cfg(test)]
    pub fn monster_def(&self) -> i32 {
        self.monster_ref().def
    }
    fn monster_ref(&self) -> &MonsterData {
        match &self.kind {
            Kind::Monster(m) => m,
            _ => panic!("not a monster"),
        }
    }
}
