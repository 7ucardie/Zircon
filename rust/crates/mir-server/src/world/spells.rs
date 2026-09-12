use super::*;

impl World {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn spawn_spell(
        &mut self,
        map: i32,
        location: Point,
        effect: u8,
        ticks: i32,
        frequency: u64,
        owner: ObjectId,
        magic: u16,
    ) {
        // Zircon: a new fire wall replaces any fire wall on the cell.
        let old: Vec<ObjectId> = self.maps[&map]
            .objects_at(location)
            .iter()
            .copied()
            .filter(|v| {
                matches!(&self.objects[v].kind, Kind::Spell(s)
                    if s.effect == effect
                        || matches!((s.effect, effect), (spell_effect::FIRE_WALL, spell_effect::TEMPEST) | (spell_effect::TEMPEST, spell_effect::FIRE_WALL)))
            })
            .collect();
        for v in old {
            self.remove_object(v);
        }
        let id = self.alloc_id();
        let obj = Object {
            id,
            kind: Kind::Spell(SpellData {
                effect,
                tick_count: ticks,
                tick_frequency: frequency,
                tick_time: 0,
                owner,
                magic,
            }),
            map,
            location,
            direction: Direction::Up,
            hp: 0,
            max_hp: 0,
            dead: false,
            stats: CombatStats::ZERO,
            action_time: 0,
            move_time: 0,
            attack_time: 0,
            cell_time: 0,
            appearance: Appearance::Spell { effect },
            visible: HashSet::new(),
            poisons: Vec::new(),
            heal: None,
        };
        self.insert_object(obj);
    }

    /// Zircon `SpellObject.Process`: tick every `frequency` until the count
    /// runs out; despawn when the owner is gone.
    pub(super) fn process_spells(&mut self) {
        let now = self.now;
        let spells: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| o.is_spell())
            .map(|o| o.id)
            .collect();
        for sid in spells {
            let (effect, owner, map, loc, magic, due, done) = {
                let o = &self.objects[&sid];
                let Kind::Spell(s) = &o.kind else { continue };
                (
                    s.effect,
                    s.owner,
                    o.map,
                    o.location,
                    s.magic,
                    now >= s.tick_time,
                    s.tick_count <= 0,
                )
            };
            let owner_gone = self.objects.get(&owner).map(|o| o.dead).unwrap_or(true);
            if owner_gone {
                self.remove_object(sid);
                continue;
            }
            if !due {
                continue;
            }
            if done {
                if effect == spell_effect::FIRE_WALL {
                    self.events.push((
                        sid,
                        ServerMessage::MapEffect {
                            location: loc,
                            effect: effect::FIRE_WALL_SMOKE,
                        },
                    ));
                }
                self.remove_object(sid);
                continue;
            }
            {
                let o = self.objects.get_mut(&sid).unwrap();
                let Kind::Spell(s) = &mut o.kind else {
                    continue;
                };
                s.tick_count -= 1;
                s.tick_time = now + s.tick_frequency;
            }
            match effect {
                spell_effect::FIRE_WALL => {
                    let victims: Vec<ObjectId> = self.maps[&map]
                        .objects_at(loc)
                        .iter()
                        .copied()
                        .filter(|v| self.objects[v].is_monster() && !self.objects[v].dead)
                        .collect();
                    for v in victims {
                        self.magic_attack(owner, v, magic, element::FIRE, 60);
                    }
                }
                spell_effect::TEMPEST => {
                    let victims: Vec<ObjectId> = self.maps[&map]
                        .objects_at(loc)
                        .iter()
                        .copied()
                        .filter(|v| self.objects[v].is_monster() && !self.objects[v].dead)
                        .collect();
                    for v in victims {
                        if self.magic_attack(owner, v, magic, element::WIND, 80) > 0 {
                            self.try_repel(v, owner, 5);
                        }
                    }
                }
                spell_effect::POISONOUS_CLOUD => {
                    // Everyone within 2 cells gains +5 agility while it lasts.
                    let until = {
                        let Kind::Spell(s) = &self.objects[&sid].kind else {
                            continue;
                        };
                        s.tick_time
                    };
                    let players: Vec<ObjectId> = self
                        .objects
                        .values()
                        .filter(|o| o.is_player() && o.map == map && o.location.distance(loc) <= 2)
                        .map(|o| o.id)
                        .collect();
                    for pid in players {
                        let stats = BuffStats {
                            agility: 5,
                            ..BuffStats::default()
                        };
                        self.buff_add(
                            pid,
                            buff_type::POISONOUS_CLOUD,
                            until.saturating_sub(now),
                            stats,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    /// Cells of Zircon's line spells: 8 steps ahead, each with two flanks.
    pub(super) fn line_cells(loc: Point, dir: Direction) -> Vec<(Point, bool)> {
        let mut out = Vec::with_capacity(24);
        let cardinal = matches!(
            dir,
            Direction::Up | Direction::Right | Direction::Down | Direction::Left
        );
        for i in 1..=8 {
            let c = loc.step(dir, i);
            out.push((c, true));
            let (a, b) = if cardinal { (-2, 2) } else { (1, -1) };
            out.push((c.step(dir.rotate(a), 1), false));
            out.push((c.step(dir.rotate(b), 1), false));
        }
        out
    }
}
