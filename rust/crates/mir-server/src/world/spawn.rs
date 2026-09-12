use super::*;

impl World {
    pub(super) fn spawn_monster(&mut self, map: i32, group: usize) -> bool {
        let (def_index, points) = {
            let g = &self.maps[&map].spawns[group];
            (g.def.monster, g.points.clone())
        };
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
        self.create_monster(def_index, map, location, Some((map, group)), None, 0);
        self.maps.get_mut(&map).unwrap().spawns[group].alive += 1;
        true
    }

    /// Put a monster of `def` on the map; pets carry an owner and a summon level.
    pub(super) fn create_monster(
        &mut self,
        def_index: i32,
        map: i32,
        location: Point,
        spawn: Option<(i32, usize)>,
        owner: Option<ObjectId>,
        summon_level: i32,
    ) -> ObjectId {
        let def: MonsterDef = self.data.monsters[&def_index].clone();
        let id = self.alloc_id();
        let dir = Direction::from_index(self.rng.random_range(0..8));
        let jitter_search = self.rng.random_range(0..SEARCH_DELAY);
        let jitter_roam = self.rng.random_range(0..ROAM_DELAY);
        let owner_name = owner
            .and_then(|o| self.objects.get(&o))
            .and_then(|o| o.player())
            .map(|p| p.name.clone());
        let obj = Object {
            id,
            kind: Kind::Monster(MonsterData {
                def: def.index,
                spawn,
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
                owner,
                shock_until: 0,
                summon_level,
                tame_until: if owner.is_some() { u64::MAX } else { 0 },
            }),
            map,
            location,
            direction: dir,
            hp: 1,
            max_hp: 1,
            dead: false,
            stats: CombatStats::ZERO,
            action_time: 0,
            move_time: 0,
            attack_time: 0,
            cell_time: 0,
            appearance: Appearance::Monster {
                name: def.name.clone(),
                image: def.image,
                owner: owner_name,
            },
            visible: HashSet::new(),
            poisons: Vec::new(),
            heal: None,
        };
        self.insert_object(obj);
        self.refresh_monster_stats(id, true);
        if let Some(o) = owner {
            if let Some(p) = self.objects.get_mut(&o).and_then(|o| o.player_mut()) {
                p.pets.push(id);
            }
        }
        id
    }

    /// Zircon `MonsterObject.RefreshStats`: base stats, +10 % per summon
    /// level, and the owner's pet DC percent.
    pub(super) fn refresh_monster_stats(&mut self, id: ObjectId, restore: bool) {
        let Some((def_index, summon_level, owner)) =
            self.objects.get(&id).and_then(|o| match &o.kind {
                Kind::Monster(m) => Some((m.def, m.summon_level, m.owner)),
                _ => None,
            })
        else {
            return;
        };
        let def = &self.data.monsters[&def_index];
        let boost = |v: i32| v + v * summon_level / 10;
        let pet_dc = owner
            .and_then(|o| self.objects.get(&o))
            .and_then(|o| o.player())
            .map(|p| p.buffs.iter().map(|b| b.stats.pet_dc_pct).sum::<i32>())
            .unwrap_or(0);
        let mut stats = CombatStats {
            accuracy: boost(def.stat(stat::ACCURACY)),
            agility: boost(def.stat(stat::AGILITY)),
            min_ac: boost(def.stat(stat::MIN_AC)),
            max_ac: boost(def.stat(stat::MAX_AC)),
            min_dc: boost(def.stat(stat::MIN_DC)),
            max_dc: boost(def.stat(stat::MAX_DC)),
            min_mr: boost(def.stat(stat::MIN_MR)),
            max_mr: boost(def.stat(stat::MAX_MR)),
            min_mc: 0,
            max_mc: 0,
            min_sc: 0,
            max_sc: 0,
        };
        stats.min_dc += stats.min_dc * pet_dc / 100;
        stats.max_dc += stats.max_dc * pet_dc / 100;
        let max_hp = boost(def.health());
        let o = self.objects.get_mut(&id).unwrap();
        o.stats = stats;
        o.max_hp = max_hp;
        o.hp = if restore { max_hp } else { o.hp.min(max_hp) };
    }

    /// Zircon `SpawnInfo.DoSpawn`, run once per second per map.
    pub(super) fn do_spawns(&mut self, map: i32) {
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

    pub(super) fn spawn_ground_item(
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
    pub(super) fn drop_loot(&mut self, monster: ObjectId, killer: Option<ObjectId>) {
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

    pub(super) fn spawn_npcs(&mut self, map: i32) {
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
}
