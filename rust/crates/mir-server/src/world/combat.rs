use super::*;

impl World {
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
        let mut karma_cost = 0;
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
            let moon_roll = self.rng.random_range(0..5);
            let (o_hp, o_max_hp) = (o.hp, o.max_hp);
            let release = o.player().and_then(|p| {
                let um = p.magics.iter().find(|m| m.magic == magic_type::RELEASE)?;
                let def = self.data.magics.get(&um.magic)?;
                (p.level >= def.need_level[0]).then(|| um.power_range(def))
            });
            let resolution = o
                .player()
                .map(|p| p.magics.iter().any(|m| m.magic == magic_type::RESOLUTION))
                .unwrap_or(false);
            let p = o.player_mut().unwrap();
            for (m, need, cost) in owned {
                if level < need {
                    continue;
                }
                match m {
                    magic_type::SWORDSMANSHIP
                    | magic_type::SPIRIT_SWORD
                    | magic_type::VINE_TREE_DANCE
                    | magic_type::DISCIPLINE
                    | magic_type::BLOODY_FLOWER => magics.push(m),
                    // Charged power attacks: consumed by the swing they were armed for.
                    magic_type::FLAMING_SWORD
                    | magic_type::DRAGON_RISE
                    | magic_type::BLADE_STORM => {
                        if attack_magic == Some(m)
                            && p.charge
                                .map(|(c, until)| c == m && self.now < until)
                                .unwrap_or(false)
                        {
                            p.charge = None;
                            toggles.push((m, false));
                            valid = Some(m);
                            magics.push(m);
                        }
                    }
                    magic_type::DESTRUCTIVE_SURGE => {
                        if attack_magic == Some(m) && p.surge_on && cost <= p.mp {
                            p.mp -= cost;
                            valid = Some(m);
                            magics.push(m);
                        }
                    }
                    // Lotus combo: the armed swing pays and cools down on a hit.
                    magic_type::FULL_BLOOM
                    | magic_type::WHITE_LOTUS
                    | magic_type::RED_LOTUS
                    | magic_type::SWEET_BRIER => {
                        let cd = p
                            .magics
                            .iter()
                            .find(|x| x.magic == m)
                            .map(|x| x.cooldown_until)
                            .unwrap_or(0);
                        if attack_magic == Some(m) && self.now >= cd && cost <= p.mp {
                            p.mp -= cost;
                            valid = Some(m);
                            magics.push(m);
                        }
                    }
                    // Karma: an HP-priced strike that needs the cloak.
                    magic_type::KARMA => {
                        let cd = p
                            .magics
                            .iter()
                            .find(|x| x.magic == m)
                            .map(|x| x.cooldown_until)
                            .unwrap_or(0);
                        let cloaked = p.buffs.iter().any(|b| b.kind == buff_type::CLOAK);
                        if attack_magic == Some(m) && self.now >= cd && cloaked {
                            let mut hp_cost = o_max_hp * cost / 100;
                            if let Some((rmin, _)) = release {
                                hp_cost -= hp_cost * rmin / 100;
                                magics.push(magic_type::RELEASE);
                            }
                            if resolution {
                                magics.push(magic_type::RESOLUTION);
                            }
                            if hp_cost < o_hp {
                                karma_cost = hp_cost;
                                valid = Some(m);
                                magics.push(m);
                            }
                        }
                    }
                    // Moon charges: consumed when armed, re-armed by a roll.
                    magic_type::CALAMITY_OF_FULL_MOON | magic_type::WANING_MOON => {
                        let cloaked = p.buffs.iter().any(|b| b.kind == buff_type::CLOAK);
                        let ready = if m == magic_type::CALAMITY_OF_FULL_MOON {
                            &mut p.full_moon_ready
                        } else {
                            &mut p.waning_moon_ready
                        };
                        if *ready && attack_magic == Some(m) {
                            *ready = false;
                            toggles.push((m, false));
                            valid = Some(m);
                            magics.push(m);
                        }
                        let allowed = if m == magic_type::CALAMITY_OF_FULL_MOON {
                            !cloaked
                        } else {
                            cloaked
                        };
                        let level = p
                            .magics
                            .iter()
                            .find(|x| x.magic == m)
                            .map(|x| x.level as i32)
                            .unwrap_or(0);
                        if !*ready && allowed && moon_roll > level {
                            *ready = true;
                            toggles.push((m, true));
                        }
                    }
                    magic_type::FLAME_SPLASH => {
                        if attack_magic == Some(m) && p.flame_splash_on && cost <= p.mp {
                            p.mp -= cost;
                            valid = Some(m);
                            magics.push(m);
                        }
                    }
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
        if karma_cost > 0 && attack_magic == valid {
            let o = self.objects.get_mut(&id).unwrap();
            o.hp -= karma_cost;
            let (hp, max_hp) = (o.hp, o.max_hp);
            self.events
                .push((id, ServerMessage::HealthChanged { id, hp, max_hp }));
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
        // Dragon Rise lands every hit of the swing at 600 ms instead of 300.
        let delay = if magics.contains(&magic_type::DRAGON_RISE) {
            600
        } else {
            300
        };
        let front = loc.step(direction, 1);
        self.pending_hits.push(PendingHit {
            time: self.now + delay,
            attacker: id,
            target_cell: (map, front),
            power,
            target: None,
            magics: magics.clone(),
            primary: true,
            raw: false,
        });
        // Secondary cells (Zircon `SecondaryAttackLocation`).
        let extra: Vec<Point> = match valid {
            Some(magic_type::THRUSTING) => vec![loc.step(direction, 2)],
            Some(magic_type::HALF_MOON) | Some(magic_type::DRAGON_RISE) => vec![
                loc.step(direction.rotate(-1), 1),
                loc.step(direction.rotate(1), 1),
                loc.step(direction.rotate(2), 1),
            ],
            Some(magic_type::DESTRUCTIVE_SURGE) => {
                (1..8).map(|i| loc.step(direction.rotate(i), 1)).collect()
            }
            // Flame Splash: up to four of the other seven directions that hold a monster.
            Some(magic_type::FLAME_SPLASH) => {
                let mut dirs: Vec<Point> = (1..8)
                    .map(|i| loc.step(direction.rotate(i), 1))
                    .filter(|c| {
                        self.maps[&map]
                            .objects_at(*c)
                            .iter()
                            .any(|v| self.objects[v].is_monster() && !self.objects[v].dead)
                    })
                    .collect();
                let mut picked = Vec::new();
                while !dirs.is_empty() && picked.len() < 4 {
                    let i = self.rng.random_range(0..dirs.len());
                    picked.push(dirs.swap_remove(i));
                }
                picked
            }
            _ => Vec::new(),
        };
        for cell in extra {
            self.pending_hits.push(PendingHit {
                time: self.now + delay,
                attacker: id,
                target_cell: (map, cell),
                power,
                target: None,
                magics: magics.clone(),
                primary: false,
                raw: false,
            });
        }
        // Any swing breaks the cloak.
        if self.objects[&id].has_buff(buff_type::CLOAK) {
            self.buff_remove(id, buff_type::CLOAK);
        }
        // Lotus chain (Zircon `AttackLocationSuccess`): on a target in front,
        // the used lotus cools down and the next one in the chain opens.
        if let Some(m) = valid.filter(|m| magic_type::is_armed(*m)) {
            let hit_something = self.maps[&map]
                .objects_at(front)
                .iter()
                .any(|v| self.objects[v].is_monster() && !self.objects[v].dead);
            if hit_something {
                let swing = attack_delay(aspeed);
                let own = self
                    .data
                    .magics
                    .get(&m)
                    .map(|d| d.delay.max(0) as u64)
                    .unwrap_or(0);
                let locks: Vec<(u16, u64)> = match m {
                    magic_type::FULL_BLOOM => {
                        vec![(m, own), (magic_type::RED_LOTUS, swing * 3 / 2)]
                    }
                    magic_type::WHITE_LOTUS => {
                        vec![(magic_type::FULL_BLOOM, swing * 3 / 2), (m, own)]
                    }
                    magic_type::RED_LOTUS => vec![
                        (magic_type::FULL_BLOOM, swing * 3 / 2),
                        (magic_type::WHITE_LOTUS, swing * 3 / 2),
                        (m, own),
                    ],
                    magic_type::SWEET_BRIER => vec![
                        (magic_type::WHITE_LOTUS, swing * 3 / 2),
                        (magic_type::RED_LOTUS, swing * 3 / 2),
                        (m, own),
                    ],
                    // Karma: locks Summon Puppet too and stops item use for 10 s.
                    _ => {
                        if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
                            p.use_item_time = self.now + 10_000;
                        }
                        vec![(m, own), (magic_type::SUMMON_PUPPET, own)]
                    }
                };
                self.send_to(
                    id,
                    ServerMessage::MagicToggle {
                        magic: m,
                        on: false,
                    },
                );
                for (lm, ms) in locks {
                    if let Some(um) = self
                        .objects
                        .get_mut(&id)
                        .and_then(|o| o.player_mut())
                        .and_then(|p| p.magics.iter_mut().find(|x| x.magic == lm))
                    {
                        um.cooldown_until = self.now + ms;
                        self.send_to(
                            id,
                            ServerMessage::MagicCooldown {
                                magic: lm,
                                delay_ms: ms as u32,
                            },
                        );
                    }
                }
            }
        }
        if valid.is_some() {
            self.send_player_stats(id);
        }
    }

    pub(super) fn roll_dc(&mut self, s: CombatStats) -> i32 {
        if s.min_dc >= s.max_dc {
            s.max_dc
        } else {
            self.rng.random_range(s.min_dc..=s.max_dc)
        }
    }

    pub(super) fn roll_ac(&mut self, s: CombatStats) -> i32 {
        if s.min_ac >= s.max_ac {
            s.max_ac
        } else {
            self.rng.random_range(s.min_ac..=s.max_ac)
        }
    }

    // ---- combat resolution ------------------------------------------------

    pub(super) fn resolve_hits(&mut self) {
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
                if !self.objects[&hit.attacker].hostile_to(target) {
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
                if hit.raw {
                    // Blade Storm's second half: damage was settled already.
                    self.damage(tid, hit.attacker, hit.power, element::NONE, false);
                    continue;
                }
                let lotus = hit
                    .magics
                    .iter()
                    .copied()
                    .find(|m| magic_type::is_lotus(*m) || *m == magic_type::SWEET_BRIER);
                let karma = hit.magics.contains(&magic_type::KARMA);
                // Resolution: Karma swings gain accuracy and pierce armour by its power.
                let resolution_pct = if karma {
                    self.objects[&hit.attacker]
                        .player()
                        .and_then(|p| p.magics.iter().find(|m| m.magic == magic_type::RESOLUTION))
                        .map(|m| m.power_range(&self.data.magics[&m.magic]).0)
                        .unwrap_or(0)
                } else {
                    0
                };
                // Hit chance: Random.Next(Agility) > Accuracy => dodge (lotus never misses).
                let roll = if tstats.agility > 0 {
                    self.rng.random_range(0..tstats.agility)
                } else {
                    0
                };
                let sure_hit = lotus.is_some() || hit.magics.contains(&magic_type::SWIFT_BLADE);
                let accuracy =
                    attacker_stats.accuracy + attacker_stats.accuracy * resolution_pct / 100;
                if !sure_hit && roll > accuracy {
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
                        magic_type::FLAMING_SWORD
                        | magic_type::DRAGON_RISE
                        | magic_type::BLADE_STORM
                        | magic_type::SWIFT_BLADE => {
                            power = power * mp / 100;
                        }
                        magic_type::DESTRUCTIVE_SURGE | magic_type::FLAME_SPLASH
                            if !hit.primary =>
                        {
                            power = power * mp / 100;
                        }
                        magic_type::CALAMITY_OF_FULL_MOON | magic_type::WANING_MOON => power += mp,
                        magic_type::KARMA => {
                            power += self.roll_dc(attacker_stats);
                            // The percent-HP execute: quartered on monsters, flat on bosses.
                            let (thp, boss) = {
                                let t = &self.objects[&tid];
                                let boss = matches!(&t.kind, Kind::Monster(m) if self.data.monsters[&m.def].is_boss);
                                (t.hp, boss)
                            };
                            let execute = if boss { mp * 20 } else { thp * mp / 100 / 4 };
                            if execute > 0 {
                                self.damage(tid, hit.attacker, execute, element::NONE, false);
                            }
                            let tloc = self.objects[&tid].location;
                            self.events.push((
                                tid,
                                ServerMessage::ObjectEffect {
                                    id: tid,
                                    effect: effect::KARMA,
                                    location: tloc,
                                },
                            ));
                        }
                        m if magic_type::is_lotus(m) || m == magic_type::SWEET_BRIER => {
                            // Zircon lotus: 2x DC minus one AC roll, plus a mana-scaled
                            // bonus minus MR; the combo buff triples the bonus.
                            let (max_mp, max_dc, combo) = {
                                let a = &self.objects[&hit.attacker];
                                let p = a.player().unwrap();
                                let prev = match m {
                                    magic_type::WHITE_LOTUS => Some(buff_type::FULL_BLOOM),
                                    magic_type::RED_LOTUS => Some(buff_type::WHITE_LOTUS),
                                    magic_type::SWEET_BRIER => Some(buff_type::RED_LOTUS),
                                    _ => None,
                                };
                                (
                                    p.max_mp,
                                    a.stats.max_dc,
                                    prev.map(|b| a.has_buff(b)).unwrap_or(false),
                                )
                            };
                            let mut bonus = max_mp * mp / 1000;
                            let ac = self.roll_ac(tstats);
                            let dc = self.roll_dc(attacker_stats);
                            power = (power - ac + dc).max(0);
                            if combo {
                                bonus *= 3;
                                power += (max_dc - 100).max(0);
                            }
                            let mr = self.roll_range(tstats.min_mr, tstats.max_mr);
                            power += (bonus - mr).max(0);
                            let (next, fx) = match m {
                                magic_type::FULL_BLOOM => {
                                    (Some(buff_type::FULL_BLOOM), effect::FULL_BLOOM)
                                }
                                magic_type::WHITE_LOTUS => {
                                    (Some(buff_type::WHITE_LOTUS), effect::WHITE_LOTUS)
                                }
                                magic_type::RED_LOTUS => {
                                    (Some(buff_type::RED_LOTUS), effect::RED_LOTUS)
                                }
                                _ => (None, effect::SWEET_BRIER),
                            };
                            self.buff_remove(hit.attacker, buff_type::FULL_BLOOM);
                            self.buff_remove(hit.attacker, buff_type::WHITE_LOTUS);
                            self.buff_remove(hit.attacker, buff_type::RED_LOTUS);
                            if let Some(next) = next {
                                self.buff_add(hit.attacker, next, 15_000, BuffStats::default());
                            }
                            let tloc = self.objects[&tid].location;
                            self.events.push((
                                tid,
                                ServerMessage::ObjectEffect {
                                    id: tid,
                                    effect: fx,
                                    location: tloc,
                                },
                            ));
                        }
                        _ => {}
                    }
                }
                if lotus.is_none() {
                    let mut ac = self.roll_ac(tstats);
                    ac -= ac * resolution_pct / 100;
                    power -= ac;
                }
                if power <= 0 {
                    continue;
                }
                if attacker_is_player && self.rng.random_range(0..100) < 1 {
                    power *= 2; // CriticalChance 1, CriticalDamage 0
                }
                // Blade Storm: half now, half 300 ms later.
                if hit.magics.contains(&magic_type::BLADE_STORM) {
                    power /= 2;
                    self.pending_hits.push(PendingHit {
                        time: self.now + 300,
                        attacker: hit.attacker,
                        target_cell: hit.target_cell,
                        power,
                        target: Some(tid),
                        magics: Vec::new(),
                        primary: true,
                        raw: true,
                    });
                }
                let dealt = self.damage(tid, hit.attacker, power, element::NONE, false);
                if dealt > 0 && attacker_is_player {
                    // Bloody Flower life steal on the primary hit (cap 750, 1500 with a lotus).
                    let (pct, hp, max_hp) = {
                        let a = &self.objects[&hit.attacker];
                        (
                            a.player().map(|p| p.life_steal).unwrap_or(0),
                            a.hp,
                            a.max_hp,
                        )
                    };
                    if pct > 0 && hit.primary && hp < max_hp {
                        let cap = if lotus.is_some() { 1500 } else { 750 };
                        let heal = (dealt * pct / 100).min(cap);
                        if heal > 0 {
                            let a = self.objects.get_mut(&hit.attacker).unwrap();
                            a.hp = (a.hp + heal).min(a.max_hp);
                            let (hp, max_hp) = (a.hp, a.max_hp);
                            self.events.push((
                                hit.attacker,
                                ServerMessage::HealthChanged {
                                    id: hit.attacker,
                                    hp,
                                    max_hp,
                                },
                            ));
                        }
                    }
                    for m in hit.magics.clone() {
                        self.level_magic(hit.attacker, m);
                    }
                }
            }
        }
    }

    pub(super) fn damage(
        &mut self,
        target: ObjectId,
        attacker: ObjectId,
        power: i32,
        elem: u8,
        magic: bool,
    ) -> i32 {
        let now = self.now;
        let mut power = if self.objects[&target]
            .poisons
            .iter()
            .any(|p| p.kind == poison_kind::RED)
        {
            power * 12 / 10 // Red poison: +20 % damage taken
        } else {
            power
        };
        // Magic Shield: 25 ms of duration per point, then 50 % absorbed.
        if self.objects[&target].has_buff(buff_type::MAGIC_SHIELD) {
            let drain = power.max(0) as u64 * 25;
            let remaining = {
                let p = self.objects.get_mut(&target).unwrap().player_mut().unwrap();
                let b = p
                    .buffs
                    .iter_mut()
                    .find(|b| b.kind == buff_type::MAGIC_SHIELD)
                    .unwrap();
                b.expires = b.expires.saturating_sub(drain);
                b.expires.saturating_sub(now)
            };
            self.send_to(
                target,
                ServerMessage::BuffTime {
                    kind: buff_type::MAGIC_SHIELD,
                    remaining_ms: remaining,
                },
            );
            power -= power * 50 / 100;
        }
        // Reflect Damage: a monster's melee hit comes back at `reflect` %.
        let reflected = if !magic && self.objects[&attacker].is_monster() {
            self.objects[&target]
                .player()
                .and_then(|p| p.buffs.iter().find(|b| b.kind == buff_type::REFLECT_DAMAGE))
                .map(|b| power.max(0) * b.stats.reflect / 100)
                .filter(|r| *r > 0)
        } else {
            None
        };
        if let Some(r) = reflected {
            self.damage(attacker, target, r, element::NONE, false);
            self.level_magic(target, magic_type::REFLECT_DAMAGE);
        }
        if let Some(m) = self.objects.get_mut(&target).and_then(|o| o.monster_mut()) {
            if m.explode_at.is_some() {
                m.explode_at = Some(now);
            }
        }
        let credit = self.objects[&attacker].side().unwrap_or(attacker);
        // Idle pets of the attacker join in (Zircon `Pets[i].Target = ob`).
        if self.objects[&target].is_monster() {
            let pets: Vec<ObjectId> = self.objects[&attacker]
                .player()
                .map(|p| p.pets.clone())
                .unwrap_or_default();
            for pet in pets {
                if let Some(m) = self.objects.get_mut(&pet).and_then(|o| o.monster_mut()) {
                    if m.target.is_none() {
                        m.target = Some(target);
                    }
                }
            }
        }
        let (died, is_player, map, struck) = {
            let t = self.objects.get_mut(&target).unwrap();
            t.hp -= power;
            let mut struck = true;
            if let Kind::Monster(m) = &mut t.kind {
                if m.exp_owner.is_none() {
                    m.exp_owner = Some(credit);
                }
                if m.target.is_none() {
                    m.target = Some(attacker);
                }
                m.shock_until = 0;
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
        // Celestial Light: survive the killing blow once at a percent of max HP.
        let celestial = self.objects[&target]
            .player()
            .and_then(|p| {
                p.buffs
                    .iter()
                    .find(|b| b.kind == buff_type::CELESTIAL_LIGHT)
            })
            .map(|b| b.stats.celestial);
        if let (true, true, Some(pct)) = (died, is_player, celestial) {
            let o = self.objects.get_mut(&target).unwrap();
            o.hp = (o.max_hp * pct / 100).max(1);
            let (hp, max_hp) = (o.hp, o.max_hp);
            self.buff_remove(target, buff_type::CELESTIAL_LIGHT);
            if let Some(um) = self
                .objects
                .get_mut(&target)
                .and_then(|o| o.player_mut())
                .and_then(|p| {
                    p.magics
                        .iter_mut()
                        .find(|m| m.magic == magic_type::CELESTIAL_LIGHT)
                })
            {
                um.cooldown_until = now + 6000;
            }
            self.send_to(
                target,
                ServerMessage::MagicCooldown {
                    magic: magic_type::CELESTIAL_LIGHT,
                    delay_ms: 6000,
                },
            );
            self.events.push((
                target,
                ServerMessage::HealthChanged {
                    id: target,
                    hp,
                    max_hp,
                },
            ));
            return power;
        }
        if died {
            if is_player {
                self.player_die(target);
            } else {
                self.monster_die(target, attacker);
            }
        }
        power
    }

    pub(super) fn monster_die(&mut self, id: ObjectId, _killer: ObjectId) {
        let (exp, owner, spawn, pet_of) = {
            let o = self.objects.get_mut(&id).unwrap();
            o.dead = true;
            o.hp = 0;
            let m = o.monster_mut().unwrap();
            m.dead_time = self.now + DEAD_DURATION;
            m.target = None;
            (m.experience, m.exp_owner, m.spawn, m.owner)
        };
        if let Some(pet_of) = pet_of {
            // Pets yield no experience or drops.
            if let Some(p) = self.objects.get_mut(&pet_of).and_then(|o| o.player_mut()) {
                p.pets.retain(|x| *x != id);
            }
            self.events.push((id, ServerMessage::ObjectDie { id }));
            return;
        }
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

    pub(super) fn roll_range(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            max
        } else {
            self.rng.random_range(min..=max)
        }
    }
}
