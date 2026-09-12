use super::*;

impl World {
    // ---- monster AI --------------------------------------------------------

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
        if !dead && paralysed(&self.objects[&id]) {
            return;
        }
        if dead {
            if now > dead_time {
                self.remove_object(id);
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
            let is_pet = self.objects[&id].monster_ref().owner.is_some();
            if search_due && (!passive || is_pet) {
                self.objects
                    .get_mut(&id)
                    .unwrap()
                    .monster_mut()
                    .unwrap()
                    .search_time = now + SEARCH_DELAY;
                let mut best: Vec<ObjectId> = Vec::new();
                let mut best_d = i32::MAX;
                let me = &self.objects[&id];
                for pid in &self.maps[&map].objects {
                    let p = &self.objects[pid];
                    if !me.hostile_to(p) || !self.can_see(me, p) {
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

        // Pets with nothing to fight walk to the cell behind their owner.
        if let Some(owner_id) = self.objects[&id].monster_ref().owner {
            if target.is_none() && self.monster_can_move(id) {
                let (oloc, odir) = {
                    let o = &self.objects[&owner_id];
                    (o.location, o.direction)
                };
                let goal = oloc.step(odir.rotate(4), 1);
                if goal != loc {
                    let dir = Direction::from_points(loc, goal);
                    for i in 0..8 {
                        let d = dir.rotate(if i % 2 == 0 {
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
            if let Some(t) = target {
                self.chase_or_attack(id, t, loc);
            }
            return;
        }
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
        self.chase_or_attack(id, t, loc);
    }

    /// Zircon `ProcessTarget`: attack when adjacent, else step toward it.
    fn chase_or_attack(&mut self, id: ObjectId, t: ObjectId, loc: Point) {
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

    pub(super) fn monster_can_move(&self, id: ObjectId) -> bool {
        let o = &self.objects[&id];
        let m = o.monster_ref();
        !o.dead
            && m.move_delay > 0
            && self.now >= o.action_time
            && self.now >= o.move_time
            && self.now >= m.shock_until
    }

    pub(super) fn monster_can_attack(&self, id: ObjectId) -> bool {
        let o = &self.objects[&id];
        let m = o.monster_ref();
        !o.dead && m.attack_delay > 0 && self.now >= o.action_time && self.now >= o.attack_time
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

    pub(super) fn monster_attack(&mut self, id: ObjectId, target: ObjectId) {
        let tloc = self.objects[&target].location;
        let (dir, power, map, loc) = {
            let o = self.objects.get_mut(&id).unwrap();
            let dir = Direction::from_points(o.location, tloc);
            o.direction = dir;
            let (md, ad) = {
                let m = o.monster_ref();
                (m.move_delay, m.attack_delay)
            };
            let slow = slow_ms(o);
            o.attack_time = self.now + ad + slow;
            o.action_time = self.now + md.min(ad.saturating_sub(100)) + slow;
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
            raw: false,
        });
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
}
