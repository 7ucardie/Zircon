use super::ai_profile::{AiProfile, Hidden, Spell};
use super::*;

/// Zircon `Globals.ProjectileSpeed`: ms per cell of travel.
const PROJECTILE_MS: u64 = 48;

impl World {
    // ---- monster AI --------------------------------------------------------

    /// Cached AI profile for a `MonsterInfo.AI` value.
    pub(super) fn ai_profile(&mut self, ai: i32) -> AiProfile {
        self.ai_profiles
            .entry(ai)
            .or_insert_with(|| super::ai_profile::profile(ai))
            .clone()
    }

    fn monster_ai(&self, id: ObjectId) -> i32 {
        self.data.monsters[&self.objects[&id].monster_ref().def].ai
    }

    pub(super) fn process_monster(&mut self, id: ObjectId) {
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
        // Paralysis and Silenced stop a monster completely.
        if paralysed(&self.objects[&id]) || has_poison(&self.objects[&id], poison_kind::SILENCED) {
            return;
        }
        // Puppets stand still and blow up after 5 s, or once something
        // targets or strikes them.
        if let Some(explode_at) = self.objects[&id].monster_ref().explode_at {
            let targeted = self.objects[&id].monster_ref().target.is_some();
            if now >= explode_at || targeted {
                self.puppet_explode(id);
            }
            return;
        }
        // Pets: follow the owner, come back when out of sight, untame on expiry.
        let owner = self.objects[&id].monster_ref().owner;
        if let Some(owner_id) = owner {
            let owner_state = self
                .objects
                .get(&owner_id)
                .filter(|o| o.is_player())
                .map(|o| {
                    (
                        o.dead,
                        o.map,
                        o.location,
                        o.direction,
                        o.visible.contains(&id),
                    )
                });
            let Some((odead, omap, oloc, odir, seen)) = owner_state else {
                self.remove_object(id);
                return;
            };
            if odead {
                self.monster_die(id, owner_id);
                return;
            }
            if now >= self.objects[&id].monster_ref().tame_until {
                self.untame(id);
            } else if !seen || omap != map {
                let behind = oloc.step(odir.rotate(4), 1);
                let to = if omap == map
                    && self.maps[&omap].file.is_walkable(behind.x, behind.y)
                    && !self.cell_blocked(omap, behind, false)
                {
                    behind
                } else {
                    oloc
                };
                if omap != map {
                    self.change_map(id, omap, to);
                } else {
                    self.teleport_object(id, to);
                }
                if let Some(m) = self.objects.get_mut(&id).and_then(|o| o.monster_mut()) {
                    m.target = None;
                }
                return;
            }
        }

        let ai = self.monster_ai(id);
        let prof = self.ai_profile(ai);

        // Drop invalid targets (Zircon `Process`): dead, elsewhere, too far,
        // or no longer attackable. Abyss shrinks the view range to 2.
        let view_range = self.monster_view_range(id);
        {
            let target = self.objects[&id].monster_ref().target;
            if let Some(t) = target {
                let me = &self.objects[&id];
                let valid = self
                    .objects
                    .get(&t)
                    .map(|to| {
                        !to.dead
                            && to.map == map
                            && to.location.distance(loc) <= MAX_VIEW_RANGE
                            && (to.location.distance(loc) <= view_range
                                || !has_poison(me, poison_kind::ABYSS))
                            && me.hostile_to(to)
                            && self.monster_may_target(me, to)
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
        }

        // Hidden monsters (burrowers, ambushers, statues) only watch for prey.
        if let Some(h) = prof.hidden {
            if self.process_hidden(id, &h, &prof) {
                return;
            }
        }

        // Regen: 2 % every 10 s, never while bleeding.
        if !has_poison(&self.objects[&id], poison_kind::HEMORRHAGE) {
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

        // Larva: nothing to chase means nothing to live for.
        if prof.dies_without_target && self.objects[&id].monster_ref().target.is_none() {
            self.monster_die(id, id);
            return;
        }

        // Search (Zircon `ProcessSearch`): every 3 s when idle; with a target
        // Zircon re-searches every tick and switches to the closest, which we
        // sample every 500 ms. Passive monsters only retaliate.
        {
            let (search_due, has_target) = {
                let m = self.objects[&id].monster_ref();
                (now >= m.search_time, m.target.is_some())
            };
            let is_pet = owner.is_some();
            if search_due && (!prof.passive || is_pet) {
                let delay = if has_target { 500 } else { SEARCH_DELAY };
                self.objects
                    .get_mut(&id)
                    .unwrap()
                    .monster_mut()
                    .unwrap()
                    .search_time = now + delay;
                if let Some(pick) = self.monster_search(id, view_range, prof.guard) {
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

        // Boss summon phases: at every 1/n of max HP lost, raise minions.
        if let Some(stages) = &prof.stages {
            self.process_stages(id, stages, target);
        }

        // Pets with nothing to fight walk to the cell behind their owner.
        if let Some(owner_id) = owner {
            if target.is_none() && self.monster_can_move(id) {
                let (oloc, odir) = {
                    let o = &self.objects[&owner_id];
                    (o.location, o.direction)
                };
                let goal = oloc.step(odir.rotate(4), 1);
                if goal != loc {
                    self.move_toward(id, goal, false);
                }
            }
            if let Some(t) = target {
                self.process_target(id, t, &prof);
            }
            return;
        }

        // Roam (Zircon `ProcessRoam`): step off a shared cell; otherwise every
        // 2 s a 10 % chance to walk (2/3) or turn (1/3) while someone watches.
        if !prof.immobile && self.monster_can_move(id) {
            let roam_due = now >= self.objects[&id].monster_ref().roam_time;
            if roam_due {
                self.objects
                    .get_mut(&id)
                    .unwrap()
                    .monster_mut()
                    .unwrap()
                    .roam_time = now + ROAM_DELAY;
                let seen = self.players().any(|p| p.visible.contains(&id));
                let shared = self.maps[&map]
                    .objects_at(loc)
                    .iter()
                    .any(|o| *o != id && self.objects[o].blocking());
                if seen && shared {
                    let dir = Direction::from_index(self.rng.random_range(0..8));
                    for i in 0..8 {
                        if self.monster_walk(id, dir.rotate(i)) {
                            break;
                        }
                    }
                } else if seen && target.is_none() && self.rng.random_range(0..10) == 0 {
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

        // Target (Zircon `ProcessTarget`).
        let Some(t) = target else {
            return;
        };
        self.process_target(id, t, &prof);
    }

    /// Zircon `ViewRange`: 2 under Abyss, else the definition's.
    fn monster_view_range(&self, id: ObjectId) -> i32 {
        let o = &self.objects[&id];
        if has_poison(o, poison_kind::ABYSS) {
            2
        } else {
            self.data.monsters[&o.monster_ref().def].view_range
        }
    }

    /// Nearest attackable object within `view_range` (ties broken at random).
    /// Guards look for wild monsters instead of players.
    fn monster_search(&mut self, id: ObjectId, view_range: i32, guard: bool) -> Option<ObjectId> {
        let (map, loc) = {
            let o = &self.objects[&id];
            (o.map, o.location)
        };
        let mut best: Vec<ObjectId> = Vec::new();
        let mut best_d = i32::MAX;
        let me = &self.objects[&id];
        for pid in &self.maps[&map].objects {
            let p = &self.objects[pid];
            if *pid == id || p.dead {
                continue;
            }
            let ok = if guard {
                matches!(&p.kind, Kind::Monster(m) if m.owner.is_none() && !m.guard)
                    && !self.data.monsters[&p.monster_ref().def].is_passive()
                    && !self.ai_profiles_passive(p.monster_ref().def)
            } else {
                me.hostile_to(p) && self.can_see(me, p) && self.monster_may_target(me, p)
            };
            if !ok {
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
        if best.is_empty() {
            None
        } else {
            Some(best[self.rng.random_range(0..best.len())])
        }
    }

    /// Whether a definition's AI profile is passive (without mutating the cache).
    fn ai_profiles_passive(&self, def: i32) -> bool {
        let ai = self.data.monsters[&def].ai;
        self.ai_profiles
            .get(&ai)
            .map(|p| p.passive)
            .unwrap_or_else(|| super::ai_profile::profile(ai).passive)
    }

    /// Hidden monsters: reveal when a target comes within `find_range`,
    /// hide again (healing) when it leaves `hide_range`. Returns true while
    /// the monster stays hidden this tick.
    fn process_hidden(&mut self, id: ObjectId, h: &Hidden, prof: &AiProfile) -> bool {
        let now = self.now;
        let (hidden, check_due, loc, map) = {
            let o = &self.objects[&id];
            let m = o.monster_ref();
            (m.hidden, now >= m.hide_check, o.location, o.map)
        };
        if !check_due {
            return hidden;
        }
        self.objects
            .get_mut(&id)
            .unwrap()
            .monster_mut()
            .unwrap()
            .hide_check = now + 3000;
        // Anyone attackable close enough?
        let range = if hidden { h.find_range } else { h.hide_range };
        let near = {
            let me = &self.objects[&id];
            self.maps[&map]
                .objects
                .iter()
                .map(|o| &self.objects[o])
                .filter(|p| me.hostile_to(p) && self.monster_may_target(me, p))
                .filter(|p| p.location.distance(loc) <= range.max(prof.attack_range))
                .map(|p| p.id)
                .min_by_key(|p| self.objects[p].location.distance(loc))
        };
        if hidden {
            if let Some(t) = near {
                self.reveal_monster(id, Some(t));
                if h.wake_range > 0 {
                    // Statues wake their neighbours of the same kind.
                    let def = self.objects[&id].monster_ref().def;
                    let others: Vec<ObjectId> = self
                        .on_map(map)
                        .filter(|o| {
                            o.id != id
                                && matches!(&o.kind, Kind::Monster(m) if m.def == def && m.hidden)
                                && o.location.distance(loc) <= h.wake_range
                        })
                        .map(|o| o.id)
                        .collect();
                    for o in others {
                        self.reveal_monster(o, Some(t));
                    }
                }
                return false;
            }
            return true;
        }
        if h.hide_range > 0 && near.is_none() {
            // Submerge; burrowers heal fully and shake off poisons.
            let o = self.objects.get_mut(&id).unwrap();
            if h.heal_on_hide {
                o.hp = o.max_hp;
                o.poisons.clear();
            }
            o.action_time = now + 1000;
            if let Kind::Monster(m) = &mut o.kind {
                m.hidden = true;
                m.target = None;
            }
            let (hp, max_hp) = (o.hp, o.max_hp);
            self.events
                .push((id, ServerMessage::HealthChanged { id, hp, max_hp }));
            return true;
        }
        false
    }

    fn reveal_monster(&mut self, id: ObjectId, target: Option<ObjectId>) {
        let o = self.objects.get_mut(&id).unwrap();
        o.action_time = self.now + 1000;
        o.cell_time = self.now + 500;
        if let Kind::Monster(m) = &mut o.kind {
            m.hidden = false;
            if target.is_some() {
                m.target = target;
            }
        }
    }

    /// Zircon `Stage` phases: each time HP crosses another 1/n, spawn
    /// `fixed + Random(random + 1)` minions from the weighted list.
    fn process_stages(
        &mut self,
        id: ObjectId,
        stages: &super::ai_profile::Stages,
        target: Option<ObjectId>,
    ) {
        let (hp, max_hp, stage) = {
            let o = &self.objects[&id];
            (o.hp, o.max_hp, o.monster_ref().stage)
        };
        let stage = if stage < 0 { stages.count } else { stage };
        if stage <= 0 || (hp as i64 * stages.count as i64 / max_hp.max(1) as i64) >= stage as i64 {
            if self.objects[&id].monster_ref().stage < 0 {
                self.objects
                    .get_mut(&id)
                    .unwrap()
                    .monster_mut()
                    .unwrap()
                    .stage = stage;
            }
            return;
        }
        self.objects
            .get_mut(&id)
            .unwrap()
            .monster_mut()
            .unwrap()
            .stage = stage - 1;
        let count = stages.fixed + self.rng.random_range(0..stages.random + 1);
        self.spawn_minions(id, count, &stages.list, stages.max_minions, target, 6);
    }

    /// Zircon `SpawnMinions`: up to `count` monsters picked by weight from
    /// `list` (MonsterFlag values), within `radius` of the master.
    pub(super) fn spawn_minions(
        &mut self,
        id: ObjectId,
        count: i32,
        list: &[(i32, i32)],
        max_minions: i32,
        target: Option<ObjectId>,
        radius: i32,
    ) {
        // Forget dead minions.
        let alive: Vec<ObjectId> = self.objects[&id]
            .monster_ref()
            .minions
            .iter()
            .copied()
            .filter(|m| self.objects.get(m).map(|o| !o.dead).unwrap_or(false))
            .collect();
        self.objects
            .get_mut(&id)
            .unwrap()
            .monster_mut()
            .unwrap()
            .minions = alive.clone();
        let room = (max_minions - alive.len() as i32).max(0);
        let count = count.min(room);
        let total: i32 = list.iter().map(|(_, w)| *w).sum();
        if count <= 0 || total <= 0 {
            return;
        }
        let (map, loc) = {
            let o = &self.objects[&id];
            (o.map, o.location)
        };
        for _ in 0..count {
            let mut roll = self.rng.random_range(0..total);
            let mut flag = list[0].0;
            for (f, w) in list {
                if roll < *w {
                    flag = *f;
                    break;
                }
                roll -= w;
            }
            let Some(def) = self
                .data
                .monsters
                .values()
                .find(|d| d.flag == flag)
                .map(|d| d.index)
            else {
                continue;
            };
            let mut spot = None;
            for _ in 0..25 {
                let p = Point::new(
                    loc.x + self.rng.random_range(-radius..=radius),
                    loc.y + self.rng.random_range(-radius..=radius),
                );
                if self.maps[&map].file.is_walkable(p.x, p.y) && !self.cell_blocked(map, p, false) {
                    spot = Some(p);
                    break;
                }
            }
            let Some(spot) = spot else { continue };
            let minion = self.create_monster(def, map, spot, None, None, 0);
            if let Some(m) = self.objects.get_mut(&minion).and_then(|o| o.monster_mut()) {
                m.master = Some(id);
                m.target = target;
            }
            self.objects
                .get_mut(&id)
                .unwrap()
                .monster_mut()
                .unwrap()
                .minions
                .push(minion);
        }
    }

    /// Zircon `ProcessTarget` with the class-specific attack shapes.
    fn process_target(&mut self, id: ObjectId, t: ObjectId, prof: &AiProfile) {
        let now = self.now;
        let (loc, map) = {
            let o = &self.objects[&id];
            (o.location, o.map)
        };
        let tloc = self.objects[&t].location;
        let dist = tloc.distance(loc);
        // Fear: run straight away and never swing.
        if has_poison(&self.objects[&id], poison_kind::FEAR) {
            if self.monster_can_move(id) && !prof.immobile {
                let away = Direction::from_points(tloc, loc);
                self.move_toward(id, loc.step(away, 2), true);
            }
            return;
        }
        // Blink packages.
        if let Some((far, cooldown)) = prof.blink_when_far {
            let blink_due = now > self.objects[&id].monster_ref().blink_time;
            if dist > far && blink_due && self.monster_can_move(id) {
                self.objects
                    .get_mut(&id)
                    .unwrap()
                    .monster_mut()
                    .unwrap()
                    .blink_time = now + cooldown;
                self.blink_beside(id, t);
                return;
            }
        }
        if prof.blink_chance > 0 && dist > 1 && !prof.immobile {
            let blink_due = now > self.objects[&id].monster_ref().blink_time;
            if blink_due {
                self.objects
                    .get_mut(&id)
                    .unwrap()
                    .monster_mut()
                    .unwrap()
                    .blink_time = now + 1000;
                if self.rng.random_range(0..prof.blink_chance) == 0 && self.blink_beside(id, t) {
                    self.objects
                        .get_mut(&id)
                        .unwrap()
                        .monster_mut()
                        .unwrap()
                        .bonus = prof.double_on_blink;
                    return;
                }
            }
        }
        let on_ray = {
            let dx = (tloc.x - loc.x).abs();
            let dy = (tloc.y - loc.y).abs();
            dx == 0 || dy == 0 || dx == dy
        };
        let ranged_allowed = prof.attack_range > 1
            && (prof.range_cooldown_ms == 0 || now > self.objects[&id].monster_ref().range_time);
        let reach = if ranged_allowed { prof.attack_range } else { 1 };
        let in_range = tloc != loc && dist <= reach && (!prof.rays_only || on_ray || dist <= 1);
        // Timed or chance-based spells.
        if let Some(spell) = &prof.spell {
            let cooled = now >= self.objects[&id].monster_ref().spell_time;
            let chance = if in_range {
                prof.spell_chance_near
            } else {
                prof.spell_chance_far
            };
            let wants = chance > 0
                && cooled
                && self.monster_can_attack(id)
                && dist <= MAGIC_RANGE
                && (chance == 1 || self.rng.random_range(0..chance) == 0);
            if wants {
                if prof.spell_cooldown_ms > 0 {
                    self.objects
                        .get_mut(&id)
                        .unwrap()
                        .monster_mut()
                        .unwrap()
                        .spell_time = now + prof.spell_cooldown_ms;
                }
                self.monster_cast(id, t, spell.clone());
                return;
            }
        }
        // Archers keep their distance.
        if prof.kite && in_range && dist < prof.attack_range - 1 && self.monster_can_move(id) {
            let away = Direction::from_points(tloc, loc);
            self.move_toward(id, loc.step(away, 2), true);
        } else if !in_range {
            if prof.immobile {
                return;
            }
            if self.monster_can_move(id) {
                if tloc == loc {
                    let dir = Direction::from_index(self.rng.random_range(0..8));
                    for i in 0..8 {
                        if self.monster_walk(id, dir.rotate(i)) {
                            break;
                        }
                    }
                } else {
                    self.move_toward(id, tloc, false);
                }
            }
            // Zircon attacks again in the same tick after moving into range.
            let nloc = self.objects[&id].location;
            let ndist = tloc.distance(nloc);
            if !(ndist <= reach && ndist > 0) {
                return;
            }
        }
        if !self.monster_can_attack(id) || now < self.objects[&id].monster_ref().fear_time {
            return;
        }
        let _ = map;
        self.monster_attack_shape(id, t, prof);
    }

    /// Straight line toward `goal`, then rotate outward through all eight
    /// directions (Zircon `MoveTo`); `away` picks the opposite rotation order.
    fn move_toward(&mut self, id: ObjectId, goal: Point, away: bool) {
        let loc = self.objects[&id].location;
        if goal == loc {
            return;
        }
        let dir = Direction::from_points(loc, goal);
        for i in 0..8 {
            let r: i8 = if i % 2 == 0 {
                (i / 2) as i8
            } else {
                -((i + 1) / 2) as i8
            };
            let d = dir.rotate(if away { -r } else { r });
            if self.monster_walk(id, d) {
                return;
            }
        }
    }

    /// Teleport next to the target (first free cell around it). Returns
    /// whether a cell was found.
    fn blink_beside(&mut self, id: ObjectId, t: ObjectId) -> bool {
        let (map, tloc) = {
            let o = &self.objects[&t];
            (o.map, o.location)
        };
        let base = Direction::from_points(tloc, self.objects[&id].location);
        for i in 0..8 {
            let cell = tloc.step(base.rotate(i), 1);
            if self.maps[&map].file.is_walkable(cell.x, cell.y)
                && !self.cell_blocked(map, cell, false)
            {
                self.teleport_object(id, cell);
                let face = Direction::from_points(cell, tloc);
                if let Some(o) = self.objects.get_mut(&id) {
                    o.direction = face;
                    o.action_time = self.now + 300;
                }
                self.events.push((
                    id,
                    ServerMessage::ObjectTurn {
                        id,
                        direction: face,
                    },
                ));
                return true;
            }
        }
        false
    }

    /// Zircon `TeleportNearby(min, max)`: a random walkable cell at that distance.
    pub(super) fn teleport_nearby(&mut self, id: ObjectId, min: i32, max: i32) {
        let (map, loc) = {
            let o = &self.objects[&id];
            (o.map, o.location)
        };
        for _ in 0..40 {
            let d = self.rng.random_range(min..=max);
            let dir = Direction::from_index(self.rng.random_range(0..8));
            let cell = loc.step(dir, d);
            if self.maps[&map].file.is_walkable(cell.x, cell.y)
                && !self.cell_blocked(map, cell, false)
            {
                self.teleport_object(id, cell);
                return;
            }
        }
    }

    /// Pick the attack shape for this swing: spawner, line, ranged, splash,
    /// self area or plain melee.
    fn monster_attack_shape(&mut self, id: ObjectId, t: ObjectId, prof: &AiProfile) {
        let now = self.now;
        let loc = self.objects[&id].location;
        let tloc = self.objects[&t].location;
        let dist = tloc.distance(loc);
        let dir = Direction::from_points(loc, tloc);
        if let Some((flag, count)) = prof.spawn_on_attack {
            self.face_and_swing(id, dir, None);
            self.spawn_minions(id, count, &[(flag, 1)], 20, Some(t), 1);
            return;
        }
        if prof.suicide_range > 0 {
            self.monster_die(id, id);
            return;
        }
        if prof.guard {
            self.face_and_swing(id, dir, None);
            // Guards end monsters outright (300 ms).
            let power = self.objects[&t].hp.max(1);
            self.pending_hits.push(PendingHit {
                time: now + 300,
                attacker: id,
                target_cell: (self.objects[&id].map, tloc),
                power,
                target: Some(t),
                magics: Vec::new(),
                primary: true,
                raw: true,
                element: element::NONE,
                ranged: true,
            });
            return;
        }
        if prof.line_attack > 0 && dist > 1 {
            self.monster_line_attack(id, dir, prof.line_attack);
            return;
        }
        if prof.self_aoe > 0
            && (prof.self_aoe_chance == 0 || self.rng.random_range(0..prof.self_aoe_chance) == 0)
        {
            self.face_and_swing(id, dir, None);
            let victims = self.hostiles_within(id, loc, prof.self_aoe);
            for v in victims {
                self.push_monster_hit(id, v, 400, false, element::NONE);
            }
            return;
        }
        if dist > 1 {
            // Ranged: projectile, optionally splashing around the target.
            self.objects
                .get_mut(&id)
                .unwrap()
                .monster_mut()
                .unwrap()
                .range_time = now + prof.range_cooldown_ms;
            self.face_and_swing(id, dir, Some(t));
            let delay = if prof.kite {
                400 + dist as u64 * PROJECTILE_MS
            } else {
                400
            };
            let splash = prof.splash > 0
                && (prof.splash_chance == 0 || self.rng.random_range(0..prof.splash_chance) == 0);
            if splash {
                let victims = self.hostiles_within(id, tloc, prof.splash);
                for v in victims {
                    self.push_monster_hit(id, v, delay, true, element::NONE);
                }
            } else {
                self.push_monster_hit(id, t, delay, true, element::NONE);
            }
            if prof.fear_rate > 0 && self.rng.random_range(0..prof.fear_rate) == 0 {
                let secs = prof.fear_duration_s + self.rng.random_range(0..4);
                self.objects
                    .get_mut(&id)
                    .unwrap()
                    .monster_mut()
                    .unwrap()
                    .fear_time = now + secs as u64 * 1000;
            }
            return;
        }
        self.monster_attack(id, t);
    }

    /// Face the target, broadcast the swing and start the attack timers.
    fn face_and_swing(&mut self, id: ObjectId, dir: Direction, ranged_at: Option<ObjectId>) {
        {
            let o = self.objects.get_mut(&id).unwrap();
            o.direction = dir;
            let (md, ad) = {
                let m = o.monster_ref();
                (m.move_delay, m.attack_delay)
            };
            let slow = slow_ms(o);
            o.attack_time = self.now + ad + slow;
            o.action_time = self.now + ad.saturating_sub(100).min(md.max(1)) + slow;
        }
        let loc = self.objects[&id].location;
        match ranged_at {
            Some(t) => self.events.push((
                id,
                ServerMessage::ObjectRangeAttack {
                    id,
                    direction: dir,
                    target: Some(t),
                    location: self.objects[&t].location,
                    magic: 0,
                },
            )),
            None => self.events.push((
                id,
                ServerMessage::ObjectAttack {
                    id,
                    direction: dir,
                    attack_magic: None,
                },
            )),
        }
        let _ = loc;
    }

    /// Hostile, living objects within `radius` of `centre` on the monster's map.
    fn hostiles_within(&self, id: ObjectId, centre: Point, radius: i32) -> Vec<ObjectId> {
        let me = &self.objects[&id];
        self.maps[&me.map]
            .objects
            .iter()
            .map(|o| &self.objects[o])
            .filter(|o| o.location.distance(centre) <= radius && me.hostile_to(o))
            .map(|o| o.id)
            .collect()
    }

    fn push_monster_hit(
        &mut self,
        id: ObjectId,
        target: ObjectId,
        delay: u64,
        ranged: bool,
        elem: u8,
    ) {
        let stats = self.objects[&id].stats;
        let mut power = self.roll_dc(stats);
        if self.objects[&id].monster_ref().bonus {
            power *= 2;
            self.objects
                .get_mut(&id)
                .unwrap()
                .monster_mut()
                .unwrap()
                .bonus = false;
        }
        let map = self.objects[&id].map;
        let tloc = self.objects[&target].location;
        self.pending_hits.push(PendingHit {
            time: self.now + delay,
            attacker: id,
            target_cell: (map, tloc),
            power,
            target: Some(target),
            magics: Vec::new(),
            primary: true,
            raw: false,
            element: elem,
            ranged,
        });
    }

    /// Zircon `LineAttack(n)`: the first object in each of the `n` cells
    /// along the facing direction is struck after 400 ms.
    fn monster_line_attack(&mut self, id: ObjectId, dir: Direction, n: i32) {
        self.face_and_swing(id, dir, None);
        let (map, loc) = {
            let o = &self.objects[&id];
            (o.map, o.location)
        };
        for i in 1..=n {
            let cell = loc.step(dir, i);
            let victim = self.maps[&map]
                .objects_at(cell)
                .iter()
                .copied()
                .find(|v| self.objects[&id].hostile_to(&self.objects[v]));
            if let Some(v) = victim {
                self.push_monster_hit(id, v, 400, true, element::NONE);
            }
        }
    }

    /// Zircon monster spells: queue the landing(s) and tell clients to play
    /// the cast with the payload on the affected targets/cells.
    fn monster_cast(&mut self, id: ObjectId, t: ObjectId, spell: Spell) {
        let spell = match spell {
            Spell::Random(list) if !list.is_empty() => {
                let i = self.rng.random_range(0..list.len());
                list[i].clone()
            }
            s => s,
        };
        let (map, loc) = {
            let o = &self.objects[&id];
            (o.map, o.location)
        };
        let tloc = self.objects[&t].location;
        let dir = Direction::from_points(loc, tloc);
        let now = self.now;
        {
            let o = self.objects.get_mut(&id).unwrap();
            o.direction = dir;
            let (md, ad) = {
                let m = o.monster_ref();
                (m.move_delay, m.attack_delay)
            };
            let slow = slow_ms(o);
            o.attack_time = now + ad + slow;
            o.action_time = now + ad.saturating_sub(100).min(md.max(1)) + slow;
        }
        let stats = self.objects[&id].stats;
        let dc = self.roll_dc(stats);
        let mut targets = Vec::new();
        let mut locations = Vec::new();
        let magic;
        match spell {
            Spell::Bolt {
                magic: m,
                element,
                travel,
                scale,
            } => {
                magic = m;
                targets.push(t);
                let delay = 500
                    + if travel {
                        tloc.distance(loc) as u64 * PROJECTILE_MS
                    } else {
                        0
                    };
                self.pending_monster_spells.push(PendingMonsterSpell {
                    time: now + delay,
                    caster: id,
                    targets: vec![t],
                    cells: Vec::new(),
                    power: dc * scale / 100,
                    element,
                    map,
                });
            }
            Spell::Aoe {
                radius,
                magic: m,
                element,
                at_self,
                scale,
            } => {
                magic = m;
                let centre = if at_self { loc } else { tloc };
                locations.push(centre);
                let cells: Vec<(Point, i32)> = (-radius..=radius)
                    .flat_map(|dx| {
                        (-radius..=radius).map(move |dy| Point::new(centre.x + dx, centre.y + dy))
                    })
                    .map(|p| (p, 100))
                    .collect();
                self.pending_monster_spells.push(PendingMonsterSpell {
                    time: now + 500,
                    caster: id,
                    targets: Vec::new(),
                    cells,
                    power: dc * scale / 100,
                    element,
                    map,
                });
            }
            Spell::Line {
                len,
                min,
                max,
                magic: m,
                element,
            } => {
                magic = m;
                for r in min..=max {
                    let d = dir.rotate(r);
                    let mut cells = Vec::new();
                    for i in 1..=len {
                        let p = loc.step(d, i);
                        if !self.maps[&map].file.is_walkable(p.x, p.y) {
                            break;
                        }
                        cells.push((p, 100));
                        // Flanks at half power (Zircon LineAoE).
                        cells.push((p.step(d.rotate(2), 1), 50));
                        cells.push((p.step(d.rotate(-2), 1), 50));
                        if i <= 3 {
                            locations.push(p);
                        }
                        self.pending_monster_spells.push(PendingMonsterSpell {
                            time: now + 500 + i as u64 * 75,
                            caster: id,
                            targets: Vec::new(),
                            cells: std::mem::take(&mut cells),
                            power: dc,
                            element,
                            map,
                        });
                    }
                }
            }
            Spell::Mass {
                magic: m,
                element,
                pct,
            } => {
                magic = m;
                let victims = self.hostiles_within(id, loc, MAX_VIEW_RANGE);
                for v in victims {
                    if pct < 100 && self.rng.random_range(0..100) >= pct {
                        continue;
                    }
                    let d = self.objects[&v].location.distance(loc) as u64;
                    targets.push(v);
                    self.pending_monster_spells.push(PendingMonsterSpell {
                        time: now + 500 + d * PROJECTILE_MS,
                        caster: id,
                        targets: vec![v],
                        cells: Vec::new(),
                        power: dc,
                        element,
                        map,
                    });
                }
            }
            Spell::ThunderStorm => {
                magic = magic_type::MONSTER_THUNDER_STORM;
                locations.push(loc);
                let cells: Vec<(Point, i32)> = (-2..=2)
                    .flat_map(|dx| (-2..=2).map(move |dy| Point::new(loc.x + dx, loc.y + dy)))
                    .map(|p| (p, 100))
                    .collect();
                self.pending_monster_spells.push(PendingMonsterSpell {
                    time: now + 500,
                    caster: id,
                    targets: Vec::new(),
                    cells,
                    power: dc,
                    element: element::LIGHTNING,
                    map,
                });
            }
            Spell::PoisonousCloud | Spell::FireWall => {
                // Persistent monster fields are not modelled yet: a burst
                // around the target stands in.
                magic = magic_type::FIRE_STORM;
                locations.push(tloc);
                let cells: Vec<(Point, i32)> = (-1..=1)
                    .flat_map(|dx| (-1..=1).map(move |dy| Point::new(tloc.x + dx, tloc.y + dy)))
                    .map(|p| (p, 100))
                    .collect();
                self.pending_monster_spells.push(PendingMonsterSpell {
                    time: now + 500,
                    caster: id,
                    targets: Vec::new(),
                    cells,
                    power: dc,
                    element: element::FIRE,
                    map,
                });
            }
            Spell::Random(_) => return,
        }
        self.events.push((
            id,
            ServerMessage::ObjectMagic {
                id,
                direction: dir,
                location: tloc,
                magic,
                targets,
                locations,
                cast: true,
            },
        ));
    }

    /// Land due monster spells: elemental damage through MR on every hostile
    /// target or cell occupant.
    pub(super) fn process_monster_spells(&mut self) {
        let due: Vec<PendingMonsterSpell> = {
            let (due, later): (Vec<_>, Vec<_>) = self
                .pending_monster_spells
                .drain(..)
                .partition(|s| s.time <= self.now);
            self.pending_monster_spells = later;
            due
        };
        for s in due {
            if self.objects.get(&s.caster).map(|o| o.dead).unwrap_or(true) {
                continue;
            }
            let mut victims: Vec<(ObjectId, i32)> = s.targets.iter().map(|t| (*t, 100)).collect();
            for (cell, scale) in &s.cells {
                if let Some(m) = self.maps.get(&s.map) {
                    for v in m.objects_at(*cell) {
                        victims.push((*v, *scale));
                    }
                }
            }
            for (v, scale) in victims {
                let Some(target) = self.objects.get(&v) else {
                    continue;
                };
                if target.dead || v == s.caster || !self.objects[&s.caster].hostile_to(target) {
                    continue;
                }
                let tstats = target.stats;
                let mr = self.roll_range(tstats.min_mr, tstats.max_mr);
                let power = s.power * scale / 100 - mr;
                if power > 0 {
                    self.damage(v, s.caster, power, s.element, true);
                }
            }
        }
    }

    /// Zircon `ProcessTarget`: plain melee when adjacent (kept for pets and
    /// the base loop).
    pub(super) fn monster_attack(&mut self, id: ObjectId, target: ObjectId) {
        let tloc = self.objects[&target].location;
        let loc = self.objects[&id].location;
        let dir = Direction::from_points(loc, tloc);
        self.face_and_swing(id, dir, None);
        self.push_monster_hit(id, target, 400, false, element::NONE);
    }

    pub(super) fn monster_can_move(&self, id: ObjectId) -> bool {
        let o = &self.objects[&id];
        let m = o.monster_ref();
        !o.dead
            && m.move_delay > 0
            && self.now >= o.action_time
            && self.now >= o.move_time
            && self.now >= m.shock_until
            && !o.poisons.iter().any(|p| {
                matches!(
                    p.kind,
                    poison_kind::WRAITH_GRIP
                        | poison_kind::SILENCED
                        | poison_kind::CONTAINMENT
                        | poison_kind::BINDING
                )
            })
    }

    pub(super) fn monster_can_attack(&self, id: ObjectId) -> bool {
        let o = &self.objects[&id];
        let m = o.monster_ref();
        !o.dead
            && m.attack_delay > 0
            && self.now >= o.action_time
            && self.now >= o.attack_time
            && !has_poison(o, poison_kind::FEAR)
            && !has_poison(o, poison_kind::SILENCED)
    }

    pub(super) fn monster_walk(&mut self, id: ObjectId, dir: Direction) -> bool {
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
            let slow = slow_ms(o);
            o.move_time = self.now + md + slow;
            o.action_time = self.now + md.saturating_sub(100).min(ad) + slow;
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

    pub(super) fn monster_turn(&mut self, id: ObjectId, dir: Direction) {
        let o = self.objects.get_mut(&id).unwrap();
        o.direction = dir;
        let (md, ad) = {
            let m = o.monster_ref();
            (m.move_delay, m.attack_delay)
        };
        let slow = slow_ms(o);
        o.move_time = self.now + md + slow;
        o.action_time = self.now + md.saturating_sub(100).min(ad) + slow;
        self.events
            .push((id, ServerMessage::ObjectTurn { id, direction: dir }));
    }

    /// Profile hooks on death: suicide bursts and corpse spawns.
    pub(super) fn monster_death_effects(&mut self, id: ObjectId) {
        let ai = self.monster_ai(id);
        let prof = self.ai_profile(ai);
        let loc = self.objects[&id].location;
        if prof.suicide_range > 0 {
            let victims = self.hostiles_within(id, loc, prof.suicide_range);
            for v in victims {
                self.push_monster_hit(id, v, 800, true, element::NONE);
            }
        }
        if let Some((radius, flag, per_player)) = prof.die_splash {
            let victims = self.hostiles_within(id, loc, radius);
            let players: Vec<ObjectId> = victims
                .iter()
                .copied()
                .filter(|v| self.objects[v].is_player())
                .collect();
            for v in victims {
                self.push_monster_hit(id, v, 400, true, element::NONE);
            }
            for p in players {
                self.spawn_minions(id, per_player, &[(flag, 1)], 20, Some(p), 1);
            }
        }
    }

    /// Zircon `UnTame`: back to the wild at a tenth of its health.
    pub(super) fn untame(&mut self, id: ObjectId) {
        let owner = self.objects[&id].monster_ref().owner;
        if let Some(p) = owner
            .and_then(|o| self.objects.get_mut(&o))
            .and_then(|o| o.player_mut())
        {
            p.pets.retain(|x| *x != id);
        }
        if let Some(o) = self.objects.get_mut(&id) {
            if let Kind::Monster(m) = &mut o.kind {
                m.owner = None;
                m.target = None;
                m.summon_level = 0;
                m.tame_until = 0;
                m.search_time = 0;
            }
            if let Appearance::Monster { owner, .. } = &mut o.appearance {
                *owner = None;
            }
        }
        self.refresh_monster_stats(id, false);
        let (max_hp, appearance) = {
            let o = &self.objects[&id];
            (o.max_hp, o.appearance.clone())
        };
        if let Some(o) = self.objects.get_mut(&id) {
            o.hp = o.hp.min((max_hp / 10).max(1));
        }
        self.events
            .push((id, ServerMessage::ObjectAppearance { id, appearance }));
    }

    /// Make `id` the pet of `owner` (Zircon tame/summon bookkeeping).
    pub(super) fn tame(&mut self, id: ObjectId, owner: ObjectId, summon_level: i32, tame_ms: u64) {
        let owner_name = self.objects[&owner].player().map(|p| p.name.clone());
        let old_spawn = self.objects[&id].monster_ref().spawn;
        if let Some((map, gi)) = old_spawn {
            if let Some(g) = self.maps.get_mut(&map).and_then(|m| m.spawns.get_mut(gi)) {
                g.alive -= 1;
            }
        }
        if let Some(o) = self.objects.get_mut(&id) {
            if let Kind::Monster(m) = &mut o.kind {
                m.owner = Some(owner);
                m.spawn = None;
                m.target = None;
                m.summon_level = summon_level;
                m.tame_until = self.now.saturating_add(tame_ms);
                m.shock_until = 0;
            }
            if let Appearance::Monster { owner: on, .. } = &mut o.appearance {
                *on = owner_name;
            }
        }
        if let Some(p) = self.objects.get_mut(&owner).and_then(|o| o.player_mut()) {
            if !p.pets.contains(&id) {
                p.pets.push(id);
            }
        }
        self.refresh_monster_stats(id, false);
        let appearance = self.objects[&id].appearance.clone();
        self.events
            .push((id, ServerMessage::ObjectAppearance { id, appearance }));
    }

    /// Re-scale every pet of a player (after Strength Of Faith changes).
    pub(super) fn refresh_pets(&mut self, owner: ObjectId) {
        let pets: Vec<ObjectId> = self
            .objects
            .get(&owner)
            .and_then(|o| o.player())
            .map(|p| p.pets.clone())
            .unwrap_or_default();
        for pet in pets {
            self.refresh_monster_stats(pet, false);
        }
    }

    /// Zircon `Puppet.Die`: the owner's Summon Puppet hits everything within
    /// two cells 800 ms later.
    pub(super) fn puppet_explode(&mut self, id: ObjectId) {
        let (owner, map, loc) = {
            let o = &self.objects[&id];
            (o.monster_ref().owner, o.map, o.location)
        };
        self.events.push((
            id,
            ServerMessage::ObjectEffect {
                id,
                effect: effect::PUPPET,
                location: loc,
            },
        ));
        if let Some(owner) = owner {
            let victims: Vec<ObjectId> = self
                .on_map(map)
                .filter(|o| !o.dead && o.location.distance(loc) <= 2)
                .filter(|o| matches!(&o.kind, Kind::Monster(m) if m.owner.is_none()))
                .map(|o| o.id)
                .collect();
            for v in victims {
                self.pending_magics.push(PendingMagic {
                    time: self.now + 800,
                    caster: owner,
                    magic: magic_type::SUMMON_PUPPET,
                    target: Some(v),
                    location: loc,
                    direction: None,
                    primary: true,
                    chain: None,
                });
            }
        }
        self.monster_die(id, owner.unwrap_or(id));
    }
}

/// Whether the object carries a poison of `kind`.
pub(super) fn has_poison(o: &Object, kind: u16) -> bool {
    o.poisons.iter().any(|p| p.kind == kind)
}
