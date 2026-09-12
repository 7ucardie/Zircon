use super::*;

impl World {
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
        let mut cast_ok = true;
        let pm =
            |time: u64, target: Option<ObjectId>, location: Point, primary: bool| PendingMagic {
                time,
                caster: id,
                magic,
                target,
                location,
                direction: None,
                primary,
            };
        let now = self.now;
        match magic {
            // Projectiles: 500 ms + 48 ms per cell of distance (Cyclone: flat 600).
            magic_type::FIRE_BALL
            | magic_type::ICE_BOLT
            | magic_type::FLAMING_DAGGERS
            | magic_type::SHREDDING
            | magic_type::LIGHTNING_BALL
            | magic_type::GUST_BLAST
            | magic_type::ADAMANTINE_FIRE_BALL
            | magic_type::ICE_BLADES
            | magic_type::CYCLONE
            | magic_type::EVIL_SLAYER
            | magic_type::GREATER_EVIL_SLAYER => {
                let base = match magic {
                    magic_type::FLAMING_DAGGERS | magic_type::SHREDDING => 1000,
                    _ => 500,
                };
                match target.filter(|t| self.objects[t].is_monster()) {
                    Some(t) => {
                        targets.push(t);
                        let time = if magic == magic_type::CYCLONE {
                            now + 600
                        } else {
                            now + base + dist(self, t) * 48
                        };
                        pending.push(pm(time, Some(t), location, true));
                        // Evil Slayer burns a holy talisman when one is worn.
                        if matches!(
                            magic,
                            magic_type::EVIL_SLAYER | magic_type::GREATER_EVIL_SLAYER
                        ) && self.amulet_holy(id)
                        {
                            self.use_amulet(id, 1);
                        }
                    }
                    None => locations.push(location),
                }
            }
            magic_type::EXPLOSIVE_TALISMAN => match target.filter(|t| self.objects[t].is_monster())
            {
                Some(t) => {
                    if self.use_amulet(id, 1).is_some() {
                        targets.push(t);
                        pending.push(pm(now + 500 + dist(self, t) * 48, Some(t), location, true));
                    } else {
                        self.send_to(
                            id,
                            ServerMessage::Chat {
                                text: "You need a talisman.".into(),
                            },
                        );
                    }
                }
                None => locations.push(location),
            },
            magic_type::THUNDER_BOLT | magic_type::POISON_DUST => {
                match target.filter(|t| self.objects[t].is_monster()) {
                    Some(t) => {
                        targets.push(t);
                        pending.push(pm(
                            now + if magic == magic_type::THUNDER_BOLT {
                                600
                            } else {
                                500
                            },
                            Some(t),
                            location,
                            true,
                        ));
                    }
                    None => locations.push(location),
                }
            }
            magic_type::REPULSION => {
                for d in Direction::ALL {
                    pending.push(PendingMagic {
                        time: now + 500,
                        caster: id,
                        magic,
                        target: None,
                        location: loc.step(d, 1),
                        direction: Some(d),
                        primary: true,
                    });
                }
            }
            magic_type::HEAL => {
                let t = target.filter(|t| self.objects[t].is_player()).unwrap_or(id);
                targets.push(t);
                pending.push(pm(now + 500, Some(t), location, true));
            }
            // Self casts.
            magic_type::TELEPORTATION => pending.push(pm(now + 500, None, loc, true)),
            magic_type::MAGIC_SHIELD => pending.push(pm(now + 1100, None, loc, true)),
            magic_type::DEFIANCE | magic_type::MIGHT | magic_type::POISONOUS_CLOUD => {
                pending.push(pm(now + 500, None, loc, true))
            }
            // Lines: 8 cells ahead with flanks; delay 800 (Lightning Beam 500).
            m if magic_type::is_line(m) => {
                let delay = if m == magic_type::LIGHTNING_BEAM {
                    500
                } else {
                    800
                };
                for (i, (cell, primary)) in Self::line_cells(loc, direction).into_iter().enumerate()
                {
                    if primary && (m != magic_type::LIGHTNING_BEAM || i == 0) {
                        locations.push(cell);
                    }
                    pending.push(pm(now + delay, None, cell, primary));
                }
            }
            // Ground casts.
            magic_type::FIRE_WALL => {
                if location.distance(loc) > MAGIC_RANGE {
                    cast_ok = false;
                } else {
                    for cell in [
                        location,
                        location.step(Direction::Up, 1),
                        location.step(Direction::Down, 1),
                        location.step(Direction::Left, 1),
                        location.step(Direction::Right, 1),
                    ] {
                        pending.push(pm(now + 500, None, cell, true));
                    }
                }
            }
            magic_type::MASS_HEAL => {
                if location.distance(loc) > MAGIC_RANGE {
                    cast_ok = false;
                } else {
                    locations.push(location);
                    let d = location.distance(loc) as u64;
                    pending.push(pm(now + 500 + d * 48, None, location, true));
                }
            }
            magic_type::MAGIC_RESISTANCE | magic_type::RESILIENCE => {
                let need = if magic == magic_type::RESILIENCE {
                    2
                } else {
                    1
                };
                if location.distance(loc) > MAGIC_RANGE {
                    cast_ok = false;
                } else if let Some((_, _)) = self.use_amulet(id, need) {
                    locations.push(location);
                    let d = location.distance(loc) as u64;
                    pending.push(pm(now + 500 + d * 48, None, location, true));
                } else {
                    cast_ok = false;
                    self.send_to(
                        id,
                        ServerMessage::Chat {
                            text: format!("{} needs {need} talisman(s).", def.name),
                        },
                    );
                }
            }
            magic_type::SHOULDER_DASH => {
                // Zircon: no ObjectMagic; the dash streams as ObjectDash steps.
                let already = self.objects[&id].player().unwrap().dash.is_some();
                if already {
                    return;
                }
                let (pmin, pmax) = um.power_range(&def);
                let distance = self.roll_range(pmin, pmax).max(1);
                {
                    let o = self.objects.get_mut(&id).unwrap();
                    o.direction = direction;
                    let p = o.player_mut().unwrap();
                    p.mp -= cost;
                    p.dash = Some(Dash {
                        remaining: distance,
                        travelled: 0,
                        direction,
                        next_step: now,
                    });
                    if let Some(m) = p.magics.iter_mut().find(|m| m.magic == magic) {
                        m.cooldown_until = now + def.delay.max(0) as u64;
                    }
                }
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
                return;
            }
            _ => {}
        }
        // Pay, set timers (Zircon: consume even when the spell fizzles).
        let face = match target {
            _ if magic_type::is_stance_cast(magic) => Direction::Down,
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
            if cast_ok {
                if let Some(m) = p.magics.iter_mut().find(|m| m.magic == magic) {
                    m.cooldown_until = self.now + def.delay.max(0) as u64;
                }
            }
        }
        let dir = self.objects[&id].direction;
        if def.delay > 0 && cast_ok {
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
                cast: cast_ok,
            },
        ));
        self.pending_magics.extend(pending);
    }

    /// Spell damage (Zircon `MagicAttack`): magic power plus the class stat,
    /// minus the target's MR and element resistance.
    pub(super) fn magic_attack(
        &mut self,
        caster: ObjectId,
        target: ObjectId,
        magic: u16,
        elem: u8,
        scale: i32,
    ) -> i32 {
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
        if !c.hostile_to(t) {
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
        // Zircon `ModifyPowerMultiplier` (flank cells 30 %, fire wall 60 %...).
        power = power * scale / 100;
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

    pub(super) fn resolve_magics(&mut self) {
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
                        self.magic_attack(pm.caster, t, pm.magic, element::FIRE, 100);
                    }
                }
                magic_type::ICE_BOLT => {
                    if let Some(t) = pm.target {
                        self.magic_attack(pm.caster, t, pm.magic, element::ICE, 100);
                    }
                }
                magic_type::THUNDER_BOLT => {
                    if let Some(t) = pm.target {
                        self.magic_attack(pm.caster, t, pm.magic, element::LIGHTNING, 100);
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
                // Single-target elemental bolts with side effects.
                magic_type::LIGHTNING_BALL => {
                    if let Some(t) = pm.target {
                        self.magic_attack(pm.caster, t, pm.magic, element::LIGHTNING, 100);
                    }
                }
                magic_type::ADAMANTINE_FIRE_BALL => {
                    if let Some(t) = pm.target {
                        self.magic_attack(pm.caster, t, pm.magic, element::FIRE, 100);
                    }
                }
                magic_type::ICE_BLADES => {
                    if let Some(t) = pm.target {
                        let dealt = self.magic_attack(pm.caster, t, pm.magic, element::ICE, 100);
                        if dealt > 0 {
                            self.try_slow(t, pm.caster, 5, 5);
                        }
                    }
                }
                magic_type::GUST_BLAST | magic_type::CYCLONE => {
                    if let Some(t) = pm.target {
                        let dealt = self.magic_attack(pm.caster, t, pm.magic, element::WIND, 100);
                        let repel = if pm.magic == magic_type::CYCLONE {
                            5
                        } else {
                            10
                        };
                        if dealt > 0 {
                            self.try_repel(t, pm.caster, repel);
                        }
                    }
                }
                magic_type::EVIL_SLAYER | magic_type::GREATER_EVIL_SLAYER => {
                    if let Some(t) = pm.target {
                        // Holy talisman bonus: 30 % (60 % for the greater one).
                        let scale = if self.amulet_holy(pm.caster) {
                            if pm.magic == magic_type::GREATER_EVIL_SLAYER {
                                160
                            } else {
                                130
                            }
                        } else {
                            100
                        };
                        self.magic_attack(pm.caster, t, pm.magic, element::HOLY, scale);
                    }
                }
                magic_type::EXPLOSIVE_TALISMAN => {
                    if let Some(t) = pm.target {
                        self.magic_attack(pm.caster, t, pm.magic, element::DARK, 100);
                    }
                }
                // Lines: everything on the cell; flanks for 30 %.
                m if magic_type::is_line(m) => {
                    let Some(map) = self.maps.get(&cmap) else {
                        continue;
                    };
                    let victims: Vec<ObjectId> = map
                        .objects_at(pm.location)
                        .iter()
                        .copied()
                        .filter(|v| self.objects[v].is_monster() && !self.objects[v].dead)
                        .collect();
                    let elem = match m {
                        magic_type::SCORCHED_EARTH => element::FIRE,
                        magic_type::LIGHTNING_BEAM => element::LIGHTNING,
                        magic_type::FROZEN_EARTH => element::ICE,
                        _ => element::WIND,
                    };
                    let scale = if pm.primary { 100 } else { 30 };
                    for v in victims {
                        let dealt = self.magic_attack(pm.caster, v, m, elem, scale);
                        if dealt > 0 {
                            match m {
                                magic_type::FROZEN_EARTH => self.try_slow(v, pm.caster, 10, 3),
                                magic_type::BLOW_EARTH => self.try_repel(v, pm.caster, 10),
                                _ => {}
                            }
                        }
                    }
                }
                magic_type::FIRE_WALL => {
                    let walkable = self
                        .maps
                        .get(&cmap)
                        .map(|m| m.file.is_walkable(pm.location.x, pm.location.y))
                        .unwrap_or(false);
                    if !walkable {
                        continue;
                    }
                    let level = self.objects[&pm.caster]
                        .player()
                        .and_then(|p| p.magics.iter().find(|m| m.magic == pm.magic))
                        .map(|m| m.level as i32)
                        .unwrap_or(0);
                    self.spawn_spell(
                        cmap,
                        pm.location,
                        spell_effect::FIRE_WALL,
                        (level + 2) * 5,
                        2000,
                        pm.caster,
                        pm.magic,
                    );
                    self.level_magic(pm.caster, pm.magic);
                }
                magic_type::POISONOUS_CLOUD => {
                    let exists = self.maps[&cmap]
                        .objects_at(cloc)
                        .iter()
                        .any(|v| matches!(&self.objects[v].kind, Kind::Spell(s) if s.effect == spell_effect::POISONOUS_CLOUD));
                    if exists {
                        continue;
                    }
                    let (pmin, pmax) = {
                        let p = self.objects[&pm.caster].player().unwrap();
                        let um = p.magics.iter().find(|m| m.magic == pm.magic).unwrap();
                        um.power_range(&self.data.magics[&pm.magic])
                    };
                    let seconds = self.roll_range(pmin, pmax).max(1) as u64;
                    self.spawn_spell(
                        cmap,
                        cloc,
                        spell_effect::POISONOUS_CLOUD,
                        1,
                        seconds * 1000,
                        pm.caster,
                        pm.magic,
                    );
                    self.level_magic(pm.caster, pm.magic);
                }
                magic_type::TELEPORTATION => {
                    let level = self.objects[&pm.caster]
                        .player()
                        .and_then(|p| p.magics.iter().find(|m| m.magic == pm.magic))
                        .map(|m| m.level as i32)
                        .unwrap_or(0);
                    // Random.Next(MagicMaxLevel + 5) > 2 + Level * 2 => fail.
                    if self.rng.random_range(0..9) > 2 + level * 2 {
                        self.level_magic(pm.caster, pm.magic);
                        continue;
                    }
                    let Some(to) = self.random_walkable(cmap) else {
                        continue;
                    };
                    self.events.push((
                        pm.caster,
                        ServerMessage::ObjectEffect {
                            id: pm.caster,
                            effect: effect::TELEPORT_OUT,
                            location: cloc,
                        },
                    ));
                    self.move_object(pm.caster, to);
                    let dir = self.objects[&pm.caster].direction;
                    self.send_to(
                        pm.caster,
                        ServerMessage::MoveDenied {
                            location: to,
                            direction: dir,
                        },
                    );
                    self.events.push((
                        pm.caster,
                        ServerMessage::ObjectEffect {
                            id: pm.caster,
                            effect: effect::TELEPORT_IN,
                            location: to,
                        },
                    ));
                    self.level_magic(pm.caster, pm.magic);
                }
                magic_type::MAGIC_SHIELD => {
                    let c = &self.objects[&pm.caster];
                    if c.has_buff(buff_type::MAGIC_SHIELD) {
                        continue;
                    }
                    let (level, mc) = {
                        let p = c.player().unwrap();
                        let um = p.magics.iter().find(|m| m.magic == pm.magic).unwrap();
                        (um.level as i64, (c.stats.min_mc, c.stats.max_mc))
                    };
                    let mc = self.roll_range(mc.0, mc.1) as i64;
                    let secs = 30 + level * 20 + mc / 2;
                    self.buff_add(
                        pm.caster,
                        buff_type::MAGIC_SHIELD,
                        secs as u64 * 1000,
                        BuffStats {
                            magic_shield: 50,
                            ..BuffStats::default()
                        },
                    );
                    self.level_magic(pm.caster, pm.magic);
                }
                magic_type::DEFIANCE | magic_type::MIGHT => {
                    let level = self.objects[&pm.caster]
                        .player()
                        .and_then(|p| p.magics.iter().find(|m| m.magic == pm.magic))
                        .map(|m| m.level as i32)
                        .unwrap_or(0);
                    let secs = (60 + level * 30) as u64;
                    let amount = 5 + level * 5;
                    if pm.magic == magic_type::DEFIANCE {
                        self.buff_remove(pm.caster, buff_type::MIGHT);
                        self.buff_add(
                            pm.caster,
                            buff_type::DEFIANCE,
                            secs * 1000,
                            BuffStats {
                                phys_def_pct: amount,
                                mag_def_pct: amount,
                                dc_pct: -20,
                                ..BuffStats::default()
                            },
                        );
                    } else {
                        self.buff_remove(pm.caster, buff_type::DEFIANCE);
                        self.buff_add(
                            pm.caster,
                            buff_type::MIGHT,
                            secs * 1000,
                            BuffStats {
                                dc_pct: amount,
                                phys_def_pct: -amount,
                                mag_def_pct: -amount,
                                ..BuffStats::default()
                            },
                        );
                    }
                    self.level_magic(pm.caster, pm.magic);
                }
                magic_type::MASS_HEAL => {
                    let (pmin, pmax, sc) = {
                        let c = &self.objects[&pm.caster];
                        let p = c.player().unwrap();
                        let um = p.magics.iter().find(|m| m.magic == pm.magic).unwrap();
                        let (pmin, pmax) = um.power_range(&self.data.magics[&pm.magic]);
                        (pmin, pmax, (c.stats.min_sc, c.stats.max_sc))
                    };
                    let players: Vec<ObjectId> = self
                        .objects
                        .values()
                        .filter(|o| {
                            o.is_player()
                                && o.map == cmap
                                && o.location.distance(pm.location) <= 2
                                && !o.dead
                                && o.hp < o.max_hp
                                && o.heal.is_none()
                        })
                        .map(|o| o.id)
                        .collect();
                    for t in players {
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
                }
                magic_type::MAGIC_RESISTANCE | magic_type::RESILIENCE => {
                    let (level, pmin, pmax, sc) = {
                        let c = &self.objects[&pm.caster];
                        let p = c.player().unwrap();
                        let um = p.magics.iter().find(|m| m.magic == pm.magic).unwrap();
                        let (pmin, pmax) = um.power_range(&self.data.magics[&pm.magic]);
                        (
                            um.level as i32,
                            pmin,
                            pmax,
                            (c.stats.min_sc, c.stats.max_sc),
                        )
                    };
                    let players: Vec<ObjectId> = self
                        .objects
                        .values()
                        .filter(|o| {
                            o.is_player()
                                && o.map == cmap
                                && !o.dead
                                && o.location.distance(pm.location) <= 3
                        })
                        .map(|o| o.id)
                        .collect();
                    for t in players {
                        let secs = (self.roll_range(pmin, pmax) + self.roll_range(sc.0, sc.1) * 2)
                            .max(1) as u64;
                        let (kind, stats) = if pm.magic == magic_type::MAGIC_RESISTANCE {
                            (
                                buff_type::MAGIC_RESISTANCE,
                                BuffStats {
                                    max_mr: 5 + level,
                                    ..BuffStats::default()
                                },
                            )
                        } else {
                            (
                                buff_type::RESILIENCE,
                                BuffStats {
                                    max_ac: 5 + level,
                                    ..BuffStats::default()
                                },
                            )
                        };
                        self.buff_add(t, kind, secs * 1000, stats);
                        self.level_magic(pm.caster, pm.magic);
                    }
                }
                _ => {}
            }
            let _ = cloc;
        }
    }

    /// Zircon `MagicAttack` slow proc: 1-in-`chance` on non-boss monsters,
    /// value doubled, 6..10 s.
    pub(super) fn try_slow(&mut self, target: ObjectId, owner: ObjectId, chance: i32, level: i32) {
        if self.rng.random_range(0..chance.max(1)) != 0 {
            return;
        }
        let boss = match &self.objects.get(&target).map(|o| &o.kind) {
            Some(Kind::Monster(m)) => self.data.monsters[&m.def].is_boss,
            _ => return,
        };
        if boss {
            return;
        }
        let secs = (3 + self.rng.random_range(0..3)) * 2;
        self.apply_poison(
            target,
            Poison {
                kind: 4,
                value: level * 2,
                ticks_left: 1,
                next_tick: self.now + secs as u64 * 1000,
                owner: Some(owner),
            },
        );
    }

    /// Zircon `MagicAttack` repel proc: 1-in-`chance`, one cell away from the
    /// caster, only on lower-level monsters.
    pub(super) fn try_repel(&mut self, target: ObjectId, caster: ObjectId, chance: i32) {
        if self.rng.random_range(0..chance.max(1)) != 0 {
            return;
        }
        let (map, tloc, cloc, clevel) = {
            let c = &self.objects[&caster];
            let t = &self.objects[&target];
            (
                t.map,
                t.location,
                c.location,
                c.player().map(|p| p.level).unwrap_or(1),
            )
        };
        let (mlevel, boss) = match &self.objects[&target].kind {
            Kind::Monster(m) => (
                self.data.monsters[&m.def].level,
                self.data.monsters[&m.def].is_boss,
            ),
            _ => return,
        };
        if boss || clevel <= mlevel || cloc == tloc {
            return;
        }
        let dir = Direction::from_points(cloc, tloc);
        let rotation = if self.rng.random_range(0..2) == 0 {
            1
        } else {
            -1
        };
        for d in [dir, dir.rotate(rotation), dir.rotate(-rotation)] {
            let to = tloc.step(d, 1);
            if self.cell_blocked(map, to, false) {
                continue;
            }
            self.move_object(target, to);
            if let Some(o) = self.objects.get_mut(&target) {
                o.direction = d.rotate(4);
            }
            self.events.push((
                target,
                ServerMessage::ObjectMove {
                    id: target,
                    from: tloc,
                    to,
                    direction: d,
                    run: false,
                },
            ));
            return;
        }
    }
}
