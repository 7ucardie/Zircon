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
    Appearance, Class, Direction, MapDescriptor, ObjectId, ObjectState, PlayerStats, Point,
    ServerMessage,
};
use rand::Rng;

use crate::accounts::CharacterRecord;
use crate::data::{GameData, MonsterDef, RespawnDef};

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
/// Prototype: revive after 10 s instead of Zircon's 10 minutes.
pub const REVIVE_DELAY: u64 = 10_000;
pub const CELL_GRACE: u64 = 300;

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
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct PlayerData {
    pub conn: ConnId,
    pub name: String,
    pub class: Class,
    /// Account character id, for persistence.
    pub character: u32,
    pub level: i32,
    pub experience: u64,
    pub max_mp: i32,
    pub mp: i32,
    pub revive_time: u64,
    pub bind_region: i32,
    pub regen_time: u64,
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
pub enum Kind {
    Player(PlayerData),
    Monster(MonsterData),
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
    pub fn blocking(&self) -> bool {
        !self.dead
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
    /// Events raised this tick: (subject object, message).
    events: Vec<(ObjectId, ServerMessage)>,
    pub outgoing: Vec<Outgoing>,
    last_spawn_check: u64,
    force_map: Option<String>,
}

impl World {
    pub fn new(data: GameData, map_dir: impl AsRef<Path>, force_map: Option<String>) -> World {
        World {
            data,
            map_dir: map_dir.as_ref().to_path_buf(),
            maps: HashMap::new(),
            objects: HashMap::new(),
            next_id: 1,
            now: 0,
            rng: rand::rng(),
            pending_hits: Vec::new(),
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
            },
        );
        self.do_spawns(index);
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
    pub fn add_player(&mut self, conn: ConnId, rec: &CharacterRecord) -> anyhow::Result<ObjectId> {
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
        let level = rec.level.max(1);
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
        let obj = Object {
            id,
            kind: Kind::Player(PlayerData {
                conn,
                name: rec.name.clone(),
                class,
                character: rec.id,
                level,
                experience: rec.experience,
                max_mp: base.mana,
                mp,
                revive_time: 0,
                bind_region,
                regen_time: self.now + REGEN_DELAY,
            }),
            map,
            location,
            direction: rec.direction,
            hp,
            max_hp: base.health,
            dead: false,
            stats: CombatStats {
                accuracy: base.accuracy,
                agility: base.agility,
                min_ac: base.min_ac,
                max_ac: base.max_ac,
                min_dc: base.min_dc,
                max_dc: base.max_dc,
            },
            action_time: 0,
            move_time: 0,
            attack_time: 0,
            cell_time: 0,
            appearance: Appearance::Player {
                name: rec.name.clone(),
                gender: rec.gender,
                class,
                armour: 0,
                weapon: 0,
                hair: rec.hair.max(1),
            },
            visible: HashSet::new(),
        };
        self.insert_object(obj);
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
        Ok(id)
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

    pub fn player_attack(&mut self, id: ObjectId, direction: Direction) {
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
        o.attack_time = self.now + attack_delay(0);
        let stats = o.stats;
        let target_cell = (o.map, o.location.step(direction, 1));
        let power = self.roll_dc(stats);
        self.events
            .push((id, ServerMessage::ObjectAttack { id, direction }));
        self.pending_hits.push(PendingHit {
            time: self.now + 300,
            attacker: id,
            target_cell,
            power,
            target: None,
        });
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
                if attacker_is_player == target.is_player() {
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
                let mut power = hit.power - self.roll_ac(tstats);
                if power <= 0 {
                    continue;
                }
                if attacker_is_player && self.rng.random_range(0..100) < 1 {
                    power *= 2; // CriticalChance 1, CriticalDamage 0
                }
                self.damage(tid, hit.attacker, power);
            }
        }
    }

    fn damage(&mut self, target: ObjectId, attacker: ObjectId, power: i32) {
        let now = self.now;
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
        if leveled {
            if let Some(base) = self.data.base_stat(class.mir_class(), level).cloned() {
                let o = self.objects.get_mut(&id).unwrap();
                o.max_hp = base.health;
                o.hp = base.health;
                o.stats = CombatStats {
                    accuracy: base.accuracy,
                    agility: base.agility,
                    min_ac: base.min_ac,
                    max_ac: base.max_ac,
                    min_dc: base.min_dc,
                    max_dc: base.max_dc,
                };
                let p = o.player_mut().unwrap();
                p.max_mp = base.mana;
                p.mp = base.mana;
            }
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
                text: "You have died. Reviving shortly...".into(),
            },
        );
    }

    fn revive_player(&mut self, id: ObjectId) -> anyhow::Result<()> {
        let (map, bind_region) = {
            let o = &self.objects[&id];
            (o.map, o.player().unwrap().bind_region)
        };
        let location = if bind_region != 0 {
            self.bind_point(map, bind_region)?
        } else {
            self.objects[&id].location
        };
        self.move_object(id, location);
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
        self.events
            .push((id, ServerMessage::ObjectAttack { id, direction: dir }));
        self.pending_hits.push(PendingHit {
            time: self.now + 400,
            attacker: id,
            target_cell: (map, loc.step(dir, 1)),
            power,
            target: Some(target),
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
                o.player_mut().unwrap().regen_time = now + REGEN_DELAY;
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
        let active: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| !o.is_player())
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

    /// Recompute each player's visible set and emit show/remove.
    fn update_visibility(&mut self) {
        let players: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| o.is_player())
            .map(|o| o.id)
            .collect();
        for pid in players {
            let (map, loc, conn) = {
                let p = &self.objects[&pid];
                (p.map, p.location, p.player().unwrap().conn)
            };
            let now_visible: HashSet<ObjectId> = self.maps[&map]
                .objects
                .iter()
                .copied()
                .filter(|id| *id != pid)
                .filter(|id| self.objects[id].location.distance(loc) <= MAX_VIEW_RANGE)
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
