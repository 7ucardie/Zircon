//! The game world: maps, objects, movement, combat, monster AI and spawning.
//!
//! Rules and timings follow Zircon's server (`ServerLibrary/Models/*`): action
//! cooldowns rather than a fixed tick, 600 ms per walk/run, 300 ms turns,
//! attack delay `max(800, 1500 - AttackSpeed * 47)`, monster AI with 3 s search,
//! 2 s roam, greedy chase, 1-cell melee. Time is milliseconds since server start.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use mir_formats::mirdb::stat;
use mir_formats::MapFile;
use mir_proto::{
    element, item_type, magic_type, parse_dialog, slot, Appearance, BeltLink, Class, Direction,
    Gender, Good, Grid, MapDescriptor, ObjectId, ObjectState, PlayerStats, Point, ServerMessage,
    Weights, CAST_TIME, MAGIC_DELAY, MAGIC_RANGE, MAX_BELT,
};
use rand::Rng;

use crate::accounts::CharacterRecord;
use crate::data::{DropDef, GameData, MonsterDef, RespawnDef};
use crate::items::{can_use, default_slot, sell_price, Bag, Changed, UserItem};
use crate::magic::{UserMagic, SKILL_EXP};

pub const MOVE_TIME: u64 = 600;
pub const TURN_TIME: u64 = 300;
pub const ATTACK_TIME: u64 = 600;
pub const ATTACK_DELAY: u64 = 1500;
pub const ASPEED_RATE: u64 = 47;
pub const MAX_VIEW_RANGE: i32 = 18;
pub const SEARCH_DELAY: u64 = 3000;
pub const ROAM_DELAY: u64 = 2000;
pub const DEAD_DURATION: u64 = 60_000;
pub const REGEN_DELAY: u64 = 10_000;
/// Zircon `Config.AutoReviveDelay`: forced town revive after 10 minutes;
/// the player can return to town at any time while dead.
pub const REVIVE_DELAY: u64 = 600_000;

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
pub fn attack_delay(attack_speed: i64) -> u64 {
    (ATTACK_DELAY as i64 - attack_speed * ASPEED_RATE as i64).max(800) as u64
}

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
pub enum Kind {
    Player(PlayerData),
    Monster(MonsterData),
    Npc(NpcData),
    Item(ItemData),
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
    pub fn blocking(&self) -> bool {
        !self.dead && !self.is_item()
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
    fn objects_at(&self, p: Point) -> &[ObjectId] {
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
}

/// Outgoing message with routing info.
#[derive(Debug)]
pub enum Outgoing {
    To(ConnId, ServerMessage),
}

pub struct World {
    pub data: GameData,
    map_dir: PathBuf,
    pub maps: HashMap<i32, MapState>,
    pub objects: HashMap<ObjectId, Object>,
    next_id: u32,
    pub now: u64,
    rng: rand::rngs::ThreadRng,
    pending_hits: Vec<PendingHit>,
    pending_magics: Vec<PendingMagic>,
    /// Events raised this tick: (subject object, message).
    events: Vec<(ObjectId, ServerMessage)>,
    pub outgoing: Vec<Outgoing>,
    last_spawn_check: u64,
    force_map: Option<String>,
    drops_by_monster: HashMap<i32, Vec<DropDef>>,
}

impl World {
    pub fn new(data: GameData, map_dir: impl AsRef<Path>, force_map: Option<String>) -> World {
        let mut drops_by_monster: HashMap<i32, Vec<DropDef>> = HashMap::new();
        for d in &data.drops {
            drops_by_monster
                .entry(d.monster)
                .or_default()
                .push(d.clone());
        }
        World {
            drops_by_monster,
            data,
            map_dir: map_dir.as_ref().to_path_buf(),
            maps: HashMap::new(),
            objects: HashMap::new(),
            next_id: 1,
            now: 0,
            rng: rand::rng(),
            pending_hits: Vec::new(),
            pending_magics: Vec::new(),
            events: Vec::new(),
            outgoing: Vec::new(),
            last_spawn_check: 0,
            force_map,
        }
    }

    fn alloc_id(&mut self) -> ObjectId {
        let id = ObjectId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Load a map (and register its spawn groups) if not already loaded.
    pub fn ensure_map(&mut self, index: i32) -> anyhow::Result<()> {
        if self.maps.contains_key(&index) {
            return Ok(());
        }
        let def = self
            .data
            .maps
            .get(&index)
            .ok_or_else(|| anyhow::anyhow!("unknown map {index}"))?
            .clone();
        let path = self.map_dir.join(format!("{}.map", def.file_name));
        let file =
            MapFile::load(&path).map_err(|e| anyhow::anyhow!("loading {}: {e}", path.display()))?;
        let width = file.width as i32;
        let mut spawns = Vec::new();
        for r in self.data.respawns.iter().filter(|r| !r.event_spawn) {
            let Some(region) = self.data.regions.get(&r.region) else {
                continue;
            };
            if region.map != index {
                continue;
            }
            if !self.data.monsters.contains_key(&r.monster) {
                continue;
            }
            let points: Vec<Point> = region
                .points(width)
                .into_iter()
                .filter(|&(x, y)| file.is_walkable(x, y))
                .map(|(x, y)| Point::new(x, y))
                .collect();
            if points.is_empty() {
                continue;
            }
            spawns.push(SpawnGroup {
                def: r.clone(),
                points,
                alive: 0,
                next_spawn: 0,
            });
        }
        tracing::info!(
            map = index,
            name = def.description,
            file = def.file_name,
            size = format!("{}x{}", file.width, file.height),
            walkable = file.walkable_count(),
            spawn_groups = spawns.len(),
            monsters = spawns.iter().map(|s| s.def.count).sum::<i32>(),
            "map loaded"
        );
        let mut movements: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, m) in self.data.movements.iter().enumerate() {
            let Some(src) = self.data.regions.get(&m.source_region) else {
                continue;
            };
            if src.map != index {
                continue;
            }
            for p in src.movement_points(width) {
                movements.entry(p).or_default().push(i);
            }
        }
        self.maps.insert(
            index,
            MapState {
                index,
                file,
                descriptor: MapDescriptor {
                    file: def.file_name.clone(),
                    name: def.description.clone(),
                },
                spawns,
                objects: Vec::new(),
                cells: HashMap::new(),
                movements,
            },
        );
        self.do_spawns(index);
        self.spawn_npcs(index);
        Ok(())
    }

    fn random_point(&mut self, points: &[Point]) -> Option<Point> {
        if points.is_empty() {
            return None;
        }
        Some(points[self.rng.random_range(0..points.len())])
    }

    /// Pick the start map/cell for a new character (Zircon `SetBindPoint` + `Spawn(BindRegion)`).
    fn start_location(&mut self, class: Class) -> anyhow::Result<(i32, Point, i32)> {
        if let Some(file) = self.force_map.clone() {
            let map = self
                .data
                .map_by_file(&file)
                .ok_or_else(|| anyhow::anyhow!("--map {file}: no such map in System.db"))?
                .index;
            self.ensure_map(map)?;
            let m = &self.maps[&map];
            let (w, h) = (m.file.width as i32, m.file.height as i32);
            for _ in 0..10_000 {
                let p = Point::new(self.rng.random_range(0..w), self.rng.random_range(0..h));
                if self.maps[&map].file.is_walkable(p.x, p.y) && !self.cell_blocked(map, p, true) {
                    return Ok((map, p, 0));
                }
            }
            anyhow::bail!("no walkable cell found on {file}");
        }
        let flag = class.flag();
        let zones: Vec<(i32, i32)> = self
            .data
            .start_zones(flag)
            .iter()
            .filter_map(|z| {
                self.data
                    .regions
                    .get(&z.bind_region)
                    .map(|r| (z.bind_region, r.map))
            })
            .collect();
        if zones.is_empty() {
            anyhow::bail!("no start zone for {class:?}");
        }
        let (bind_region, map) = zones[self.rng.random_range(0..zones.len())];
        self.ensure_map(map)?;
        let point = self.bind_point(map, bind_region)?;
        Ok((map, point, bind_region))
    }

    fn bind_point(&mut self, map: i32, bind_region: i32) -> anyhow::Result<Point> {
        let width = self.maps[&map].file.width as i32;
        let points: Vec<Point> = self.data.regions[&bind_region]
            .points(width)
            .into_iter()
            .map(|(x, y)| Point::new(x, y))
            .filter(|p| self.maps[&map].file.is_walkable(p.x, p.y))
            .collect();
        for _ in 0..20 {
            if let Some(p) = self.random_point(&points) {
                if !self.cell_blocked(map, p, true) {
                    return Ok(p);
                }
            }
        }
        self.random_point(&points)
            .ok_or_else(|| anyhow::anyhow!("bind region {bind_region} has no walkable cells"))
    }

    /// Enter the world with a saved character. Spawns at the saved map/cell
    /// when it is walkable, otherwise at a start zone for the class.
    pub fn add_player(
        &mut self,
        conn: ConnId,
        account: u32,
        rec: &CharacterRecord,
    ) -> anyhow::Result<ObjectId> {
        let class = rec.class;
        let mut placed = None;
        if !rec.map.is_empty() && self.force_map.is_none() {
            if let Some(m) = self.data.map_by_file(&rec.map).map(|m| m.index) {
                if self.ensure_map(m).is_ok()
                    && self.maps[&m]
                        .file
                        .is_walkable(rec.location.x, rec.location.y)
                    && !self.cell_blocked(m, rec.location, true)
                {
                    placed = Some((m, rec.location));
                }
            }
        }
        let (start_map, start_loc, bind_region) = self.start_location(class)?;
        let (map, location) = placed.unwrap_or((start_map, start_loc));
        // Developer aid: ZIRCON_DEV_LEVEL starts fresh characters at that level.
        let dev_level = std::env::var("ZIRCON_DEV_LEVEL")
            .ok()
            .and_then(|v| v.parse::<i32>().ok())
            .filter(|_| rec.last_login == 0);
        let level = dev_level.unwrap_or(rec.level).max(1);
        let base = self
            .data
            .base_stat(class.mir_class(), level)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no BaseStat rows for {class:?}"))?;
        let hp = if rec.hp > 0 {
            rec.hp.min(base.health)
        } else {
            base.health
        };
        let mp = if rec.mp > 0 {
            rec.mp.min(base.mana)
        } else {
            base.mana
        };
        let id = self.alloc_id();
        let bag = Bag::from_stored(&rec.items, rec.gold);
        let belt = load_belt(&rec.belt, &bag);
        let next_item_id = rec.next_item_id.max(bag.max_id());
        let obj = Object {
            id,
            kind: Kind::Player(PlayerData {
                conn,
                account,
                name: rec.name.clone(),
                class,
                gender: rec.gender,
                hair: rec.hair.max(1),
                character: rec.id,
                level,
                experience: rec.experience,
                max_mp: base.mana,
                mp,
                revive_time: 0,
                bind_region,
                regen_time: self.now + REGEN_DELAY,
                bag,
                next_item_id,
                attack_speed: 0,
                max_bag: base.bag_weight,
                max_wear: base.wear_weight,
                max_hand: base.hand_weight,
                npc: None,
                magics: rec.magics.iter().map(UserMagic::from_stored).collect(),
                magic_time: 0,
                slaying_charged: false,
                thrusting_on: false,
                half_moon_on: false,
                belt,
                use_item_time: 0,
            }),
            map,
            location,
            direction: rec.direction,
            hp,
            max_hp: base.health,
            dead: false,
            stats: CombatStats::ZERO,
            action_time: 0,
            move_time: 0,
            attack_time: 0,
            cell_time: 0,
            appearance: Appearance::Player {
                name: rec.name.clone(),
                gender: rec.gender,
                class,
                armour: 0,
                weapon: None,
                hair: rec.hair.max(1),
                helmet: 0,
                shield: None,
            },
            visible: HashSet::new(),
            poisons: Vec::new(),
            heal: None,
        };
        self.insert_object(obj);
        // Never had items yet (new character, or one from before items existed).
        if rec.next_item_id == 0 && rec.items.is_empty() {
            self.give_start_items(id);
        }
        // Developer aid: ZIRCON_DEV_SKILLS grants every class skill the level allows.
        if std::env::var_os("ZIRCON_DEV_SKILLS").is_some() && rec.magics.is_empty() {
            let (class, level) = (rec.class.mir_class(), level);
            let mut grant: Vec<u16> = self
                .data
                .magics
                .values()
                .filter(|m| {
                    m.class == class && m.school != 0 && m.school != 20 && m.need_level[0] <= level
                })
                .map(|m| m.magic)
                .collect();
            grant.sort();
            if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
                for (i, m) in grant.into_iter().enumerate() {
                    p.magics.push(UserMagic {
                        magic: m,
                        level: 0,
                        experience: 0,
                        key: if i < 11 { i as u8 + 1 } else { 0 },
                        cooldown_until: 0,
                    });
                }
            }
        }
        // Developer aid: ZIRCON_DEV_ITEMS="Bronze Helmet;Healing Potion*5" gives
        // the named items on entry (once per name already in the bag).
        if let Ok(list) = std::env::var("ZIRCON_DEV_ITEMS") {
            for entry in list.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                let (name, count) = match entry.split_once('*') {
                    Some((n, c)) => (n.trim(), c.trim().parse().unwrap_or(1)),
                    None => (entry, 1),
                };
                let Some(info) = self
                    .data
                    .items
                    .values()
                    .find(|d| d.name.eq_ignore_ascii_case(name))
                    .map(|d| d.index)
                else {
                    tracing::warn!("ZIRCON_DEV_ITEMS: no item named {name:?}");
                    continue;
                };
                let has = self.objects[&id]
                    .player()
                    .map(|p| p.bag.count_of(info) > 0)
                    .unwrap_or(true);
                if has {
                    continue;
                }
                if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
                    let mut next = p.next_item_id;
                    p.bag.gain(&self.data, info, count, &mut next);
                    p.next_item_id = next;
                }
                // Wearables go straight on.
                let wearable = !item_type::slots(self.data.items[&info].item_type).is_empty();
                let slot = self.objects[&id].player().and_then(|p| {
                    p.bag
                        .inventory
                        .iter()
                        .position(|s| s.as_ref().map(|i| i.info == info).unwrap_or(false))
                });
                if let (true, Some(slot)) = (wearable, slot) {
                    self.item_use(id, slot as u8);
                }
            }
        }
        self.refresh_stats(id, false);
        self.refresh_appearance(id);
        let o = &self.objects[&id];
        let stats = self.player_stats(o);
        let desc = self.maps[&map].descriptor.clone();
        self.outgoing.push(Outgoing::To(
            conn,
            ServerMessage::Welcome {
                id,
                map: desc,
                location,
                direction: rec.direction,
                stats,
            },
        ));
        self.send_inventory(id);
        self.send_belt(id);
        self.send_magics(id);
        Ok(id)
    }

    fn send_belt(&mut self, id: ObjectId) {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let belt = p.belt.clone();
        self.send_to(id, ServerMessage::BeltLinks(belt));
    }

    /// Zircon `BeltLinkChanged`: link a belt slot to an item type or a
    /// specific inventory item, or clear it.
    pub fn belt_link(&mut self, id: ObjectId, slot: u8, info: Option<i32>, item: Option<u32>) {
        if slot as usize >= MAX_BELT || (info.is_some() && item.is_some()) {
            return;
        }
        let info = info.filter(|i| self.data.items.contains_key(i));
        let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) else {
            return;
        };
        let item = item.filter(|i| {
            p.bag
                .inventory
                .iter()
                .any(|s| s.as_ref().map(|it| it.id == *i).unwrap_or(false))
        });
        p.belt[slot as usize] = BeltLink { slot, info, item };
    }

    /// Copy the live state of a player back into its character record.
    pub fn snapshot(&self, id: ObjectId, rec: &mut CharacterRecord) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let Some(p) = o.player() else {
            return;
        };
        rec.level = p.level;
        rec.experience = p.experience;
        rec.hp = if o.dead { 0 } else { o.hp };
        rec.mp = p.mp;
        rec.direction = o.direction;
        rec.items = p.bag.to_stored();
        rec.gold = p.bag.gold;
        rec.next_item_id = p.next_item_id;
        rec.magics = p.magics.iter().map(|m| m.stored()).collect();
        rec.belt = p.belt.clone();
        if let Some(m) = self.maps.get(&o.map) {
            rec.map = m.descriptor.file.clone();
            rec.location = o.location;
        }
    }

    fn player_stats(&self, o: &Object) -> PlayerStats {
        let p = o.player().expect("player");
        PlayerStats {
            level: p.level as u8,
            hp: o.hp,
            max_hp: o.max_hp,
            mp: p.mp,
            max_mp: p.max_mp,
            experience: p.experience,
            max_experience: GameData::max_experience(p.level),
            min_dc: o.stats.min_dc,
            max_dc: o.stats.max_dc,
            min_ac: o.stats.min_ac,
            max_ac: o.stats.max_ac,
            accuracy: o.stats.accuracy,
            agility: o.stats.agility,
        }
    }

    fn insert_object(&mut self, obj: Object) {
        let map = self.maps.get_mut(&obj.map).expect("map loaded");
        map.objects.push(obj.id);
        map.add_to_cell(obj.id, obj.location);
        self.objects.insert(obj.id, obj);
    }

    pub fn remove_object(&mut self, id: ObjectId) {
        if let Some(obj) = self.objects.remove(&id) {
            if let Some(map) = self.maps.get_mut(&obj.map) {
                map.objects.retain(|o| *o != id);
                map.remove_from_cell(id, obj.location);
            }
            if let Kind::Monster(m) = &obj.kind {
                if !obj.dead {
                    if let Some((map, gi)) = m.spawn {
                        if let Some(g) = self.maps.get_mut(&map).and_then(|m| m.spawns.get_mut(gi))
                        {
                            g.alive -= 1;
                        }
                    }
                }
            }
            // Everyone who saw it gets a remove.
            for other in self.objects.values_mut() {
                if other.visible.remove(&id) {
                    if let Some(p) = other.player() {
                        self.outgoing
                            .push(Outgoing::To(p.conn, ServerMessage::ObjectRemove { id }));
                    }
                }
            }
        }
    }

    /// Test helper: move an object to a cell without validation.
    #[cfg(test)]
    pub fn teleport(&mut self, id: ObjectId, to: Point) {
        self.move_object(id, to);
    }

    #[cfg(test)]
    pub fn test_hp(&self, id: ObjectId) -> (i32, i32, bool) {
        let o = &self.objects[&id];
        (o.hp, o.max_hp, o.dead)
    }

    #[cfg(test)]
    pub fn test_set_hp(&mut self, id: ObjectId, hp: i32) {
        if let Some(o) = self.objects.get_mut(&id) {
            o.hp = hp;
        }
    }

    #[cfg(test)]
    pub fn test_kill(&mut self, id: ObjectId) {
        self.player_die(id);
    }

    #[cfg(test)]
    pub fn test_belt(&self, id: ObjectId) -> Vec<BeltLink> {
        self.objects[&id].player().unwrap().belt.clone()
    }

    #[cfg(test)]
    pub fn test_set_gold(&mut self, id: ObjectId, gold: u64) {
        if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
            p.bag.gold = gold;
        }
    }

    #[cfg(test)]
    pub fn test_open_page(&mut self, id: ObjectId, page: i32) {
        if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
            p.npc = Some((ObjectId(0), page));
        }
    }

    #[cfg(test)]
    pub fn test_bag(&self, id: ObjectId) -> (u64, Vec<(i32, u32)>) {
        let p = self.objects[&id].player().unwrap();
        (
            p.bag.gold,
            p.bag
                .inventory
                .iter()
                .flatten()
                .map(|i| (i.info, i.count))
                .collect(),
        )
    }

    #[cfg(test)]
    pub fn test_give_item(&mut self, id: ObjectId, info: i32, count: u32) {
        if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
            let mut next = p.next_item_id;
            p.bag.gain(&self.data, info, count, &mut next);
            p.next_item_id = next;
        }
    }

    #[cfg(test)]
    pub fn test_slot_of(&self, id: ObjectId, info: i32) -> Option<u8> {
        self.objects[&id]
            .player()?
            .bag
            .inventory
            .iter()
            .position(|s| s.as_ref().map(|i| i.info == info).unwrap_or(false))
            .map(|i| i as u8)
    }

    #[cfg(test)]
    pub fn test_magics(&self, id: ObjectId) -> Vec<u16> {
        self.objects[&id]
            .player()
            .map(|p| p.magics.iter().map(|m| m.magic).collect())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub fn test_magic_exp(&self, id: ObjectId, magic: u16) -> u64 {
        self.objects[&id]
            .player()
            .and_then(|p| p.magics.iter().find(|m| m.magic == magic))
            .map(|m| m.experience + m.level as u64 * 1000)
            .unwrap_or(0)
    }

    #[cfg(test)]
    pub fn test_movement_cells(&self, map: i32) -> Vec<Point> {
        self.maps[&map]
            .movements
            .keys()
            .map(|(x, y)| Point::new(*x, *y))
            .collect()
    }

    fn spawn_monster(&mut self, map: i32, group: usize) -> bool {
        let (def_index, points) = {
            let g = &self.maps[&map].spawns[group];
            (g.def.monster, g.points.clone())
        };
        let def: MonsterDef = self.data.monsters[&def_index].clone();
        let mut location = None;
        for _ in 0..20 {
            if let Some(p) = self.random_point(&points) {
                if !self.cell_blocked(map, p, false) {
                    location = Some(p);
                    break;
                }
            }
        }
        let Some(location) = location else {
            return false;
        };
        let id = self.alloc_id();
        let hp = def.health();
        let dir = Direction::from_index(self.rng.random_range(0..8));
        let jitter_search = self.rng.random_range(0..SEARCH_DELAY);
        let jitter_roam = self.rng.random_range(0..ROAM_DELAY);
        let obj = Object {
            id,
            kind: Kind::Monster(MonsterData {
                def: def.index,
                spawn: Some((map, group)),
                target: None,
                search_time: self.now + jitter_search,
                roam_time: self.now + jitter_roam,
                dead_time: 0,
                struck_time: 0,
                regen_time: self.now + REGEN_DELAY,
                attack_delay: def.attack_delay.max(0) as u64,
                move_delay: def.move_delay.max(0) as u64,
                experience: def.experience,
                exp_owner: None,
            }),
            map,
            location,
            direction: dir,
            hp,
            max_hp: hp,
            dead: false,
            stats: CombatStats {
                accuracy: def.stat(stat::ACCURACY),
                agility: def.stat(stat::AGILITY),
                min_ac: def.stat(stat::MIN_AC),
                max_ac: def.stat(stat::MAX_AC),
                min_dc: def.stat(stat::MIN_DC),
                max_dc: def.stat(stat::MAX_DC),
                min_mr: def.stat(stat::MIN_MR),
                max_mr: def.stat(stat::MAX_MR),
                min_mc: 0,
                max_mc: 0,
                min_sc: 0,
                max_sc: 0,
            },
            action_time: 0,
            move_time: 0,
            attack_time: 0,
            cell_time: 0,
            appearance: Appearance::Monster {
                name: def.name.clone(),
                image: def.image,
            },
            visible: HashSet::new(),
            poisons: Vec::new(),
            heal: None,
        };
        self.insert_object(obj);
        self.maps.get_mut(&map).unwrap().spawns[group].alive += 1;
        true
    }

    /// Zircon `SpawnInfo.DoSpawn`, run once per second per map.
    fn do_spawns(&mut self, map: i32) {
        let count = self.maps[&map].spawns.len();
        for gi in 0..count {
            let (due, alive, want, delay, announce) = {
                let g = &self.maps[&map].spawns[gi];
                (
                    self.now >= g.next_spawn,
                    g.alive,
                    g.def.count,
                    g.def.delay,
                    g.def.announce,
                )
            };
            if !due {
                continue;
            }
            let delay_s = (delay.max(0) as u64).min(1_000_000) * 60;
            let next = if announce {
                delay_s * 1000
            } else {
                let d = if delay_s > 0 {
                    self.rng.random_range(0..delay_s)
                } else {
                    0
                };
                (d + delay_s / 2) * 1000
            };
            self.maps.get_mut(&map).unwrap().spawns[gi].next_spawn = self.now + next.max(1000);
            for _ in alive..want {
                if !self.spawn_monster(map, gi) {
                    break;
                }
            }
        }
    }

    /// Is `p` blocked for movement? `grace` applies Zircon's 300 ms vacated-cell
    /// grace that only players get.
    fn cell_blocked(&self, map: i32, p: Point, grace: bool) -> bool {
        let Some(m) = self.maps.get(&map) else {
            return true;
        };
        if !m.file.is_walkable(p.x, p.y) {
            return true;
        }
        m.objects_at(p).iter().any(|id| {
            let o = &self.objects[id];
            o.blocking() && !(grace && o.cell_time > self.now)
        })
    }

    fn move_object(&mut self, id: ObjectId, to: Point) {
        let obj = self.objects.get_mut(&id).unwrap();
        let from = obj.location;
        obj.location = to;
        obj.cell_time = self.now + CELL_GRACE;
        let map = obj.map;
        let m = self.maps.get_mut(&map).unwrap();
        m.remove_from_cell(id, from);
        m.add_to_cell(id, to);
    }

    fn send_to(&mut self, id: ObjectId, msg: ServerMessage) {
        if let Some(p) = self.objects.get(&id).and_then(|o| o.player()) {
            self.outgoing.push(Outgoing::To(p.conn, msg));
        }
    }

    // ---- player commands -------------------------------------------------

    pub fn player_turn(&mut self, id: ObjectId, direction: Direction) {
        let Some(o) = self.objects.get_mut(&id) else {
            return;
        };
        if o.dead || self.now < o.action_time {
            let (loc, dir) = (o.location, o.direction);
            self.send_to(
                id,
                ServerMessage::MoveDenied {
                    location: loc,
                    direction: dir,
                },
            );
            return;
        }
        o.direction = direction;
        o.action_time = self.now + TURN_TIME;
        self.events
            .push((id, ServerMessage::ObjectTurn { id, direction }));
    }

    pub fn player_move(&mut self, id: ObjectId, direction: Direction, run: bool) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let (map, from) = (o.map, o.location);
        let distance = if run { 2 } else { 1 };
        let ok = !o.dead && self.now >= o.action_time && self.now >= o.move_time && {
            (1..=distance).all(|i| !self.cell_blocked(map, from.step(direction, i), true))
        };
        if !ok {
            let dir = o.direction;
            self.send_to(
                id,
                ServerMessage::MoveDenied {
                    location: from,
                    direction: dir,
                },
            );
            return;
        }
        let to = from.step(direction, distance);
        {
            let o = self.objects.get_mut(&id).unwrap();
            o.direction = direction;
            o.action_time = self.now + MOVE_TIME;
            o.move_time = self.now + MOVE_TIME;
        }
        self.move_object(id, to);
        if self.try_travel(id) {
            return;
        }
        self.events.push((
            id,
            ServerMessage::ObjectMove {
                id,
                from,
                to,
                direction,
                run,
            },
        ));
    }

    /// Zircon `PlayerObject.Attack`: melee swing with optional attack skill.
    pub fn player_attack(&mut self, id: ObjectId, direction: Direction, attack_magic: Option<u16>) {
        let Some(o) = self.objects.get_mut(&id) else {
            return;
        };
        if o.dead || self.now < o.action_time || self.now < o.attack_time {
            let (loc, dir) = (o.location, o.direction);
            self.send_to(
                id,
                ServerMessage::MoveDenied {
                    location: loc,
                    direction: dir,
                },
            );
            return;
        }
        o.direction = direction;
        o.action_time = self.now + ATTACK_TIME;
        let aspeed = o.player().map(|p| p.attack_speed).unwrap_or(0);
        o.attack_time = self.now + attack_delay(aspeed);
        let stats = o.stats;
        let (map, loc) = (o.map, o.location);

        // Which attack skills ride on this swing (Zircon `AttackCast`), in
        // MagicType order; the last one that "casts" is the valid attack magic.
        let mut magics: Vec<u16> = Vec::new();
        let mut valid: Option<u16> = None;
        let mut toggles = Vec::new();
        {
            let level = o.player().map(|p| p.level).unwrap_or(1);
            let mut owned: Vec<(u16, i32, i32)> = o
                .player()
                .map(|p| {
                    p.magics
                        .iter()
                        .filter_map(|m| {
                            let def = self.data.magics.get(&m.magic)?;
                            Some((m.magic, def.need_level[0], m.cost(def)))
                        })
                        .collect()
                })
                .unwrap_or_default();
            owned.sort_by_key(|(m, _, _)| *m);
            let roll: bool = self.rng.random_range(0..5) == 0;
            let p = o.player_mut().unwrap();
            for (m, need, cost) in owned {
                if level < need {
                    continue;
                }
                match m {
                    magic_type::SWORDSMANSHIP | magic_type::SPIRIT_SWORD => magics.push(m),
                    magic_type::SLAYING => {
                        if p.slaying_charged && attack_magic == Some(m) {
                            p.slaying_charged = false;
                            toggles.push((m, false));
                            valid = Some(m);
                            magics.push(m);
                        }
                        if !p.slaying_charged && roll {
                            p.slaying_charged = true;
                            toggles.push((m, true));
                        }
                    }
                    magic_type::THRUSTING | magic_type::HALF_MOON => {
                        let on = if m == magic_type::THRUSTING {
                            p.thrusting_on
                        } else {
                            p.half_moon_on
                        };
                        if attack_magic == Some(m) && on && cost <= p.mp {
                            p.mp -= cost;
                            valid = Some(m);
                            magics.push(m);
                        }
                    }
                    _ => {}
                }
            }
        }
        for (m, on) in toggles {
            self.send_to(id, ServerMessage::MagicToggle { magic: m, on });
        }
        if attack_magic != valid {
            // Zircon logs and resyncs; the swing does not happen.
            self.send_to(
                id,
                ServerMessage::MoveDenied {
                    location: loc,
                    direction,
                },
            );
            return;
        }
        let power = self.roll_dc(stats);
        self.events.push((
            id,
            ServerMessage::ObjectAttack {
                id,
                direction,
                attack_magic: valid,
            },
        ));
        self.pending_hits.push(PendingHit {
            time: self.now + 300,
            attacker: id,
            target_cell: (map, loc.step(direction, 1)),
            power,
            target: None,
            magics: magics.clone(),
            primary: true,
        });
        // Secondary cells (Zircon `SecondaryAttackLocation`).
        let extra: Vec<Point> = match valid {
            Some(magic_type::THRUSTING) => vec![loc.step(direction, 2)],
            Some(magic_type::HALF_MOON) => vec![
                loc.step(direction.rotate(-1), 1),
                loc.step(direction.rotate(1), 1),
                loc.step(direction.rotate(2), 1),
            ],
            _ => Vec::new(),
        };
        for cell in extra {
            self.pending_hits.push(PendingHit {
                time: self.now + 300,
                attacker: id,
                target_cell: (map, cell),
                power,
                target: None,
                magics: magics.clone(),
                primary: false,
            });
        }
        if valid.is_some() {
            self.send_player_stats(id);
        }
    }

    fn roll_dc(&mut self, s: CombatStats) -> i32 {
        if s.min_dc >= s.max_dc {
            s.max_dc
        } else {
            self.rng.random_range(s.min_dc..=s.max_dc)
        }
    }

    fn roll_ac(&mut self, s: CombatStats) -> i32 {
        if s.min_ac >= s.max_ac {
            s.max_ac
        } else {
            self.rng.random_range(s.min_ac..=s.max_ac)
        }
    }

    // ---- combat resolution ------------------------------------------------

    fn resolve_hits(&mut self) {
        let due: Vec<PendingHit> = {
            let (due, later): (Vec<_>, Vec<_>) = self
                .pending_hits
                .drain(..)
                .partition(|h| h.time <= self.now);
            self.pending_hits = later;
            due
        };
        for hit in due {
            let Some(attacker) = self.objects.get(&hit.attacker) else {
                continue;
            };
            if attacker.dead {
                continue;
            }
            let attacker_stats = attacker.stats;
            let attacker_is_player = attacker.is_player();
            let targets: Vec<ObjectId> = match hit.target {
                Some(t) => vec![t],
                None => self
                    .maps
                    .get(&hit.target_cell.0)
                    .map(|m| m.objects_at(hit.target_cell.1).to_vec())
                    .unwrap_or_default(),
            };
            for tid in targets {
                let Some(target) = self.objects.get(&tid) else {
                    continue;
                };
                if target.dead || tid == hit.attacker {
                    continue;
                }
                // Players hit monsters; monsters hit players (no PvP in the prototype).
                let valid = if attacker_is_player {
                    target.is_monster()
                } else {
                    target.is_player()
                };
                if !valid {
                    continue;
                }
                if hit.target.is_some()
                    && target
                        .location
                        .distance(self.objects[&hit.attacker].location)
                        > 1
                {
                    continue; // target walked away before the swing landed
                }
                let tstats = target.stats;
                // Hit chance: Random.Next(Agility) > Accuracy => dodge.
                let roll = if tstats.agility > 0 {
                    self.rng.random_range(0..tstats.agility)
                } else {
                    0
                };
                if roll > attacker_stats.accuracy {
                    continue;
                }
                let mut power = hit.power;
                // Attack skill modifiers (Zircon `ModifyPowerAdditionner`).
                for m in &hit.magics {
                    let Some(def) = self.data.magics.get(m) else {
                        continue;
                    };
                    let Some(um) = self.objects[&hit.attacker]
                        .player()
                        .and_then(|p| p.magics.iter().find(|x| x.magic == *m))
                    else {
                        continue;
                    };
                    let (pmin, pmax) = um.power_range(def);
                    let mp = if pmin >= pmax {
                        pmin
                    } else {
                        self.rng.random_range(pmin..=pmax)
                    };
                    match *m {
                        magic_type::SLAYING => power += mp,
                        magic_type::THRUSTING | magic_type::HALF_MOON if !hit.primary => {
                            power = power * mp / 100;
                        }
                        _ => {}
                    }
                }
                power -= self.roll_ac(tstats);
                if power <= 0 {
                    continue;
                }
                if attacker_is_player && self.rng.random_range(0..100) < 1 {
                    power *= 2; // CriticalChance 1, CriticalDamage 0
                }
                let dealt = self.damage(tid, hit.attacker, power, element::NONE, false);
                if dealt > 0 && attacker_is_player {
                    for m in hit.magics.clone() {
                        self.level_magic(hit.attacker, m);
                    }
                }
            }
        }
    }

    fn damage(
        &mut self,
        target: ObjectId,
        attacker: ObjectId,
        power: i32,
        elem: u8,
        magic: bool,
    ) -> i32 {
        let now = self.now;
        let power = if self.objects[&target].poisons.iter().any(|p| p.kind == 2) {
            power * 12 / 10 // Red poison: +20 % damage taken
        } else {
            power
        };
        let (died, is_player, map, struck) = {
            let t = self.objects.get_mut(&target).unwrap();
            t.hp -= power;
            let mut struck = true;
            if let Kind::Monster(m) = &mut t.kind {
                if m.exp_owner.is_none() {
                    m.exp_owner = Some(attacker);
                }
                if m.target.is_none() {
                    m.target = Some(attacker);
                }
                struck = now > m.struck_time + 300;
                if struck {
                    m.struck_time = now;
                }
            }
            (t.hp <= 0, t.is_player(), t.map, struck)
        };
        let _ = map;
        if struck {
            self.events.push((
                target,
                ServerMessage::ObjectStruck {
                    id: target,
                    attacker,
                    damage: power,
                    element: elem,
                    magic,
                },
            ));
        }
        let (hp, max_hp) = {
            let t = &self.objects[&target];
            (t.hp.max(0), t.max_hp)
        };
        self.events.push((
            target,
            ServerMessage::HealthChanged {
                id: target,
                hp,
                max_hp,
            },
        ));
        if died {
            if is_player {
                self.player_die(target);
            } else {
                self.monster_die(target, attacker);
            }
        }
        power
    }

    fn monster_die(&mut self, id: ObjectId, _killer: ObjectId) {
        let (exp, owner, spawn) = {
            let o = self.objects.get_mut(&id).unwrap();
            o.dead = true;
            o.hp = 0;
            let m = o.monster_mut().unwrap();
            m.dead_time = self.now + DEAD_DURATION;
            m.target = None;
            (m.experience, m.exp_owner, m.spawn)
        };
        if let Some((map, gi)) = spawn {
            if let Some(g) = self.maps.get_mut(&map).and_then(|m| m.spawns.get_mut(gi)) {
                g.alive -= 1;
            }
        }
        self.events.push((id, ServerMessage::ObjectDie { id }));
        if let Some(owner) = owner {
            self.gain_experience(owner, exp as u64);
        }
        self.drop_loot(id, owner);
    }

    fn gain_experience(&mut self, id: ObjectId, amount: u64) {
        let Some(o) = self.objects.get_mut(&id) else {
            return;
        };
        let Some(p) = o.player_mut() else {
            return;
        };
        p.experience += amount;
        let mut leveled = false;
        while p.level < 40
            && p.experience >= GameData::max_experience(p.level)
            && GameData::max_experience(p.level) > 0
        {
            p.experience -= GameData::max_experience(p.level);
            p.level += 1;
            leveled = true;
        }
        let name = p.name.clone();
        let level = p.level;
        let class = p.class;
        let _ = class;
        if leveled {
            self.refresh_stats(id, true);
            self.events.push((
                id,
                ServerMessage::Chat {
                    text: format!("{name} has reached level {level}!"),
                },
            ));
            let (hp, max_hp) = {
                let o = &self.objects[&id];
                (o.hp, o.max_hp)
            };
            self.events
                .push((id, ServerMessage::HealthChanged { id, hp, max_hp }));
        }
        let stats = self.player_stats(&self.objects[&id]);
        self.send_to(id, ServerMessage::StatsChanged(stats));
    }

    fn player_die(&mut self, id: ObjectId) {
        let o = self.objects.get_mut(&id).unwrap();
        o.dead = true;
        o.hp = 0;
        let p = o.player_mut().unwrap();
        p.revive_time = self.now + REVIVE_DELAY;
        self.events.push((id, ServerMessage::ObjectDie { id }));
        self.send_to(
            id,
            ServerMessage::Chat {
                text: "You have died. Use the Revive button to return to town.".into(),
            },
        );
    }

    /// Zircon `C.TownRevive`: return to the bind point immediately.
    pub fn town_revive(&mut self, id: ObjectId) {
        let dead = self.objects.get(&id).map(|o| o.dead).unwrap_or(false);
        if !dead {
            return;
        }
        if let Err(e) = self.revive_player(id) {
            tracing::warn!("revive failed: {e}");
        }
    }

    fn revive_player(&mut self, id: ObjectId) -> anyhow::Result<()> {
        let bind_region = self.objects[&id].player().unwrap().bind_region;
        let location = if bind_region != 0 {
            self.go_to_bind_point(id)?
        } else {
            let location = self.objects[&id].location;
            self.move_object(id, location);
            location
        };
        let (hp, dir) = {
            let o = self.objects.get_mut(&id).unwrap();
            o.dead = false;
            o.hp = o.max_hp;
            o.direction = Direction::Down;
            let p = o.player_mut().unwrap();
            p.mp = p.max_mp;
            (o.hp, o.direction)
        };
        self.events.push((
            id,
            ServerMessage::ObjectRevive {
                id,
                location,
                direction: dir,
                hp,
            },
        ));
        let stats = self.player_stats(&self.objects[&id]);
        self.send_to(id, ServerMessage::StatsChanged(stats));
        Ok(())
    }

    // ---- monster AI --------------------------------------------------------

    fn process_monster(&mut self, id: ObjectId) {
        let now = self.now;
        let (map, loc, dead, dead_time) = {
            let o = &self.objects[&id];
            let m = match &o.kind {
                Kind::Monster(m) => m,
                _ => return,
            };
            (o.map, o.location, o.dead, m.dead_time)
        };
        if dead {
            if now > dead_time {
                self.remove_object(id);
            }
            return;
        }
        // Drop invalid targets.
        let target = {
            let o = self.objects.get_mut(&id).unwrap();
            let m = o.monster_mut().unwrap();
            if let Some(t) = m.target {
                let valid = self
                    .objects
                    .get(&t)
                    .map(|to| {
                        !to.dead && to.map == map && to.location.distance(loc) <= MAX_VIEW_RANGE
                    })
                    .unwrap_or(false);
                if !valid {
                    self.objects
                        .get_mut(&id)
                        .unwrap()
                        .monster_mut()
                        .unwrap()
                        .target = None;
                }
            }
            self.objects[&id].monster_mut_ref().target
        };

        // Regen.
        {
            let o = self.objects.get_mut(&id).unwrap();
            let m = match &mut o.kind {
                Kind::Monster(m) => m,
                _ => unreachable!(),
            };
            if now >= m.regen_time {
                m.regen_time = now + REGEN_DELAY;
                if o.hp < o.max_hp {
                    o.hp = (o.hp + (o.max_hp as f32 * 0.02).max(1.0) as i32).min(o.max_hp);
                    let (hp, max_hp) = (o.hp, o.max_hp);
                    self.events
                        .push((id, ServerMessage::HealthChanged { id, hp, max_hp }));
                }
            }
        }

        // Search (Zircon ProcessSearch): nearest player within ViewRange every 3 s.
        // Passive monsters (AI 1/2: chicken, pig, deer, cow; trees) never search;
        // they only retaliate once hit.
        if target.is_none() {
            let (search_due, view_range, passive) = {
                let o = &self.objects[&id];
                let m = o.monster_ref();
                let def = &self.data.monsters[&m.def];
                (now >= m.search_time, def.view_range, def.is_passive())
            };
            if search_due && !passive {
                self.objects
                    .get_mut(&id)
                    .unwrap()
                    .monster_mut()
                    .unwrap()
                    .search_time = now + SEARCH_DELAY;
                let mut best: Vec<ObjectId> = Vec::new();
                let mut best_d = i32::MAX;
                for pid in &self.maps[&map].objects {
                    let p = &self.objects[pid];
                    if !p.is_player() || p.dead {
                        continue;
                    }
                    let d = p.location.distance(loc);
                    if d > view_range {
                        continue;
                    }
                    if d < best_d {
                        best_d = d;
                        best.clear();
                    }
                    if d == best_d {
                        best.push(*pid);
                    }
                }
                if !best.is_empty() {
                    let pick = best[self.rng.random_range(0..best.len())];
                    self.objects
                        .get_mut(&id)
                        .unwrap()
                        .monster_mut()
                        .unwrap()
                        .target = Some(pick);
                }
            }
        }
        let target = self.objects[&id].monster_ref().target;

        // Roam (Zircon ProcessRoam): every 2 s, 10% chance to walk or turn.
        let can_move = self.monster_can_move(id);
        if can_move {
            let roam_due = now >= self.objects[&id].monster_ref().roam_time;
            if roam_due {
                self.objects
                    .get_mut(&id)
                    .unwrap()
                    .monster_mut()
                    .unwrap()
                    .roam_time = now + ROAM_DELAY;
                let seen = self
                    .objects
                    .values()
                    .any(|p| p.is_player() && p.visible.contains(&id));
                if seen && target.is_none() && self.rng.random_range(0..10) == 0 {
                    if self.rng.random_range(0..3) > 0 {
                        let dir = self.objects[&id].direction;
                        self.monster_walk(id, dir);
                    } else {
                        let dir = Direction::from_index(self.rng.random_range(0..8));
                        self.monster_turn(id, dir);
                    }
                }
            }
        }

        // Target (Zircon ProcessTarget).
        let Some(t) = target else {
            return;
        };
        let tloc = self.objects[&t].location;
        let in_range = tloc != loc && tloc.distance(loc) <= 1;
        if in_range {
            if self.monster_can_attack(id) {
                self.monster_attack(id, t);
            }
        } else if self.monster_can_move(id) {
            let dir = Direction::from_points(loc, tloc);
            let rot: i8 = match self.rng.random_range(0..3) {
                0 => -1,
                1 => 0,
                _ => 1,
            };
            let start = dir.rotate(rot);
            for i in 0..8 {
                let d = start.rotate(if i % 2 == 0 {
                    (i / 2) as i8
                } else {
                    -((i + 1) / 2) as i8
                });
                if self.monster_walk(id, d) {
                    break;
                }
            }
        }
    }

    fn monster_can_move(&self, id: ObjectId) -> bool {
        let o = &self.objects[&id];
        let m = o.monster_ref();
        !o.dead && m.move_delay > 0 && self.now >= o.action_time && self.now >= o.move_time
    }

    fn monster_can_attack(&self, id: ObjectId) -> bool {
        let o = &self.objects[&id];
        let m = o.monster_ref();
        !o.dead && m.attack_delay > 0 && self.now >= o.action_time && self.now >= o.attack_time
    }

    fn monster_walk(&mut self, id: ObjectId, dir: Direction) -> bool {
        let (map, from) = {
            let o = &self.objects[&id];
            (o.map, o.location)
        };
        let to = from.step(dir, 1);
        if self.cell_blocked(map, to, false) {
            return false;
        }
        {
            let o = self.objects.get_mut(&id).unwrap();
            o.direction = dir;
            let (md, ad) = {
                let m = o.monster_ref();
                (m.move_delay, m.attack_delay)
            };
            o.move_time = self.now + md;
            o.action_time = self.now + md.saturating_sub(100).min(ad);
        }
        self.move_object(id, to);
        self.events.push((
            id,
            ServerMessage::ObjectMove {
                id,
                from,
                to,
                direction: dir,
                run: false,
            },
        ));
        true
    }

    fn monster_turn(&mut self, id: ObjectId, dir: Direction) {
        let o = self.objects.get_mut(&id).unwrap();
        o.direction = dir;
        let (md, ad) = {
            let m = o.monster_ref();
            (m.move_delay, m.attack_delay)
        };
        o.move_time = self.now + md;
        o.action_time = self.now + md.saturating_sub(100).min(ad);
        self.events
            .push((id, ServerMessage::ObjectTurn { id, direction: dir }));
    }

    fn monster_attack(&mut self, id: ObjectId, target: ObjectId) {
        let tloc = self.objects[&target].location;
        let (dir, power, map, loc) = {
            let o = self.objects.get_mut(&id).unwrap();
            let dir = Direction::from_points(o.location, tloc);
            o.direction = dir;
            let (md, ad) = {
                let m = o.monster_ref();
                (m.move_delay, m.attack_delay)
            };
            o.attack_time = self.now + ad;
            o.action_time = self.now + md.min(ad.saturating_sub(100));
            let stats = o.stats;
            (dir, stats, o.map, o.location)
        };
        let power = self.roll_dc(power);
        self.events.push((
            id,
            ServerMessage::ObjectAttack {
                id,
                direction: dir,
                attack_magic: None,
            },
        ));
        self.pending_hits.push(PendingHit {
            time: self.now + 400,
            attacker: id,
            target_cell: (map, loc.step(dir, 1)),
            power,
            target: Some(target),
            magics: Vec::new(),
            primary: true,
        });
    }

    // ---- tick ----------------------------------------------------------------

    pub fn tick(&mut self, now: u64) {
        self.now = now;

        // Player timers: regen and revive.
        let players: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| o.is_player())
            .map(|o| o.id)
            .collect();
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
                let rate = if p.class == Class::Wizard { 0.03 } else { 0.02 };
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

    // ---- stats, inventory, equipment --------------------------------------

    /// Zircon `RefreshStats`: base stats for class/level plus equipped items.
    fn refresh_stats(&mut self, id: ObjectId, restore: bool) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let Some(p) = o.player() else {
            return;
        };
        let Some(base) = self.data.base_stat(p.class.mir_class(), p.level).cloned() else {
            return;
        };
        let eq = p.bag.equipment_stats(&self.data);
        let g = |k: i32| eq.get(&k).copied().unwrap_or(0);
        // Passive skills (Zircon `GetPassiveStats`).
        let mut pas_acc = 0;
        let mut pas_agi = 0;
        let mut pas_dc = 0;
        for m in &p.magics {
            let Some(def) = self.data.magics.get(&m.magic) else {
                continue;
            };
            if p.level < def.need_level[0] {
                continue;
            }
            match m.magic {
                magic_type::SWORDSMANSHIP | magic_type::SPIRIT_SWORD => {
                    pas_acc += m.power_range(def).0
                }
                magic_type::WILLOW_DANCE => pas_agi += m.power_range(def).0,
                magic_type::SLAYING => {
                    pas_acc += m.level as i32 * 2;
                    pas_dc += m.level as i32 * 2;
                }
                _ => {}
            }
        }
        let o = self.objects.get_mut(&id).unwrap();
        o.max_hp = base.health + g(stat::HEALTH);
        o.stats = CombatStats {
            accuracy: base.accuracy + g(stat::ACCURACY) + pas_acc,
            agility: base.agility + g(stat::AGILITY) + pas_agi,
            min_ac: base.min_ac + g(stat::MIN_AC),
            max_ac: base.max_ac + g(stat::MAX_AC),
            min_dc: base.min_dc + g(stat::MIN_DC) + pas_dc,
            max_dc: base.max_dc + g(stat::MAX_DC) + pas_dc,
            min_mr: base.min_mr + g(stat::MIN_MR),
            max_mr: base.max_mr + g(stat::MAX_MR),
            min_mc: base.min_mc + g(stat::MIN_MC),
            max_mc: base.max_mc + g(stat::MAX_MC),
            min_sc: base.min_sc + g(stat::MIN_SC),
            max_sc: base.max_sc + g(stat::MAX_SC),
        };
        if restore {
            o.hp = o.max_hp;
        } else {
            o.hp = o.hp.min(o.max_hp);
        }
        let p = o.player_mut().unwrap();
        p.max_mp = base.mana + g(stat::MANA);
        if restore {
            p.mp = p.max_mp;
        } else {
            p.mp = p.mp.min(p.max_mp);
        }
        p.attack_speed = g(stat::ATTACK_SPEED) as i64;
        p.max_bag = base.bag_weight + g(73);
        p.max_wear = base.wear_weight + g(74);
        p.max_hand = base.hand_weight + g(75);
    }

    fn weights_of(&self, id: ObjectId) -> Option<Weights> {
        let p = self.objects.get(&id)?.player()?;
        Some(p.bag.weights(&self.data, p.max_bag, p.max_wear, p.max_hand))
    }

    fn send_inventory(&mut self, id: ObjectId) {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let inventory = p
            .bag
            .inventory
            .iter()
            .enumerate()
            .filter_map(|(i, it)| it.as_ref().map(|it| (i as u8, it.instance())))
            .collect();
        let equipment = p
            .bag
            .equipment
            .iter()
            .enumerate()
            .filter_map(|(i, it)| it.as_ref().map(|it| (i as u8, it.instance())))
            .collect();
        let gold = p.bag.gold;
        let weights = p.bag.weights(&self.data, p.max_bag, p.max_wear, p.max_hand);
        self.send_to(
            id,
            ServerMessage::Inventory {
                inventory,
                equipment,
                gold,
                weights,
            },
        );
    }

    fn send_changes(&mut self, id: ObjectId, changes: Changed) {
        for (grid, slot, item) in changes {
            self.send_to(id, ServerMessage::ItemChanged { grid, slot, item });
        }
        if let Some(w) = self.weights_of(id) {
            self.send_to(id, ServerMessage::WeightsChanged(w));
        }
    }

    fn send_gold(&mut self, id: ObjectId) {
        if let Some(gold) = self
            .objects
            .get(&id)
            .and_then(|o| o.player())
            .map(|p| p.bag.gold)
        {
            self.send_to(id, ServerMessage::GoldChanged { gold });
        }
    }

    fn send_player_stats(&mut self, id: ObjectId) {
        if let Some(o) = self.objects.get(&id) {
            if o.is_player() {
                let stats = self.player_stats(o);
                let (hp, max_hp) = (o.hp, o.max_hp);
                self.send_to(id, ServerMessage::StatsChanged(stats));
                self.events
                    .push((id, ServerMessage::HealthChanged { id, hp, max_hp }));
            }
        }
    }

    /// Update the player's look from equipment and broadcast on change.
    fn refresh_appearance(&mut self, id: ObjectId) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let Some(p) = o.player() else {
            return;
        };
        let armour = p
            .bag
            .equipped_shape(&self.data, slot::ARMOUR)
            .unwrap_or(0)
            .max(0) as u16;
        let weapon = p
            .bag
            .equipped_shape(&self.data, slot::WEAPON)
            .map(|s| s.max(0) as u16);
        // Zircon: `Helmet = Equipment[Helmet]?.Info.Shape ?? 0` (1-based),
        // `Shield = Equipment[Shield]?.Info.Shape ?? -1`.
        let helmet = p
            .bag
            .equipped_shape(&self.data, slot::HELMET)
            .unwrap_or(0)
            .max(0) as u16;
        let shield = p
            .bag
            .equipped_shape(&self.data, slot::SHIELD)
            .map(|s| s.max(0) as u16);
        let appearance = Appearance::Player {
            name: p.name.clone(),
            gender: p.gender,
            class: p.class,
            armour,
            weapon,
            hair: p.hair,
            helmet,
            shield,
        };
        if o.appearance != appearance {
            let o = self.objects.get_mut(&id).unwrap();
            o.appearance = appearance.clone();
            self.events
                .push((id, ServerMessage::ObjectAppearance { id, appearance }));
        }
    }

    /// Zircon `NewCharacter`: every `StartItem` usable by the class/gender.
    fn give_start_items(&mut self, id: ObjectId) {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let (class, gender) = (p.class, p.gender);
        let gflag = match gender {
            Gender::Male => 1,
            Gender::Female => 2,
        };
        let mut starts: Vec<i32> = self
            .data
            .items
            .values()
            .filter(|i| {
                i.start_item
                    && i.required_class & class.flag() != 0
                    && i.required_gender & gflag != 0
            })
            .map(|i| i.index)
            .collect();
        starts.sort();
        let o = self.objects.get_mut(&id).unwrap();
        let p = o.player_mut().unwrap();
        for info in starts {
            let mut next = p.next_item_id;
            p.bag.gain(&self.data, info, 1, &mut next);
            p.next_item_id = next;
        }
        // Auto-equip what fits so the character does not start naked.
        let mut auto = Vec::new();
        for (i, it) in p.bag.inventory.iter().enumerate() {
            if let Some(it) = it {
                if let Some(def) = self.data.items.get(&it.info) {
                    if !item_type::slots(def.item_type).is_empty() {
                        auto.push(i as u8);
                    }
                }
            }
        }
        for slot in auto {
            let _ = self.item_use_inner(id, slot);
        }
    }

    pub fn item_move(&mut self, id: ObjectId, from: Grid, from_slot: u8, to: Grid, to_slot: u8) {
        let result = self.item_move_inner(id, from, from_slot, to, to_slot);
        match result {
            Ok(changes) => {
                self.send_changes(id, changes);
                self.refresh_stats(id, false);
                self.refresh_appearance(id);
                self.send_player_stats(id);
            }
            Err(e) => self.send_to(id, ServerMessage::Chat { text: e }),
        }
    }

    fn item_move_inner(
        &mut self,
        id: ObjectId,
        from: Grid,
        from_slot: u8,
        to: Grid,
        to_slot: u8,
    ) -> Result<Changed, String> {
        let o = self.objects.get(&id).ok_or("no player")?;
        let p = o.player().ok_or("no player")?;
        let (class, gender, level) = (p.class, p.gender, p.level);
        let (max_wear, max_hand) = (p.max_wear, p.max_hand);
        if from == Grid::Equipment && to == Grid::Equipment {
            return Err("Cannot move between equipment slots".into());
        }
        let src = p
            .bag
            .grid(from)
            .get(from_slot as usize)
            .cloned()
            .flatten()
            .ok_or("Nothing there")?;
        let dst = p.bag.grid(to).get(to_slot as usize).cloned().flatten();
        if to == Grid::Equipment || from == Grid::Equipment {
            // The item moving INTO equipment must fit; the item moving out
            // (if any) needs no check.
            let (moving_in, target_slot) = if to == Grid::Equipment {
                (Some(&src), to_slot as usize)
            } else {
                (dst.as_ref(), from_slot as usize)
            };
            if let Some(item) = moving_in {
                let def = self.data.items.get(&item.info).ok_or("Unknown item")?;
                if !item_type::slots(def.item_type).contains(&target_slot) {
                    return Err("That does not go there".into());
                }
                can_use(def, class, gender, level)?;
                let replaced_weight = p
                    .bag
                    .equipment
                    .get(target_slot)
                    .and_then(|c| c.as_ref())
                    .map(|c| crate::items::item_weight(&self.data, c))
                    .unwrap_or(0);
                let hand = matches!(
                    def.item_type,
                    item_type::WEAPON | item_type::TORCH | item_type::SHIELD
                );
                let (current, max) = if hand {
                    (p.bag.hand_weight(&self.data), max_hand)
                } else {
                    (p.bag.wear_weight(&self.data), max_wear)
                };
                if current - replaced_weight + def.weight > max {
                    return Err("Too heavy to wear".into());
                }
            }
        }
        let o = self.objects.get_mut(&id).unwrap();
        let p = o.player_mut().unwrap();
        let mut changes = Changed::new();
        // Merge stacks when moving within the inventory.
        if from == Grid::Inventory && to == Grid::Inventory {
            if let Some(d) = &dst {
                if d.info == src.info && d.id != src.id {
                    let stack = self
                        .data
                        .items
                        .get(&src.info)
                        .map(|i| i.stack_size)
                        .unwrap_or(1) as u32;
                    if d.count < stack {
                        let add = (stack - d.count).min(src.count);
                        let d = p.bag.inventory[to_slot as usize].as_mut().unwrap();
                        d.count += add;
                        changes.push((to, to_slot, Some(d.instance())));
                        let s = p.bag.inventory[from_slot as usize].as_mut().unwrap();
                        s.count -= add;
                        if s.count == 0 {
                            p.bag.inventory[from_slot as usize] = None;
                            changes.push((from, from_slot, None));
                        } else {
                            changes.push((from, from_slot, Some(s.instance())));
                        }
                        return Ok(changes);
                    }
                }
            }
        }
        p.bag.grid_mut(from)[from_slot as usize] = dst.clone();
        p.bag.grid_mut(to)[to_slot as usize] = Some(src.clone());
        changes.push((from, from_slot, dst.map(|d| d.instance())));
        changes.push((to, to_slot, Some(src.instance())));
        Ok(changes)
    }

    pub fn item_use(&mut self, id: ObjectId, slot: u8) {
        match self.item_use_inner(id, slot) {
            Ok(changes) => {
                self.send_changes(id, changes);
                self.refresh_stats(id, false);
                self.refresh_appearance(id);
                self.send_player_stats(id);
            }
            Err(e) => self.send_to(id, ServerMessage::Chat { text: e }),
        }
    }

    fn item_use_inner(&mut self, id: ObjectId, slot: u8) -> Result<Changed, String> {
        let o = self.objects.get(&id).ok_or("no player")?;
        let p = o.player().ok_or("no player")?;
        let item = p
            .bag
            .inventory
            .get(slot as usize)
            .cloned()
            .flatten()
            .ok_or("Nothing there")?;
        let def = self
            .data
            .items
            .get(&item.info)
            .ok_or("Unknown item")?
            .clone();
        if !item_type::slots(def.item_type).is_empty() {
            let target = default_slot(def.item_type, &p.bag.equipment).ok_or("Cannot equip")?;
            return self.item_move_inner(id, Grid::Inventory, slot, Grid::Equipment, target as u8);
        }
        match def.item_type {
            item_type::CONSUMABLE => {
                can_use(&def, p.class, p.gender, p.level)?;
                if o.dead {
                    return Err("You cannot use that while dead".into());
                }
                // Zircon: `if (SEnvir.Now < UseItemTime) return;` (silent).
                if self.now < p.use_item_time {
                    return Ok(Vec::new());
                }
                self.use_consumable(id, slot, &def)
            }
            item_type::BOOK => self.learn_book(id, slot, &def),
            _ => Err(format!("{} cannot be used", def.name)),
        }
    }

    /// Zircon `ItemUse` for `ItemType.Consumable`: potions heal instantly
    /// (boosted by Potion Mastery), town/random teleport scrolls move the
    /// player, and every use starts a `Durability` ms cooldown.
    fn use_consumable(
        &mut self,
        id: ObjectId,
        slot: u8,
        def: &crate::data::ItemDef,
    ) -> Result<Changed, String> {
        match def.shape {
            0 => {
                let mut health = def.stat(stat::HEALTH);
                let mut mana = def.stat(stat::MANA);
                // Potion Mastery: `health += health * GetPower() / 100`, rolled
                // separately per stat; levels while something was missing.
                let mastery = self.objects[&id]
                    .player()
                    .and_then(|p| {
                        p.magics
                            .iter()
                            .find(|m| m.magic == magic_type::POTION_MASTERY)
                    })
                    .and_then(|m| self.data.magics.get(&m.magic).map(|d| m.power_range(d)));
                if let Some((pmin, pmax)) = mastery {
                    let mut roll = || {
                        if pmin >= pmax {
                            pmin
                        } else {
                            self.rng.random_range(pmin..=pmax)
                        }
                    };
                    let (hb, mb) = (roll(), roll());
                    health += health * hb / 100;
                    mana += mana * mb / 100;
                    let missing = {
                        let o = &self.objects[&id];
                        let p = o.player().unwrap();
                        o.hp < o.max_hp || p.mp < p.max_mp
                    };
                    if missing {
                        self.level_magic(id, magic_type::POTION_MASTERY);
                    }
                }
                let o = self.objects.get_mut(&id).unwrap();
                o.hp = (o.hp + health).min(o.max_hp);
                let p = o.player_mut().unwrap();
                p.mp = (p.mp + mana).min(p.max_mp);
                let exp = def.stat(stat::EXPERIENCE);
                if exp > 0 {
                    self.gain_experience(id, exp as u64);
                }
            }
            2 => {
                // Town teleport: a random cell of the bind point.
                let bind_region = self.objects[&id].player().unwrap().bind_region;
                if bind_region == 0 {
                    return Err("You have no town to return to".into());
                }
                self.go_to_bind_point(id).map_err(|e| e.to_string())?;
            }
            3 => {
                let map = self.objects[&id].map;
                let to = self.random_walkable(map).ok_or("Nowhere to teleport to")?;
                self.move_object(id, to);
            }
            _ => return Err(format!("{} cannot be used", def.name)),
        }
        let o = self.objects.get_mut(&id).unwrap();
        let p = o.player_mut().unwrap();
        p.use_item_time = self.now + def.durability.max(0) as u64;
        let change = p.bag.take(Grid::Inventory, slot, 1);
        Ok(change.into_iter().collect())
    }

    /// Move a player to a random cell of its bind point, changing map if
    /// the bind region lies elsewhere.
    fn go_to_bind_point(&mut self, id: ObjectId) -> anyhow::Result<Point> {
        let (map, bind_region) = {
            let o = &self.objects[&id];
            (o.map, o.player().unwrap().bind_region)
        };
        let target_map = self
            .data
            .regions
            .get(&bind_region)
            .map(|r| r.map)
            .ok_or_else(|| anyhow::anyhow!("unknown bind region {bind_region}"))?;
        self.ensure_map(target_map)?;
        let location = self.bind_point(target_map, bind_region)?;
        if target_map == map {
            self.move_object(id, location);
        } else {
            self.change_map(id, target_map, location);
        }
        Ok(location)
    }

    pub fn item_drop(&mut self, id: ObjectId, slot: u8, count: u32) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let Some(p) = o.player() else {
            return;
        };
        if o.dead {
            return;
        }
        let (map, loc) = (o.map, o.location);
        let Some(item) = p.bag.inventory.get(slot as usize).cloned().flatten() else {
            return;
        };
        let Some(def) = self.data.items.get(&item.info) else {
            return;
        };
        if !def.can_drop {
            self.send_to(
                id,
                ServerMessage::Chat {
                    text: "That cannot be dropped".into(),
                },
            );
            return;
        }
        let count = count.clamp(1, item.count);
        let change = self
            .objects
            .get_mut(&id)
            .unwrap()
            .player_mut()
            .unwrap()
            .bag
            .take(Grid::Inventory, slot, count);
        let dropped = UserItem { count, ..item };
        self.spawn_ground_item(map, loc, dropped, None, 0);
        self.send_changes(id, change.into_iter().collect());
    }

    pub fn pick_up(&mut self, id: ObjectId) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let Some(p) = o.player() else {
            return;
        };
        if o.dead {
            return;
        }
        let (map, loc, account, max_bag) = (o.map, o.location, p.account, p.max_bag);
        let mut candidates: Vec<ObjectId> = Vec::new();
        for d in 0..=PICKUP_RADIUS {
            for dy in -d..=d {
                for dx in -d..=d {
                    if dx.abs().max(dy.abs()) != d {
                        continue;
                    }
                    let cell = Point::new(loc.x + dx, loc.y + dy);
                    if let Some(m) = self.maps.get(&map) {
                        candidates.extend(m.objects_at(cell).iter().copied());
                    }
                }
            }
        }
        for cid in candidates {
            let (item, allowed) = {
                let Some(obj) = self.objects.get(&cid) else {
                    continue;
                };
                let Kind::Item(i) = &obj.kind else { continue };
                let allowed = i.owner.is_none_or(|a| a == account)
                    || self.now >= i.spawn_time + DROP_SHARE_AFTER;
                (i.item.clone(), allowed)
            };
            if !allowed {
                continue;
            }
            let Some(def) = self.data.items.get(&item.info).cloned() else {
                continue;
            };
            if item.info == self.data.gold_item {
                let o = self.objects.get_mut(&id).unwrap();
                let p = o.player_mut().unwrap();
                p.bag.gold += item.count as u64;
                self.remove_object(cid);
                self.send_gold(id);
                self.send_to(
                    id,
                    ServerMessage::Chat {
                        text: format!("You picked up {} gold.", item.count),
                    },
                );
                return;
            }
            let p = self.objects[&id].player().unwrap();
            if !p.bag.can_gain(&self.data, item.info, item.count, max_bag) {
                self.send_to(
                    id,
                    ServerMessage::Chat {
                        text: "You cannot carry any more.".into(),
                    },
                );
                return;
            }
            let o = self.objects.get_mut(&id).unwrap();
            let p = o.player_mut().unwrap();
            let mut next = p.next_item_id;
            let changes = p.bag.gain(&self.data, item.info, item.count, &mut next);
            p.next_item_id = next;
            self.remove_object(cid);
            self.send_changes(id, changes);
            let text = if item.count > 1 {
                format!("You picked up {} ({}).", def.name, item.count)
            } else {
                format!("You picked up {}.", def.name)
            };
            self.send_to(id, ServerMessage::Chat { text });
            return;
        }
    }

    fn spawn_ground_item(
        &mut self,
        map: i32,
        near: Point,
        item: UserItem,
        owner: Option<u32>,
        spread: i32,
    ) {
        let mut location = near;
        if spread > 0 {
            for _ in 0..20 {
                let p = Point::new(
                    near.x + self.rng.random_range(-spread..=spread),
                    near.y + self.rng.random_range(-spread..=spread),
                );
                let ok = self
                    .maps
                    .get(&map)
                    .map(|m| m.file.is_walkable(p.x, p.y))
                    .unwrap_or(false);
                if ok {
                    location = p;
                    break;
                }
            }
        }
        if !self.maps.contains_key(&map) {
            return;
        }
        let id = self.alloc_id();
        let appearance = Appearance::Item {
            info: item.info,
            count: item.count,
        };
        let obj = Object {
            id,
            kind: Kind::Item(ItemData {
                item,
                owner,
                spawn_time: self.now,
                expire: self.now + DROP_DURATION,
            }),
            map,
            location,
            direction: Direction::Down,
            hp: 0,
            max_hp: 0,
            dead: false,
            stats: CombatStats::ZERO,
            action_time: 0,
            move_time: 0,
            attack_time: 0,
            cell_time: 0,
            appearance,
            visible: HashSet::new(),
            poisons: Vec::new(),
            heal: None,
        };
        self.insert_object(obj);
    }

    /// Zircon `MonsterObject.Drop`: every DropInfo row rolls `1 in Chance`.
    fn drop_loot(&mut self, monster: ObjectId, killer: Option<ObjectId>) {
        let (map, loc, def_index) = {
            let o = &self.objects[&monster];
            (o.map, o.location, o.monster_ref().def)
        };
        let owner = killer
            .and_then(|k| self.objects.get(&k))
            .and_then(|o| o.player())
            .map(|p| p.account);
        let drops = self
            .drops_by_monster
            .get(&def_index)
            .cloned()
            .unwrap_or_default();
        for d in drops {
            if d.chance <= 0 || d.part_only {
                continue;
            }
            let Some(item) = self.data.items.get(&d.item).cloned() else {
                continue;
            };
            let amount = (d.amount / 2 + self.rng.random_range(0..d.amount.max(1))).max(1);
            if self.rng.random_range(0..d.chance) != 0 {
                continue;
            }
            let mut remaining = amount as u32;
            let stack = item.stack_size.max(1) as u32;
            while remaining > 0 {
                let count = remaining.min(stack);
                remaining -= count;
                let ui = UserItem {
                    id: 0,
                    info: item.index,
                    count,
                    durability: item.durability,
                    max_durability: item.durability,
                };
                self.spawn_ground_item(map, loc, ui, owner, DROP_DISTANCE);
                if item.index == self.data.gold_item {
                    break;
                }
            }
        }
    }

    // ---- NPCs ----------------------------------------------------------------------

    fn spawn_npcs(&mut self, map: i32) {
        let width = self.maps[&map].file.width as i32;
        let npcs: Vec<(i32, i32, String, i32, i32)> = self
            .data
            .npcs
            .iter()
            .filter_map(|n| {
                let r = self.data.regions.get(&n.region)?;
                if r.map != map {
                    return None;
                }
                Some((n.index, n.region, n.name.clone(), n.image, n.entry_page))
            })
            .collect();
        for (info, region, name, image, entry_page) in npcs {
            let points: Vec<Point> = self.data.regions[&region]
                .points(width)
                .into_iter()
                .map(|(x, y)| Point::new(x, y))
                .collect();
            let Some(location) = self.random_point(&points) else {
                continue;
            };
            let id = self.alloc_id();
            let display = name.rsplit('_').next().unwrap_or(&name).to_string();
            let obj = Object {
                id,
                kind: Kind::Npc(NpcData { info, entry_page }),
                map,
                location,
                direction: Direction::Up,
                hp: 1,
                max_hp: 1,
                dead: false,
                stats: CombatStats::ZERO,
                action_time: 0,
                move_time: 0,
                attack_time: 0,
                cell_time: 0,
                appearance: Appearance::Npc {
                    name: display,
                    image: image.max(0) as u16,
                },
                visible: HashSet::new(),
                poisons: Vec::new(),
                heal: None,
            };
            self.insert_object(obj);
        }
    }

    pub fn npc_call(&mut self, id: ObjectId, npc: ObjectId) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let (map, loc) = (o.map, o.location);
        let entry = match self.objects.get(&npc) {
            Some(n) if n.map == map && n.location.distance(loc) <= MAX_VIEW_RANGE => {
                match &n.kind {
                    Kind::Npc(d) => d.entry_page,
                    _ => return,
                }
            }
            _ => return,
        };
        if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
            p.npc = None;
        }
        self.npc_run_page(id, npc, entry);
    }

    pub fn npc_button(&mut self, id: ObjectId, button: i32) {
        let Some((npc, page)) = self
            .objects
            .get(&id)
            .and_then(|o| o.player())
            .and_then(|p| p.npc)
        else {
            return;
        };
        let Some(def) = self.data.npc_pages.get(&page) else {
            return;
        };
        let Some((_, dest)) = def.buttons.iter().find(|(b, d)| *b == button && *d != 0) else {
            return;
        };
        let dest = *dest;
        self.npc_run_page(id, npc, dest);
    }

    pub fn npc_close(&mut self, id: ObjectId) {
        if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
            p.npc = None;
        }
    }

    /// Zircon `NPCObject.NPCCall`: walk pages through checks and actions until
    /// one with text is reached.
    fn npc_run_page(&mut self, id: ObjectId, npc: ObjectId, mut page: i32) {
        for _ in 0..20 {
            if page == 0 {
                self.npc_close(id);
                self.send_to(id, ServerMessage::NpcClose);
                return;
            }
            let Some(def) = self.data.npc_pages.get(&page).cloned() else {
                self.npc_close(id);
                self.send_to(id, ServerMessage::NpcClose);
                return;
            };
            let mut failed = None;
            for c in &def.checks {
                if !self.npc_check(id, c) {
                    failed = Some(c.fail_page);
                    break;
                }
            }
            if let Some(fail) = failed {
                page = fail;
                continue;
            }
            for a in &def.actions {
                self.npc_action(id, a);
            }
            if def.say.trim().is_empty() {
                if def.success_page != 0 {
                    page = def.success_page;
                    continue;
                }
                self.npc_close(id);
                self.send_to(id, ServerMessage::NpcClose);
                return;
            }
            let goods = def
                .goods
                .iter()
                .filter_map(|(item, rate)| {
                    let d = self.data.items.get(item)?;
                    Some(Good {
                        info: *item,
                        price: ((d.price as f64 * rate).round() as u64).max(1),
                    })
                })
                .collect();
            if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
                p.npc = Some((npc, page));
            }
            // Strip markup the client does not render.
            let say = def.say.clone();
            let _ = parse_dialog(&say);
            self.send_to(
                id,
                ServerMessage::NpcResponse {
                    npc,
                    page,
                    say,
                    dialog_type: def.dialog_type,
                    goods,
                    sell_types: def.types.clone(),
                },
            );
            return;
        }
    }

    fn npc_check(&mut self, id: ObjectId, c: &crate::data::NpcCheckDef) -> bool {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return false;
        };
        let cmp = |op: i32, a: i64, b: i64| match op {
            0 => a == b,
            1 => a != b,
            2 => a < b,
            3 => a <= b,
            4 => a > b,
            5 => a >= b,
            _ => true,
        };
        // Zircon `Config.RedPoint`.
        const RED_POINT: i64 = 200;
        let weapon = p.bag.equipment.get(slot::WEAPON).and_then(|w| w.as_ref());
        let equal = c.operator == 0;
        match c.check_type {
            0 => cmp(c.operator, p.level as i64, c.int1 as i64),
            1 => cmp(c.operator, p.class.mir_class() as i64, c.int1 as i64),
            // Gender has no server case in Zircon: always passes.
            2 => true,
            3 => cmp(c.operator, p.bag.gold as i64, c.int1 as i64),
            4 => c.item1 == 0 || cmp(c.operator, p.bag.count_of(c.item1) as i64, c.int1 as i64),
            // PK points: nobody is red in the prototype.
            5 => {
                let threshold = if c.int1 == 0 {
                    RED_POINT
                } else {
                    c.int1 as i64
                };
                cmp(c.operator, 0, threshold)
            }
            6 => weapon.is_some() == equal,
            // Weapon level / element / added stats: no refining yet, so the
            // weapon counts as level 0 with no element (Zircon would throw
            // without a weapon; treat that as failing).
            7 => weapon.is_some() && cmp(c.operator, 0, c.int1 as i64),
            8 => weapon.is_some() && cmp(c.operator, 0, c.int2 as i64),
            9 => weapon.is_some() && !equal,
            16 => weapon.is_some() && cmp(c.operator, 0, c.int1 as i64),
            // Horse: none owned.
            10 => cmp(c.operator, 0, c.int1 as i64),
            // Marriage and wedding ring: not married.
            11 | 12 => !equal,
            13 => {
                c.item1 == 0
                    || p.bag
                        .can_gain(&self.data, c.item1, c.int1.max(1) as u32, p.max_bag)
            }
            // Weapon reset cooldown: never on cooldown.
            14 => weapon.is_some() && equal,
            15 => {
                let roll = self.rng.random_range(0..c.int1.max(1)) as i64;
                cmp(c.operator, roll, c.int2 as i64)
            }
            // Currency by name: only gold exists.
            17 => {
                if c.string1.eq_ignore_ascii_case("gold") {
                    cmp(c.operator, p.bag.gold as i64, c.int1 as i64)
                } else {
                    true
                }
            }
            // Roll results, data lists and fame do not exist yet: fail like
            // Zircon does when the data is missing.
            18 | 19 | 21 => false,
            20 => cmp(c.operator, 0, c.int2 as i64),
            _ => true,
        }
    }

    fn npc_action(&mut self, id: ObjectId, a: &crate::data::NpcActionDef) {
        match a.action_type {
            0 => {
                // Teleport to MapParameter1 at (int1, int2) or a random cell.
                let Some(map) = self.data.maps.get(&a.map1).map(|m| m.index) else {
                    return;
                };
                if self.ensure_map(map).is_err() {
                    return;
                }
                let target = if a.int1 == 0 && a.int2 == 0 {
                    self.random_walkable(map)
                } else {
                    Some(Point::new(a.int1, a.int2))
                };
                if let Some(t) = target {
                    self.change_map(id, map, t);
                }
            }
            1 | 2 => {
                if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
                    if a.action_type == 1 {
                        p.bag.gold += a.int1.max(0) as u64;
                    } else {
                        p.bag.gold = p.bag.gold.saturating_sub(a.int1.max(0) as u64);
                    }
                }
                self.send_gold(id);
            }
            3 => {
                let count = a.int1.max(1) as u32;
                if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
                    let mut next = p.next_item_id;
                    let changes = p.bag.gain(&self.data, a.item1, count, &mut next);
                    p.next_item_id = next;
                    self.send_changes(id, changes);
                }
            }
            4 => {
                let count = a.int1.max(1) as u32;
                if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
                    let changes = p.bag.take_info(a.item1, count);
                    self.send_changes(id, changes);
                }
            }
            _ => {}
        }
    }

    pub fn npc_buy(&mut self, id: ObjectId, info: i32, count: u32) {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let Some((_, page)) = p.npc else {
            return;
        };
        let Some(def) = self.data.npc_pages.get(&page) else {
            return;
        };
        let Some((_, rate)) = def.goods.iter().find(|(i, _)| *i == info) else {
            return;
        };
        let Some(item) = self.data.items.get(&info) else {
            return;
        };
        let count = count.clamp(1, item.stack_size.max(1) as u32);
        let price = ((item.price as f64 * rate).round() as u64).max(1);
        let total = price * count as u64;
        if p.bag.gold < total {
            self.send_to(
                id,
                ServerMessage::Chat {
                    text: "Not enough gold.".into(),
                },
            );
            return;
        }
        if !p.bag.can_gain(&self.data, info, count, p.max_bag) {
            self.send_to(
                id,
                ServerMessage::Chat {
                    text: "You cannot carry that.".into(),
                },
            );
            return;
        }
        let name = item.name.clone();
        let o = self.objects.get_mut(&id).unwrap();
        let p = o.player_mut().unwrap();
        p.bag.gold -= total;
        let mut next = p.next_item_id;
        let changes = p.bag.gain(&self.data, info, count, &mut next);
        p.next_item_id = next;
        self.send_gold(id);
        self.send_changes(id, changes);
        self.send_to(
            id,
            ServerMessage::Chat {
                text: format!("Bought {name} x{count} for {total} gold."),
            },
        );
    }

    pub fn npc_sell(&mut self, id: ObjectId, slots: Vec<u8>) {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let Some((_, page)) = p.npc else {
            return;
        };
        let Some(def) = self.data.npc_pages.get(&page).cloned() else {
            return;
        };
        if def.dialog_type != 1 || def.types.is_empty() {
            return;
        }
        let mut earned = 0u64;
        let mut sold = 0u32;
        let mut changes = Changed::new();
        for slot in slots {
            let Some(item) = self.objects[&id]
                .player()
                .unwrap()
                .bag
                .inventory
                .get(slot as usize)
                .cloned()
                .flatten()
            else {
                continue;
            };
            let Some(idef) = self.data.items.get(&item.info) else {
                continue;
            };
            if !idef.can_sell || !def.types.contains(&idef.item_type) {
                continue;
            }
            let price = sell_price(idef, &item);
            let o = self.objects.get_mut(&id).unwrap();
            let p = o.player_mut().unwrap();
            if let Some(c) = p.bag.take(Grid::Inventory, slot, item.count) {
                changes.push(c);
            }
            p.bag.gold += price;
            earned += price;
            sold += item.count;
        }
        self.send_gold(id);
        self.send_changes(id, changes);
        if sold > 0 {
            self.send_to(
                id,
                ServerMessage::Chat {
                    text: format!("Sold {sold} item(s) for {earned} gold."),
                },
            );
        }
    }

    // ---- map travel -------------------------------------------------------------

    fn random_walkable(&mut self, map: i32) -> Option<Point> {
        let (w, h) = {
            let m = self.maps.get(&map)?;
            (m.file.width as i32, m.file.height as i32)
        };
        for _ in 0..10_000 {
            let p = Point::new(self.rng.random_range(0..w), self.rng.random_range(0..h));
            if self.maps[&map].file.is_walkable(p.x, p.y) && !self.cell_blocked(map, p, true) {
                return Some(p);
            }
        }
        None
    }

    /// Zircon `Cell.GetMovement`: pick a random movement on the cell, a random
    /// destination cell, check level, then relocate.
    fn try_travel(&mut self, id: ObjectId) -> bool {
        let Some(o) = self.objects.get(&id) else {
            return false;
        };
        if !o.is_player() {
            return false;
        }
        let (map, loc, level) = (o.map, o.location, o.player().unwrap().level);
        let Some(indices) = self
            .maps
            .get(&map)
            .and_then(|m| m.movements.get(&(loc.x, loc.y)).cloned())
        else {
            return false;
        };
        for _ in 0..5 {
            let mi = indices[self.rng.random_range(0..indices.len())];
            let m = self.data.movements[mi].clone();
            let Some(dest) = self.data.regions.get(&m.destination_region).cloned() else {
                continue;
            };
            let Some(dest_map) = self.data.maps.get(&dest.map).cloned() else {
                continue;
            };
            if dest_map.minimum_level > level {
                self.send_to(
                    id,
                    ServerMessage::Chat {
                        text: format!(
                            "You need level {} to enter {}.",
                            dest_map.minimum_level, dest_map.description
                        ),
                    },
                );
                return false;
            }
            if self.ensure_map(dest_map.index).is_err() {
                continue;
            }
            let width = self.maps[&dest_map.index].file.width as i32;
            let points: Vec<Point> = dest
                .points(width)
                .into_iter()
                .map(|(x, y)| Point::new(x, y))
                .filter(|p| self.maps[&dest_map.index].file.is_walkable(p.x, p.y))
                .collect();
            let Some(target) = self.random_point(&points) else {
                continue;
            };
            self.change_map(id, dest_map.index, target);
            return true;
        }
        false
    }

    /// Move a player to another map (or cell) and resync the client.
    fn change_map(&mut self, id: ObjectId, map: i32, to: Point) {
        if self.ensure_map(map).is_err() {
            return;
        }
        let (old_map, old_loc) = {
            let o = &self.objects[&id];
            (o.map, o.location)
        };
        if let Some(m) = self.maps.get_mut(&old_map) {
            m.objects.retain(|x| *x != id);
            m.remove_from_cell(id, old_loc);
        }
        {
            let o = self.objects.get_mut(&id).unwrap();
            o.map = map;
            o.location = to;
            o.cell_time = self.now + CELL_GRACE;
            if let Some(p) = o.player_mut() {
                p.npc = None;
            }
        }
        let m = self.maps.get_mut(&map).unwrap();
        m.objects.push(id);
        m.add_to_cell(id, to);
        let desc = m.descriptor.clone();
        let dir = self.objects[&id].direction;
        // Forget everything seen; visibility will re-add what is around.
        let old: Vec<ObjectId> = self.objects[&id].visible.iter().copied().collect();
        for v in old {
            self.send_to(id, ServerMessage::ObjectRemove { id: v });
        }
        self.objects.get_mut(&id).unwrap().visible.clear();
        self.send_to(
            id,
            ServerMessage::MapChanged {
                map: desc,
                location: to,
                direction: dir,
            },
        );
        // Landing on another movement cell chains (Zircon recurses).
        if old_map != map || old_loc != to {
            let _ = self.try_travel(id);
        }
    }

    // ---- magic ----------------------------------------------------------------------

    fn send_magics(&mut self, id: ObjectId) {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let list = p.magics.iter().map(|m| m.summary()).collect();
        self.send_to(id, ServerMessage::Magics(list));
    }

    /// Zircon book use: learn the magic the book's `Shape` points at.
    fn learn_book(
        &mut self,
        id: ObjectId,
        slot: u8,
        def: &crate::data::ItemDef,
    ) -> Result<Changed, String> {
        let magic = self
            .data
            .magic_by_index(def.shape)
            .cloned()
            .ok_or("This book teaches nothing")?;
        if magic.school == 0 {
            return Err("This skill is disabled".into());
        }
        let o = self.objects.get_mut(&id).ok_or("no player")?;
        let p = o.player_mut().ok_or("no player")?;
        if p.class.mir_class() != magic.class {
            return Err("Your class cannot learn this".into());
        }
        if let Some(known) = p.magics.iter().find(|m| m.magic == magic.magic) {
            if known.level < 3 {
                return Err(format!("You already know {}", magic.name));
            }
            return Err("Level 4 skills are not supported yet".into());
        }
        let book = p
            .bag
            .inventory
            .get(slot as usize)
            .cloned()
            .flatten()
            .ok_or("Nothing there")?;
        let change = p.bag.take(Grid::Inventory, slot, 1);
        // Success chance = the book's current durability (100 when new).
        if self.rng.random_range(0..100) >= book.durability.max(0) {
            self.send_to(
                id,
                ServerMessage::Chat {
                    text: format!("You failed to learn {}.", magic.name),
                },
            );
            return Ok(change.into_iter().collect());
        }
        let um = UserMagic {
            magic: magic.magic,
            level: 0,
            experience: 0,
            key: 0,
            cooldown_until: 0,
        };
        let summary = um.summary();
        self.objects
            .get_mut(&id)
            .unwrap()
            .player_mut()
            .unwrap()
            .magics
            .push(um);
        self.send_to(id, ServerMessage::NewMagic(summary));
        self.send_to(
            id,
            ServerMessage::Chat {
                text: format!("You learned {}.", magic.name),
            },
        );
        Ok(change.into_iter().collect())
    }

    pub fn magic_key(&mut self, id: ObjectId, magic: u16, key: u8) {
        let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) else {
            return;
        };
        if key != 0 {
            for m in p.magics.iter_mut() {
                if m.key == key {
                    m.key = 0;
                }
            }
        }
        if let Some(m) = p.magics.iter_mut().find(|m| m.magic == magic) {
            m.key = key.min(12);
        }
    }

    pub fn magic_toggle(&mut self, id: ObjectId, magic: u16, on: bool) {
        let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) else {
            return;
        };
        if !p.magics.iter().any(|m| m.magic == magic) {
            return;
        }
        match magic {
            magic_type::THRUSTING => p.thrusting_on = on,
            magic_type::HALF_MOON => p.half_moon_on = on,
            _ => return,
        }
        self.send_to(id, ServerMessage::MagicToggle { magic, on });
    }

    /// Zircon `LevelMagic`: 1..=3 experience per success while the player
    /// level allows it; levels up at the thresholds.
    fn level_magic(&mut self, id: ObjectId, magic: u16) {
        let Some(def) = self.data.magics.get(&magic).cloned() else {
            return;
        };
        let exp = self.rng.random_range(1..=SKILL_EXP) as u64;
        let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) else {
            return;
        };
        let level = p.level;
        let Some(m) = p.magics.iter_mut().find(|m| m.magic == magic) else {
            return;
        };
        let Some(need) = m.need_level(&def) else {
            return;
        };
        let Some(max) = m.next_experience(&def) else {
            return;
        };
        if level < need || m.level >= 3 {
            return;
        }
        m.experience += exp;
        let mut leveled = false;
        if m.experience as i64 >= max && max > 0 {
            m.experience -= max as u64;
            m.level += 1;
            leveled = true;
        }
        let (lvl, xp) = (m.level, m.experience);
        self.send_to(
            id,
            ServerMessage::MagicLeveled {
                magic,
                level: lvl,
                experience: xp,
            },
        );
        if leveled {
            self.refresh_stats(id, false);
            self.send_player_stats(id);
            self.send_to(
                id,
                ServerMessage::Chat {
                    text: format!("{} is now level {}.", def.name, lvl),
                },
            );
        }
    }

    /// Zircon `PlayerObject.Magic`: validate, pay, schedule the effect,
    /// broadcast the cast.
    pub fn cast(
        &mut self,
        id: ObjectId,
        magic: u16,
        direction: Direction,
        target: Option<ObjectId>,
        location: Point,
    ) {
        let Some(def) = self.data.magics.get(&magic).cloned() else {
            return;
        };
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let Some(p) = o.player() else {
            return;
        };
        let (map, loc) = (o.map, o.location);
        let Some(um) = p.magics.iter().find(|m| m.magic == magic).cloned() else {
            return;
        };
        let deny = |w: &mut World, why: &str| {
            w.send_to(id, ServerMessage::Chat { text: why.into() });
            w.send_to(
                id,
                ServerMessage::MoveDenied {
                    location: loc,
                    direction,
                },
            );
        };
        if !magic_type::is_castable(magic) {
            deny(self, "That skill cannot be cast.");
            return;
        }
        if p.level < def.need_level[0] {
            deny(
                self,
                &format!("{} needs level {}.", def.name, def.need_level[0]),
            );
            return;
        }
        if o.dead || self.now < o.action_time || self.now < p.magic_time {
            deny(self, "");
            return;
        }
        if self.now < um.cooldown_until {
            deny(self, &format!("{} is still cooling down.", def.name));
            return;
        }
        let cost = um.cost(&def);
        if cost > p.mp {
            deny(self, "Not enough mana.");
            return;
        }
        // Target must be visible and within magic range.
        let target = target.filter(|t| {
            self.objects
                .get(t)
                .map(|to| to.map == map && !to.dead && to.location.distance(loc) <= MAGIC_RANGE)
                .unwrap_or(false)
        });
        let mut targets: Vec<ObjectId> = Vec::new();
        let mut locations: Vec<Point> = Vec::new();
        let mut pending: Vec<PendingMagic> = Vec::new();
        let dist = |w: &World, t: ObjectId| w.objects[&t].location.distance(loc) as u64;
        match magic {
            magic_type::FIRE_BALL
            | magic_type::ICE_BOLT
            | magic_type::FLAMING_DAGGERS
            | magic_type::SHREDDING => {
                let base = if matches!(magic, magic_type::FIRE_BALL | magic_type::ICE_BOLT) {
                    500
                } else {
                    1000
                };
                match target.filter(|t| self.objects[t].is_monster()) {
                    Some(t) => {
                        targets.push(t);
                        pending.push(PendingMagic {
                            time: self.now + base + dist(self, t) * 48,
                            caster: id,
                            magic,
                            target: Some(t),
                            location,
                            direction: None,
                        });
                    }
                    None => locations.push(location),
                }
            }
            magic_type::THUNDER_BOLT => match target.filter(|t| self.objects[t].is_monster()) {
                Some(t) => {
                    targets.push(t);
                    pending.push(PendingMagic {
                        time: self.now + 600,
                        caster: id,
                        magic,
                        target: Some(t),
                        location,
                        direction: None,
                    });
                }
                None => locations.push(location),
            },
            magic_type::REPULSION => {
                for d in Direction::ALL {
                    pending.push(PendingMagic {
                        time: self.now + 500,
                        caster: id,
                        magic,
                        target: None,
                        location: loc.step(d, 1),
                        direction: Some(d),
                    });
                }
            }
            magic_type::HEAL => {
                let t = target.filter(|t| self.objects[t].is_player()).unwrap_or(id);
                targets.push(t);
                pending.push(PendingMagic {
                    time: self.now + 500,
                    caster: id,
                    magic,
                    target: Some(t),
                    location,
                    direction: None,
                });
            }
            magic_type::POISON_DUST => match target.filter(|t| self.objects[t].is_monster()) {
                Some(t) => {
                    targets.push(t);
                    pending.push(PendingMagic {
                        time: self.now + 500,
                        caster: id,
                        magic,
                        target: Some(t),
                        location,
                        direction: None,
                    });
                }
                None => locations.push(location),
            },
            _ => {}
        }
        // Pay, set timers (Zircon: consume even when the spell fizzles).
        let face = match target {
            Some(t) if t != id => Direction::from_points(loc, self.objects[&t].location),
            _ => direction,
        };
        {
            let o = self.objects.get_mut(&id).unwrap();
            o.action_time = self.now + CAST_TIME;
            o.direction = face;
            let p = o.player_mut().unwrap();
            p.mp -= cost;
            p.magic_time = self.now + MAGIC_DELAY;
            if let Some(m) = p.magics.iter_mut().find(|m| m.magic == magic) {
                m.cooldown_until = self.now + def.delay.max(0) as u64;
            }
        }
        let dir = self.objects[&id].direction;
        if def.delay > 0 {
            self.send_to(
                id,
                ServerMessage::MagicCooldown {
                    magic,
                    delay_ms: def.delay as u32,
                },
            );
        }
        self.send_player_stats(id);
        self.events.push((
            id,
            ServerMessage::ObjectMagic {
                id,
                direction: dir,
                location: loc,
                magic,
                targets,
                locations,
                cast: true,
            },
        ));
        self.pending_magics.extend(pending);
    }

    fn roll_range(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            max
        } else {
            self.rng.random_range(min..=max)
        }
    }

    /// Spell damage (Zircon `MagicAttack`): magic power plus the class stat,
    /// minus the target's MR and element resistance.
    fn magic_attack(&mut self, caster: ObjectId, target: ObjectId, magic: u16, elem: u8) -> i32 {
        let Some(def) = self.data.magics.get(&magic).cloned() else {
            return 0;
        };
        let Some(c) = self.objects.get(&caster) else {
            return 0;
        };
        let Some(p) = c.player() else {
            return 0;
        };
        let Some(um) = p.magics.iter().find(|m| m.magic == magic).cloned() else {
            return 0;
        };
        let (pmin, pmax) = um.power_range(&def);
        let cs = c.stats;
        let class = p.class;
        let Some(t) = self.objects.get(&target) else {
            return 0;
        };
        if t.dead || !t.is_monster() {
            return 0;
        }
        let ts = t.stats;
        let resist = match &t.kind {
            Kind::Monster(m) => {
                let d = &self.data.monsters[&m.def];
                match elem {
                    element::FIRE => d.stat(21),
                    element::ICE => d.stat(23),
                    element::LIGHTNING => d.stat(25),
                    element::WIND => d.stat(27),
                    element::HOLY => d.stat(29),
                    element::DARK => d.stat(31),
                    element::PHANTOM => d.stat(33),
                    _ => 0,
                }
            }
            _ => 0,
        };
        let mut power = if pmin >= pmax {
            pmin
        } else {
            self.rng.random_range(pmin..=pmax)
        };
        power += match class {
            Class::Wizard => self.roll_range(cs.min_mc, cs.max_mc),
            Class::Taoist => self.roll_range(cs.min_sc, cs.max_sc),
            Class::Assassin => self.roll_range(cs.min_mc.min(cs.min_sc), cs.max_mc.min(cs.max_sc)),
            Class::Warrior => 0,
        };
        power -= self.roll_range(ts.min_mr, ts.max_mr);
        if resist != 0 {
            power -= power * resist / 10;
        }
        if power <= 0 {
            return 0;
        }
        if self.rng.random_range(0..100) < 1 {
            power = power * 12 / 10;
        }
        let dealt = self.damage(target, caster, power, elem, true);
        if dealt > 0 {
            self.level_magic(caster, magic);
        }
        dealt
    }

    fn resolve_magics(&mut self) {
        let due: Vec<PendingMagic> = {
            let (due, later): (Vec<_>, Vec<_>) = self
                .pending_magics
                .drain(..)
                .partition(|m| m.time <= self.now);
            self.pending_magics = later;
            due
        };
        for pm in due {
            let Some(c) = self.objects.get(&pm.caster) else {
                continue;
            };
            if c.dead {
                continue;
            }
            let (cmap, cloc, clevel) =
                (c.map, c.location, c.player().map(|p| p.level).unwrap_or(1));
            match pm.magic {
                magic_type::FIRE_BALL | magic_type::FLAMING_DAGGERS | magic_type::SHREDDING => {
                    if let Some(t) = pm.target {
                        self.magic_attack(pm.caster, t, pm.magic, element::FIRE);
                    }
                }
                magic_type::ICE_BOLT => {
                    if let Some(t) = pm.target {
                        self.magic_attack(pm.caster, t, pm.magic, element::ICE);
                    }
                }
                magic_type::THUNDER_BOLT => {
                    if let Some(t) = pm.target {
                        self.magic_attack(pm.caster, t, pm.magic, element::LIGHTNING);
                    }
                }
                magic_type::REPULSION => {
                    let Some(dir) = pm.direction else { continue };
                    let Some(m) = self.maps.get(&cmap) else {
                        continue;
                    };
                    let victims: Vec<ObjectId> = m.objects_at(pm.location).to_vec();
                    let (lvl, power) = {
                        let p = self.objects[&pm.caster].player().unwrap();
                        let um = p.magics.iter().find(|m| m.magic == pm.magic).unwrap();
                        let def = &self.data.magics[&pm.magic];
                        (um.level as i32, um.power_range(def))
                    };
                    for v in victims {
                        let (start_loc, mlevel, is_boss) = match self.objects.get(&v) {
                            Some(o) if o.is_monster() && !o.dead => match &o.kind {
                                Kind::Monster(m) => (
                                    o.location,
                                    self.data.monsters[&m.def].level,
                                    self.data.monsters[&m.def].is_boss,
                                ),
                                _ => continue,
                            },
                            _ => continue,
                        };
                        if is_boss || mlevel >= clevel {
                            continue;
                        }
                        if self.rng.random_range(0..16) >= 6 + lvl * 3 + clevel - mlevel {
                            continue;
                        }
                        let distance = self.roll_range(power.0, power.1);
                        let mut from = start_loc;
                        let mut moved = 0;
                        for _ in 0..distance {
                            let next = from.step(dir, 1);
                            if self.cell_blocked(cmap, next, false) {
                                break;
                            }
                            from = next;
                            moved += 1;
                        }
                        if moved > 0 {
                            let start = self.objects[&v].location;
                            self.move_object(v, from);
                            self.events.push((
                                v,
                                ServerMessage::ObjectMove {
                                    id: v,
                                    from: start,
                                    to: from,
                                    direction: dir,
                                    run: moved > 1,
                                },
                            ));
                            self.level_magic(pm.caster, pm.magic);
                        }
                    }
                }
                magic_type::HEAL => {
                    let Some(t) = pm.target else { continue };
                    let Some(to) = self.objects.get(&t) else {
                        continue;
                    };
                    if to.dead || to.hp >= to.max_hp || to.heal.is_some() {
                        continue;
                    }
                    let (pmin, pmax, sc) = {
                        let c = &self.objects[&pm.caster];
                        let p = c.player().unwrap();
                        let um = p.magics.iter().find(|m| m.magic == pm.magic).unwrap();
                        let def = &self.data.magics[&pm.magic];
                        let (pmin, pmax) = um.power_range(def);
                        (pmin, pmax, (c.stats.min_sc, c.stats.max_sc))
                    };
                    let healing = self.roll_range(pmin, pmax) + self.roll_range(sc.0, sc.1);
                    if healing <= 0 {
                        continue;
                    }
                    self.objects.get_mut(&t).unwrap().heal = Some(HealBuff {
                        pool: healing,
                        cap: 30,
                        next_tick: self.now,
                    });
                    self.level_magic(pm.caster, pm.magic);
                }
                magic_type::POISON_DUST => {
                    let Some(t) = pm.target else { continue };
                    let Some(to) = self.objects.get(&t) else {
                        continue;
                    };
                    if to.dead || !to.is_monster() {
                        continue;
                    }
                    let (lvl, duration, kind, poison_slot) = {
                        let c = &self.objects[&pm.caster];
                        let p = c.player().unwrap();
                        let um = p.magics.iter().find(|m| m.magic == pm.magic).unwrap();
                        let def = &self.data.magics[&pm.magic];
                        let (pmin, pmax) = um.power_range(def);
                        // Zircon consumes one equipped poison; its shape picks Green/Red.
                        let poison_item =
                            p.bag.equipment.get(slot::POISON).and_then(|s| s.as_ref());
                        let kind = match poison_item.and_then(|i| self.data.items.get(&i.info)) {
                            Some(d) if d.shape != 0 => 2,
                            _ => 1,
                        };
                        (
                            um.level as i32,
                            (pmin, pmax, c.stats.min_sc, c.stats.max_sc),
                            kind,
                            poison_item.map(|_| slot::POISON as u8),
                        )
                    };
                    if let Some(ps) = poison_slot {
                        let o = self.objects.get_mut(&pm.caster).unwrap();
                        let change = o.player_mut().unwrap().bag.take(Grid::Equipment, ps, 1);
                        self.send_changes(pm.caster, change.into_iter().collect());
                    }
                    let dur = self.roll_range(duration.0, duration.1)
                        + self.roll_range(duration.2, duration.3);
                    let value = lvl + 1 + clevel / 14;
                    let poison = Poison {
                        kind,
                        value,
                        ticks_left: (dur / 2).max(1),
                        next_tick: self.now + 2000,
                        owner: Some(pm.caster),
                    };
                    self.apply_poison(t, poison);
                    self.level_magic(pm.caster, pm.magic);
                }
                _ => {}
            }
            let _ = cloc;
        }
    }

    /// Zircon `ApplyPoison`: a stronger instance of the same type wins.
    fn apply_poison(&mut self, target: ObjectId, poison: Poison) {
        let Some(o) = self.objects.get_mut(&target) else {
            return;
        };
        if let Some(existing) = o.poisons.iter().position(|p| p.kind == poison.kind) {
            if o.poisons[existing].value > poison.value {
                return;
            }
            o.poisons.remove(existing);
        }
        let was = !o.poisons.is_empty();
        o.poisons.push(poison);
        if !was {
            self.events.push((
                target,
                ServerMessage::ObjectPoisoned {
                    id: target,
                    poisoned: true,
                },
            ));
        }
    }

    fn process_poisons(&mut self) {
        let now = self.now;
        let ids: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| !o.poisons.is_empty())
            .map(|o| o.id)
            .collect();
        for id in ids {
            let (damage, owner, cleared) = {
                let o = self.objects.get_mut(&id).unwrap();
                let mut damage = 0;
                let mut owner = None;
                for p in o.poisons.iter_mut() {
                    if now < p.next_tick {
                        continue;
                    }
                    p.next_tick = now + 2000;
                    p.ticks_left -= 1;
                    if p.kind == 1 {
                        damage += p.value;
                        owner = p.owner;
                    }
                }
                o.poisons.retain(|p| p.ticks_left >= 0);
                let cleared = o.poisons.is_empty();
                if o.dead {
                    damage = 0;
                }
                // Poison never kills (Zircon `CanKill = false`).
                damage = damage.min(o.hp - 1).max(0);
                (damage, owner, cleared)
            };
            if damage > 0 {
                let attacker = owner.unwrap_or(id);
                let o = self.objects.get_mut(&id).unwrap();
                o.hp -= damage;
                let (hp, max_hp) = (o.hp, o.max_hp);
                self.events
                    .push((id, ServerMessage::HealthChanged { id, hp, max_hp }));
                if let Some(m) = self.objects.get_mut(&id).and_then(|o| o.monster_mut()) {
                    if m.target.is_none() && attacker != id {
                        m.target = Some(attacker);
                    }
                }
            }
            if cleared {
                self.events.push((
                    id,
                    ServerMessage::ObjectPoisoned {
                        id,
                        poisoned: false,
                    },
                ));
            }
        }
    }

    fn process_heals(&mut self) {
        let now = self.now;
        let ids: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| o.heal.is_some())
            .map(|o| o.id)
            .collect();
        for id in ids {
            let o = self.objects.get_mut(&id).unwrap();
            let Some(h) = o.heal.as_mut() else { continue };
            if now < h.next_tick {
                continue;
            }
            h.next_tick = now + 1000;
            let amount = h.pool.min(h.cap);
            h.pool -= amount;
            o.hp = (o.hp + amount).min(o.max_hp);
            if o.hp >= o.max_hp || h.pool <= 0 || o.dead {
                o.heal = None;
            }
            let (hp, max_hp) = (o.hp, o.max_hp);
            self.events
                .push((id, ServerMessage::HealthChanged { id, hp, max_hp }));
            if self.objects[&id].is_player() {
                self.send_player_stats(id);
            }
        }
    }

    /// Recompute each player's visible set and emit show/remove.
    fn update_visibility(&mut self) {
        let players: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| o.is_player())
            .map(|o| o.id)
            .collect();
        for pid in players {
            let (map, loc, conn, account) = {
                let p = &self.objects[&pid];
                let pd = p.player().unwrap();
                (p.map, p.location, pd.conn, pd.account)
            };
            let now = self.now;
            let now_visible: HashSet<ObjectId> = self.maps[&map]
                .objects
                .iter()
                .copied()
                .filter(|id| *id != pid)
                .filter(|id| {
                    let o = &self.objects[id];
                    if o.location.distance(loc) > MAX_VIEW_RANGE {
                        return false;
                    }
                    match &o.kind {
                        Kind::Item(i) => {
                            i.owner.is_none_or(|a| a == account)
                                || now >= i.spawn_time + DROP_SHARE_AFTER
                        }
                        _ => true,
                    }
                })
                .collect();
            let old = std::mem::take(&mut self.objects.get_mut(&pid).unwrap().visible);
            for id in old.difference(&now_visible) {
                self.outgoing
                    .push(Outgoing::To(conn, ServerMessage::ObjectRemove { id: *id }));
            }
            for id in now_visible.difference(&old) {
                let state = self.objects[id].state();
                self.outgoing
                    .push(Outgoing::To(conn, ServerMessage::ObjectShow(state)));
            }
            self.objects.get_mut(&pid).unwrap().visible = now_visible;
        }
    }

    /// Deliver this tick's events to every player that can see the subject
    /// (Zircon `Broadcast` to `SeenByPlayers`). Self-originated movement and
    /// attacks are not echoed; the client predicts those.
    fn flush_events(&mut self) {
        let events = std::mem::take(&mut self.events);
        let players: Vec<(ObjectId, ConnId)> = self
            .objects
            .values()
            .filter_map(|o| o.player().map(|p| (o.id, p.conn)))
            .collect();
        for (subject, msg) in events {
            let echo_self = !matches!(
                msg,
                ServerMessage::ObjectMove { .. }
                    | ServerMessage::ObjectTurn { .. }
                    | ServerMessage::ObjectAttack { .. }
            );
            for (pid, conn) in &players {
                let sees =
                    *pid == subject && echo_self || self.objects[pid].visible.contains(&subject);
                if sees {
                    self.outgoing.push(Outgoing::To(*conn, msg.clone()));
                }
            }
        }
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
    fn monster_mut_ref(&self) -> &MonsterData {
        self.monster_ref()
    }
}
