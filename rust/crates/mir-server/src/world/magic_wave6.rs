//! Skills wave six: the last player magics from
//! `docs/research/skills-remaining.md` - Elemental Hurricane (channel),
//! Mirror Image, Frost Bite, Tornado, Cursed Doll, Soul Resonance, Corpse
//! Exploder, Summon Dead, Dragon Blood and Chain, plus the passives and
//! augments Shuriken, Burning, Shocked, Augment Poison Dust, Infection,
//! Magic Combustion and Chain Of Fire that ride on other paths.

use super::monster_ai::has_poison;
use super::*;
use mir_proto::magic_type as m;

/// Zircon `Globals.ShurikenLibraryWeaponShape`.
pub const SHURIKEN_SHAPE: u16 = 33;
/// Channelled beams stop after this long without a re-cast.
const CHANNEL_LIMIT: u64 = 10_000;

/// A channelled spell in progress (Elemental Hurricane).
#[derive(Debug, Clone, Copy)]
pub struct Channel {
    pub magic: u16,
    pub direction: Direction,
    pub next_tick: u64,
    pub until: u64,
    pub cost: i32,
}

pub(super) fn handled(magic: u16) -> bool {
    matches!(
        magic,
        m::ELEMENTAL_HURRICANE
            | m::MIRROR_IMAGE
            | m::FROST_BITE
            | m::TORNADO
            | m::CURSED_DOLL
            | m::SOUL_RESONANCE
            | m::CORPSE_EXPLODER
            | m::SUMMON_DEAD
            | m::DRAGON_BLOOD
            | m::CHAIN
    )
}

impl World {
    /// Does the player know a magic (any level)?
    pub(super) fn knows(&self, id: ObjectId, magic: u16) -> bool {
        self.objects
            .get(&id)
            .and_then(|o| o.player())
            .is_some_and(|p| p.magics.iter().any(|x| x.magic == magic))
    }

    /// Consume one Dark Stone from the bag (Mirror Image's focus).
    fn use_dark_stone(&mut self, id: ObjectId) -> bool {
        let slot = {
            let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
                return false;
            };
            p.bag.inventory.iter().position(|it| {
                it.as_ref().is_some_and(|it| {
                    self.data
                        .items
                        .get(&it.info)
                        .is_some_and(|d| d.item_type == item_type::DARK_STONE)
                })
            })
        };
        let Some(slot) = slot else {
            return false;
        };
        let change = self
            .objects
            .get_mut(&id)
            .and_then(|o| o.player_mut())
            .and_then(|p| p.bag.take(Grid::Inventory, slot as u8, 1));
        self.send_changes(id, change.into_iter().collect());
        true
    }

    fn summon_spot(&mut self, map: i32, near: Point) -> Option<Point> {
        for _ in 0..25 {
            let p = Point::new(
                near.x + self.rng.random_range(-1..=1),
                near.y + self.rng.random_range(-1..=1),
            );
            if self.maps[&map].file.is_walkable(p.x, p.y) && !self.cell_blocked(map, p, false) {
                return Some(p);
            }
        }
        None
    }

    fn monster_with_flag(&self, flag: i32) -> Option<i32> {
        self.data
            .monsters
            .values()
            .find(|d| d.flag == flag)
            .map(|d| d.index)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn wave6_cast(
        &mut self,
        id: ObjectId,
        magic: u16,
        target: Option<ObjectId>,
        location: Point,
        direction: Direction,
        loc: Point,
        now: u64,
        pending: &mut Vec<PendingMagic>,
        targets: &mut Vec<ObjectId>,
        locations: &mut Vec<Point>,
    ) -> bool {
        let level = self.magic_level(id, magic);
        let mk = |time: u64, target: Option<ObjectId>, cell: Point| PendingMagic {
            time,
            caster: id,
            magic,
            target,
            location: cell,
            direction: Some(direction),
            primary: true,
            chain: None,
        };
        let dist_ms = |w: &World, t: ObjectId| w.objects[&t].location.distance(loc) as u64 * 48;
        let map = self.objects[&id].map;
        match magic {
            m::ELEMENTAL_HURRICANE => {
                // Start costs Mana * Cost / 1000 on top; each tick costs Cost.
                let (cost, max_mp) = {
                    let p = self.objects[&id].player().unwrap();
                    let def = &self.data.magics[&magic];
                    let um = p.magics.iter().find(|x| x.magic == magic).unwrap();
                    (um.cost(def), p.max_mp)
                };
                let p = self.objects.get_mut(&id).unwrap().player_mut().unwrap();
                p.mp = (p.mp - max_mp * cost / 1000).max(0);
                p.channel = Some(Channel {
                    magic,
                    direction,
                    next_tick: now + 500,
                    until: now + CHANNEL_LIMIT,
                    cost,
                });
                locations.push(loc.step(direction, 1));
            }
            m::MIRROR_IMAGE => {
                if location.distance(loc) > MAGIC_RANGE || !self.use_dark_stone(id) {
                    return false;
                }
                locations.push(location);
                pending.push(mk(now + 500, None, location));
            }
            m::FROST_BITE | m::DRAGON_BLOOD => pending.push(mk(now + 500, None, loc)),
            m::TORNADO => {
                if location.distance(loc) > MAGIC_RANGE {
                    return false;
                }
                locations.push(location);
                pending.push(mk(now + 500, None, location));
            }
            m::CURSED_DOLL => {
                let clevel = self.level_of(&self.objects[&id]);
                let Some(t) = target.filter(|t| {
                    let o = &self.objects[t];
                    o.is_monster() && !o.dead && self.level_of(o) <= clevel + 2
                }) else {
                    locations.push(location);
                    return false;
                };
                if self.use_amulet(id, 1).is_none() {
                    return false;
                }
                targets.push(t);
                pending.push(mk(now + 500 + dist_ms(self, t), Some(t), location));
            }
            m::SOUL_RESONANCE => {
                let members = self.group_members(id);
                let Some(t) = target.filter(|t| *t != id && members.contains(t)) else {
                    self.send_to(
                        id,
                        ServerMessage::Chat {
                            text: "Soul Resonance links you with a group member.".into(),
                        },
                    );
                    return false;
                };
                targets.push(t);
                pending.push(mk(now + 500, Some(t), location));
            }
            m::CORPSE_EXPLODER | m::SUMMON_DEAD => {
                let Some(t) =
                    target.filter(|t| self.objects[t].is_monster() && self.objects[t].dead)
                else {
                    self.send_to(
                        id,
                        ServerMessage::Chat {
                            text: "That needs a corpse.".into(),
                        },
                    );
                    return false;
                };
                let amulets = if magic == m::SUMMON_DEAD { 10 } else { 2 };
                if self.use_amulet(id, amulets).is_none() {
                    self.send_to(
                        id,
                        ServerMessage::Chat {
                            text: format!("You need {amulets} amulets."),
                        },
                    );
                    return false;
                }
                targets.push(t);
                let delay = if magic == m::SUMMON_DEAD {
                    800
                } else {
                    1000 + dist_ms(self, t)
                };
                pending.push(mk(now + delay, Some(t), self.objects[&t].location));
            }
            m::CHAIN => {
                let Some(t) =
                    target.filter(|t| self.objects[t].is_monster() && !self.objects[t].dead)
                else {
                    locations.push(location);
                    return false;
                };
                targets.push(t);
                pending.push(mk(now + 500 + dist_ms(self, t), Some(t), location));
            }
            _ => return false,
        }
        let _ = (level, map);
        true
    }

    pub(super) fn wave6_land(&mut self, pm: &PendingMagic, cmap: i32, cloc: Point) {
        let caster = pm.caster;
        let level = self.magic_level(caster, pm.magic);
        let (pmin, pmax, _) = self.caster_power(caster, pm.magic);
        let cs = self.objects[&caster].stats;
        let power = self.roll_range(pmin, pmax);
        let sc = self.roll_range(cs.min_sc, cs.max_sc);
        let now = self.now;
        match pm.magic {
            m::MIRROR_IMAGE => {
                let Some(def) = self.monster_with_flag(7) else {
                    return;
                };
                let spot = self.summon_spot(cmap, pm.location).unwrap_or(cloc);
                let decoy = self.create_monster(def, cmap, spot, None, Some(caster), 0);
                if let Some(mm) = self.objects.get_mut(&decoy).and_then(|o| o.monster_mut()) {
                    mm.despawn_at = Some(now + (level.max(1) * 5) as u64 * 1000);
                }
                self.level_magic(caster, pm.magic);
            }
            m::FROST_BITE => {
                let secs = 3 + 3 * level;
                self.buff_add(
                    caster,
                    buff_type::FROST_BITE,
                    secs as u64 * 1000,
                    BuffStats {
                        frost: cs.max_mc + power,
                        ..BuffStats::default()
                    },
                );
                self.level_magic(caster, pm.magic);
            }
            m::TORNADO => {
                let Some(def) = self.monster_with_flag(8) else {
                    return;
                };
                let spot = self.summon_spot(cmap, pm.location).unwrap_or(cloc);
                let tornado = self.create_monster(def, cmap, spot, None, Some(caster), level * 2);
                let (hp, ac, mr, mc) = {
                    let c = &self.objects[&caster];
                    let l = level.max(1);
                    (
                        (c.max_hp / 10 * l).max(1),
                        c.stats.max_ac / 10 * l,
                        c.stats.max_mr / 10 * l,
                        c.stats.max_mc / 10 * l,
                    )
                };
                if let Some(o) = self.objects.get_mut(&tornado) {
                    o.max_hp = hp;
                    o.hp = hp;
                    o.stats.max_ac = o.stats.max_ac.max(ac);
                    o.stats.max_mr = o.stats.max_mr.max(mr);
                    o.stats.max_dc = o.stats.max_dc.max(mc);
                    if let Some(mm) = o.monster_mut() {
                        mm.despawn_at = Some(now + 10_000);
                    }
                }
                self.level_magic(caster, pm.magic);
            }
            m::CURSED_DOLL => {
                let Some(t) = pm.target.filter(|t| !self.objects[t].dead) else {
                    return;
                };
                let Some(def) = self.monster_with_flag(5) else {
                    return;
                };
                let spot = self.summon_spot(cmap, cloc).unwrap_or(cloc);
                let doll = self.create_monster(def, cmap, spot, None, Some(caster), 0);
                if let Some(mm) = self.objects.get_mut(&doll).and_then(|o| o.monster_mut()) {
                    mm.despawn_at = Some(now + (10 + 5 * level) as u64 * 1000);
                    mm.link = Some(t);
                }
                self.level_magic(caster, pm.magic);
            }
            m::SOUL_RESONANCE => {
                let Some(t) = pm.target.filter(|t| self.objects.contains_key(t)) else {
                    return;
                };
                for (a, b) in [(caster, t), (t, caster)] {
                    if let Some(p) = self.objects.get_mut(&a).and_then(|o| o.player_mut()) {
                        p.soul_link = Some(b);
                    }
                    self.buff_add(
                        a,
                        buff_type::SOUL_RESONANCE,
                        60_000,
                        BuffStats {
                            hp_pct: power,
                            ..BuffStats::default()
                        },
                    );
                }
                self.level_magic(caster, pm.magic);
            }
            m::CORPSE_EXPLODER => {
                let Some(t) = pm
                    .target
                    .filter(|t| self.objects.get(t).is_some_and(|o| o.dead))
                else {
                    return;
                };
                let at = self.objects[&t].location;
                let victims: Vec<ObjectId> = self
                    .on_map(cmap)
                    .filter(|o| {
                        !o.dead
                            && o.location.distance(at) <= 1
                            && self.objects[&caster].hostile_to(o)
                    })
                    .map(|o| o.id)
                    .collect();
                self.remove_object(t);
                for v in victims {
                    let damage = power + sc;
                    let ts = self.objects[&v].stats;
                    let dealt = damage - self.roll_range(ts.min_mr, ts.max_mr);
                    if dealt > 0 {
                        self.damage(v, caster, dealt, element::DARK, true);
                    }
                }
                self.level_magic(caster, pm.magic);
            }
            m::SUMMON_DEAD => {
                let Some(t) = pm
                    .target
                    .filter(|t| self.objects.get(t).is_some_and(|o| o.dead))
                else {
                    return;
                };
                let at = self.objects[&t].location;
                self.remove_object(t);
                // Success: Random(MaxLevel + 1) <= Level (MaxLevel 3 here).
                if self.rng.random_range(0..4) > level {
                    self.send_to(
                        caster,
                        ServerMessage::Chat {
                            text: "The corpse does not rise.".into(),
                        },
                    );
                    return;
                }
                let Some(def) = self
                    .data
                    .monsters
                    .values()
                    .find(|d| d.name.to_ascii_lowercase().replace(' ', "") == "undeadsoul")
                    .map(|d| d.index)
                else {
                    return;
                };
                let pets = self.objects[&caster]
                    .player()
                    .map(|p| p.pets.len())
                    .unwrap_or(0);
                if pets >= 2 {
                    self.send_to(
                        caster,
                        ServerMessage::Chat {
                            text: "You cannot control more summons.".into(),
                        },
                    );
                    return;
                }
                let spot = self.summon_spot(cmap, at).unwrap_or(at);
                self.create_monster(def, cmap, spot, None, Some(caster), level * 2);
                self.level_magic(caster, pm.magic);
            }
            m::DRAGON_BLOOD => {
                if let Some(p) = self.objects.get_mut(&caster).and_then(|o| o.player_mut()) {
                    p.dragon_blood = true;
                }
                self.send_to(
                    caster,
                    ServerMessage::Chat {
                        text: "Your blades drip with poison.".into(),
                    },
                );
                self.level_magic(caster, pm.magic);
            }
            m::CHAIN => {
                let Some(t) = pm.target.filter(|t| !self.objects[t].dead) else {
                    return;
                };
                let at = self.objects[&t].location;
                let max = (5 + 2 * level) as usize;
                let followers: Vec<ObjectId> = self
                    .on_map(cmap)
                    .filter(|o| {
                        o.id != t
                            && o.is_monster()
                            && !o.dead
                            && o.location.distance(at) <= 2
                            && self.objects[&caster].hostile_to(o)
                    })
                    .map(|o| o.id)
                    .take(max)
                    .collect();
                let until = now + power.max(1) as u64 * 1000;
                for f in followers {
                    if let Some(mm) = self.objects.get_mut(&f).and_then(|o| o.monster_mut()) {
                        mm.chained = Some((t, until));
                    }
                }
                // Chain Of Fire: the tether explodes around the leader.
                if self.knows(caster, m::CHAIN_OF_FIRE) {
                    let (amin, amax, _) = self.caster_power(caster, m::CHAIN_OF_FIRE);
                    let aug = self.roll_range(amin, amax);
                    let mc = self.roll_range(cs.min_mc, cs.max_mc);
                    let victims: Vec<ObjectId> = self
                        .on_map(cmap)
                        .filter(|o| {
                            !o.dead
                                && o.location.distance(at) <= 2
                                && self.objects[&caster].hostile_to(o)
                        })
                        .map(|o| o.id)
                        .collect();
                    for v in victims {
                        let ts = self.objects[&v].stats;
                        let dealt = mc * aug / 100 - self.roll_range(ts.min_mr, ts.max_mr);
                        if dealt > 0 {
                            self.damage(v, caster, dealt, element::FIRE, true);
                        }
                    }
                }
                self.level_magic(caster, pm.magic);
            }
            _ => {}
        }
    }

    // ---- hooks ---------------------------------------------------------

    /// Stop a channelled spell (move, turn, struck, or out of mana).
    pub(super) fn channel_cancel(&mut self, id: ObjectId) {
        if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
            p.channel = None;
        }
    }

    /// Elemental Hurricane ticks: an 8-cell beam with weaker flanks every
    /// 500 ms, each tick costing the spell's mana.
    pub(super) fn process_channels(&mut self) {
        let now = self.now;
        let ids: Vec<ObjectId> = self
            .players()
            .filter(|o| o.player().is_some_and(|p| p.channel.is_some()))
            .map(|o| o.id)
            .collect();
        for id in ids {
            let (ch, map, loc, dead) = {
                let o = &self.objects[&id];
                (
                    o.player().unwrap().channel.unwrap(),
                    o.map,
                    o.location,
                    o.dead,
                )
            };
            if dead || now >= ch.until {
                self.channel_cancel(id);
                continue;
            }
            if now < ch.next_tick {
                continue;
            }
            {
                let p = self.objects.get_mut(&id).unwrap().player_mut().unwrap();
                if p.mp < ch.cost {
                    p.channel = None;
                    continue;
                }
                p.mp -= ch.cost;
                if let Some(c) = p.channel.as_mut() {
                    c.next_tick = now + 500;
                }
            }
            self.send_player_stats(id);
            let (l, r) = if ch.direction.index().is_multiple_of(2) {
                (2i8, -2i8)
            } else {
                (1, -1)
            };
            let mut cells: Vec<(Point, i32)> = Vec::new();
            for i in 1..=8 {
                let c = loc.step(ch.direction, i);
                if !self.maps[&map].file.is_walkable(c.x, c.y) {
                    break;
                }
                cells.push((c, 100));
                cells.push((c.step(ch.direction.rotate(l), 1), 30));
                cells.push((c.step(ch.direction.rotate(r), 1), 30));
            }
            let mut hit: Vec<ObjectId> = Vec::new();
            for (cell, scale) in cells {
                let victims: Vec<ObjectId> = self.maps[&map]
                    .objects_at(cell)
                    .iter()
                    .copied()
                    .filter(|v| !hit.contains(v) && self.objects[&id].hostile_to(&self.objects[v]))
                    .collect();
                for v in victims {
                    hit.push(v);
                    self.magic_attack(id, v, ch.magic, element::FIRE, scale);
                }
            }
            if !hit.is_empty() {
                self.level_magic(id, ch.magic);
            }
        }
    }

    /// Frost Bite banks the damage its bearer takes.
    pub(super) fn frost_store(&mut self, target: ObjectId, amount: i32) {
        if let Some(b) = self
            .objects
            .get_mut(&target)
            .and_then(|o| o.player_mut())
            .and_then(|p| p.buffs.iter_mut().find(|b| b.kind == buff_type::FROST_BITE))
        {
            b.stats.frost += amount;
        }
    }

    /// When Frost Bite ends the banked damage bursts on monsters within 3.
    pub(super) fn frost_bite_burst(&mut self, id: ObjectId, stored: i32) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let (map, loc, cap) = (o.map, o.location, o.stats.max_mc * 50);
        let amount = stored.min(cap.max(1));
        if amount <= 0 {
            return;
        }
        let victims: Vec<ObjectId> = self
            .on_map(map)
            .filter(|v| v.is_monster() && !v.dead && v.location.distance(loc) <= 3)
            .map(|v| v.id)
            .collect();
        for v in victims {
            self.damage(v, id, amount, element::ICE, true);
        }
    }

    /// Melee hits by players: Dragon Blood poisons, Magic Combustion drains
    /// a player's mana.
    pub(super) fn melee_landed(&mut self, attacker: ObjectId, target: ObjectId, dealt: i32) {
        let _ = dealt;
        let armed = self.objects[&attacker]
            .player()
            .is_some_and(|p| p.dragon_blood);
        if armed {
            let (pmin, pmax, _) = self.caster_power(attacker, m::DRAGON_BLOOD);
            let power = self.roll_range(pmin, pmax);
            let cs = self.objects[&attacker].stats;
            let sp = self.roll_range(cs.min_mc.min(cs.min_sc), cs.max_mc.min(cs.max_sc));
            let value = (sp * power / 100).max(1);
            let existing = self.objects[&target]
                .poisons
                .iter()
                .find(|p| p.kind == poison_kind::GREEN)
                .map(|p| p.value)
                .unwrap_or(0);
            self.apply_poison(
                target,
                Poison {
                    kind: poison_kind::GREEN,
                    value: (existing + value).min(value * 4),
                    ticks_left: 10,
                    next_tick: self.now + 2000,
                    owner: Some(attacker),
                },
            );
            // Zircon re-arms on Random(5) == 0 with a green poison item;
            // here the blade simply stays wet four times in five.
            if self.rng.random_range(0..5) == 0 {
                if let Some(p) = self.objects.get_mut(&attacker).and_then(|o| o.player_mut()) {
                    p.dragon_blood = false;
                }
            }
        }
        if self.objects[&target].is_player() && self.knows(attacker, m::MAGIC_COMBUSTION) {
            let (pmin, pmax, _) = self.caster_power(attacker, m::MAGIC_COMBUSTION);
            let power = self.roll_range(pmin, pmax);
            let cs = self.objects[&attacker].stats;
            let dc = self.roll_dc(cs);
            let drain = dc * power / 100;
            if drain > 0 {
                if let Some(p) = self.objects.get_mut(&target).and_then(|o| o.player_mut()) {
                    p.mp = (p.mp - drain).max(0);
                }
                self.send_player_stats(target);
            }
        }
    }

    /// Shuriken: with a shuriken weapon the swing becomes a thrown hit on
    /// the first enemy along the facing line (up to 8 cells).
    pub(super) fn shuriken_attack(
        &mut self,
        id: ObjectId,
        direction: Direction,
        map: i32,
        loc: Point,
        stats: CombatStats,
    ) -> bool {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return false;
        };
        if !p.magics.iter().any(|x| x.magic == m::SHURIKEN) {
            return false;
        }
        let shape = p.bag.equipped_shape(&self.data, slot::WEAPON).unwrap_or(-1);
        if shape != SHURIKEN_SHAPE as i32 {
            return false;
        }
        let mut target = None;
        for i in 1..=8 {
            let cell = loc.step(direction, i);
            if let Some(t) = self.maps[&map]
                .objects_at(cell)
                .iter()
                .copied()
                .find(|t| self.objects[&id].hostile_to(&self.objects[t]))
            {
                target = Some((t, i as u64));
                break;
            }
            if !self.maps[&map].file.is_walkable(cell.x, cell.y) {
                break;
            }
        }
        let Some((t, dist)) = target else {
            return false;
        };
        let power = self.roll_dc(stats);
        let tloc = self.objects[&t].location;
        self.events.push((
            id,
            ServerMessage::ObjectRangeAttack {
                id,
                direction,
                target: Some(t),
                location: tloc,
                magic: m::SHURIKEN,
            },
        ));
        self.pending_hits.push(PendingHit {
            time: self.now + (dist * 50).clamp(100, 750),
            attacker: id,
            target_cell: (map, tloc),
            power,
            target: Some(t),
            magics: vec![m::SHURIKEN],
            primary: true,
            raw: false,
            element: element::NONE,
            ranged: true,
        });
        true
    }

    /// Burning / Shocked augments: a fire or lightning spell hit sometimes
    /// carries the augment's power on top.
    pub(super) fn burn_shock_bonus(&mut self, attacker: ObjectId, elem: u8) -> i32 {
        let aug = match elem {
            element::FIRE => m::BURNING,
            element::LIGHTNING => m::SHOCKED,
            _ => return 0,
        };
        if !self.knows(attacker, aug) {
            return 0;
        }
        let level = self.magic_level(attacker, aug);
        if self.rng.random_range(0..4) > level {
            return 0;
        }
        let (pmin, pmax, _) = self.caster_power(attacker, aug);
        self.roll_range(pmin, pmax)
    }

    /// Soul Resonance: when one linked player dies, so does the other.
    pub(super) fn soul_resonance_death(&mut self, id: ObjectId) {
        let Some(partner) = self
            .objects
            .get_mut(&id)
            .and_then(|o| o.player_mut())
            .and_then(|p| p.soul_link.take())
        else {
            return;
        };
        let linked_back = self
            .objects
            .get(&partner)
            .and_then(|o| o.player())
            .is_some_and(|p| p.soul_link == Some(id));
        if !linked_back || self.objects[&partner].dead {
            return;
        }
        if let Some(p) = self.objects.get_mut(&partner).and_then(|o| o.player_mut()) {
            p.soul_link = None;
        }
        self.buff_remove(id, buff_type::SOUL_RESONANCE);
        self.buff_remove(partner, buff_type::SOUL_RESONANCE);
        self.send_to(
            partner,
            ServerMessage::Chat {
                text: "Your resonating soul follows your partner into death.".into(),
            },
        );
        self.objects.get_mut(&partner).unwrap().hp = 0;
        self.player_die(partner);
    }

    /// A cursed doll forwards what it suffers to its victim.
    pub(super) fn doll_link(&self, id: ObjectId) -> Option<ObjectId> {
        let o = self.objects.get(&id)?;
        let mm = match &o.kind {
            Kind::Monster(mm) => mm,
            _ => return None,
        };
        let victim = mm.link?;
        let v = self.objects.get(&victim)?;
        (!v.dead && !matches!(&v.kind, Kind::Monster(x) if x.link.is_some())).then_some(victim)
    }

    /// Timed summons vanish; they leave their owner's pet list too.
    pub(super) fn despawn_summon(&mut self, id: ObjectId) {
        if let Some(owner) = self.objects.get(&id).and_then(|o| o.monster_ref().owner) {
            if let Some(p) = self.objects.get_mut(&owner).and_then(|o| o.player_mut()) {
                p.pets.retain(|x| *x != id);
            }
        }
        self.remove_object(id);
    }

    /// Infection: a Parasite tick spreads to one monster next to the victim.
    pub(super) fn infection_spread(&mut self, victim: ObjectId, owner: ObjectId, value: i32) {
        if !self.knows(owner, m::INFECTION) {
            return;
        }
        let level = self.magic_level(owner, m::INFECTION);
        let (pmin, pmax, _) = self.caster_power(owner, m::INFECTION);
        let ticks = self.roll_range(pmin, pmax).max(1);
        let Some(vo) = self.objects.get(&victim) else {
            return;
        };
        let (map, loc) = (vo.map, vo.location);
        let next = self
            .on_map(map)
            .find(|o| {
                o.id != victim
                    && o.is_monster()
                    && !o.dead
                    && o.location.distance(loc) <= 1
                    && !has_poison(o, poison_kind::PARASITE)
            })
            .map(|o| o.id);
        if let Some(n) = next {
            self.apply_poison(
                n,
                Poison {
                    kind: poison_kind::PARASITE,
                    value: (value * level + 1) / 10,
                    ticks_left: ticks,
                    next_tick: self.now + 2000,
                    owner: Some(owner),
                },
            );
        }
    }
}
