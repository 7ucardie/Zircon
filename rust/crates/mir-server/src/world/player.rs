use super::*;

impl World {
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
                buffs: Vec::new(),
                charge: None,
                surge_on: false,
                dash: None,
                life_steal: 0,
                pets: Vec::new(),
                full_moon_ready: false,
                waning_moon_ready: false,
                flame_splash_on: false,
                currencies: rec.currencies.iter().copied().collect(),
                rebirth: rec.rebirth,
                npc_roll: None,
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
                    m.class & (1 << class) != 0
                        && m.school != 0
                        && m.school != 20
                        && m.need_level[0] <= level
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

    pub(super) fn send_belt(&mut self, id: ObjectId) {
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
        rec.currencies = p.currencies.iter().map(|(k, v)| (*k, *v)).collect();
        rec.rebirth = p.rebirth;
        if let Some(m) = self.maps.get(&o.map) {
            rec.map = m.descriptor.file.clone();
            rec.location = o.location;
        }
    }

    pub(super) fn player_stats(&self, o: &Object) -> PlayerStats {
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
            attack_speed: o.player().map(|p| p.attack_speed as i32).unwrap_or(0),
        }
    }

    pub(super) fn gain_experience(&mut self, id: ObjectId, amount: u64) {
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

    pub(super) fn player_die(&mut self, id: ObjectId) {
        let o = self.objects.get_mut(&id).unwrap();
        o.dead = true;
        o.hp = 0;
        let p = o.player_mut().unwrap();
        p.revive_time = self.now + REVIVE_DELAY;
        let pets = std::mem::take(&mut p.pets);
        self.events.push((id, ServerMessage::ObjectDie { id }));
        for pet in pets {
            if self.objects.get(&pet).map(|o| !o.dead).unwrap_or(false) {
                self.monster_die(pet, id);
            }
        }
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

    pub(super) fn revive_player(&mut self, id: ObjectId) -> anyhow::Result<()> {
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

    // ---- stats, inventory, equipment --------------------------------------

    /// Zircon `RefreshStats`: base stats for class/level plus equipped items.
    pub(super) fn refresh_stats(&mut self, id: ObjectId, restore: bool) {
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
        let mut pas_min_dc = 0;
        let mut life_steal = 0;
        let mut pas_mc_pct = 0;
        for m in &p.magics {
            let Some(def) = self.data.magics.get(&m.magic) else {
                continue;
            };
            if p.level < def.need_level[0] {
                continue;
            }
            let (pmin, _) = m.power_range(def);
            match m.magic {
                magic_type::SWORDSMANSHIP | magic_type::SPIRIT_SWORD => pas_acc += pmin,
                magic_type::WILLOW_DANCE => pas_agi += pmin,
                magic_type::SLAYING => {
                    pas_acc += m.level as i32 * 2;
                    pas_dc += m.level as i32 * 2;
                }
                // Zircon rolls GetPower() on every refresh; we use the minimum.
                magic_type::VINE_TREE_DANCE => pas_acc += pmin,
                magic_type::DISCIPLINE => {
                    pas_acc += pmin / 3;
                    pas_min_dc += pmin;
                }
                magic_type::BLOODY_FLOWER => life_steal += pmin,
                // Zircon quirk: owning Renounce grants its MC percent passively.
                magic_type::RENOUNCE => pas_mc_pct += (m.level as i32 + 1) * 10,
                _ => {}
            }
        }
        // Buff stats: flat first, then the summed percents once.
        let mut bs = BuffStats::default();
        for b in &p.buffs {
            bs.dc_pct += b.stats.dc_pct;
            bs.phys_def_pct += b.stats.phys_def_pct;
            bs.mag_def_pct += b.stats.mag_def_pct;
            bs.max_ac += b.stats.max_ac;
            bs.max_mr += b.stats.max_mr;
            bs.agility += b.stats.agility;
            bs.hp_pct += b.stats.hp_pct;
            bs.mc_pct += b.stats.mc_pct;
            bs.max_dc += b.stats.max_dc;
            bs.max_mc += b.stats.max_mc;
            bs.max_sc += b.stats.max_sc;
        }
        bs.mc_pct += pas_mc_pct;
        let o = self.objects.get_mut(&id).unwrap();
        o.max_hp = base.health + g(stat::HEALTH);
        o.max_hp += o.max_hp * bs.hp_pct / 100;
        o.max_hp = o.max_hp.max(10);
        let mut s = CombatStats {
            accuracy: base.accuracy + g(stat::ACCURACY) + pas_acc,
            agility: base.agility + g(stat::AGILITY) + pas_agi + bs.agility,
            min_ac: base.min_ac + g(stat::MIN_AC),
            max_ac: base.max_ac + g(stat::MAX_AC) + bs.max_ac,
            min_dc: base.min_dc + g(stat::MIN_DC) + pas_dc + pas_min_dc,
            max_dc: base.max_dc + g(stat::MAX_DC) + pas_dc + bs.max_dc,
            min_mr: base.min_mr + g(stat::MIN_MR),
            max_mr: base.max_mr + g(stat::MAX_MR) + bs.max_mr,
            min_mc: base.min_mc + g(stat::MIN_MC),
            max_mc: base.max_mc + g(stat::MAX_MC) + bs.max_mc,
            min_sc: base.min_sc + g(stat::MIN_SC),
            max_sc: base.max_sc + g(stat::MAX_SC) + bs.max_sc,
        };
        s.min_dc += s.min_dc * bs.dc_pct / 100;
        s.max_dc += s.max_dc * bs.dc_pct / 100;
        s.min_mc += s.min_mc * bs.mc_pct / 100;
        s.max_mc += s.max_mc * bs.mc_pct / 100;
        s.min_ac += s.min_ac * bs.phys_def_pct / 100;
        s.max_ac += s.max_ac * bs.phys_def_pct / 100;
        s.min_mr += s.min_mr * bs.mag_def_pct / 100;
        s.max_mr += s.max_mr * bs.mag_def_pct / 100;
        s.min_ac = s.min_ac.max(0);
        s.max_ac = s.max_ac.max(0);
        s.min_mr = s.min_mr.max(0);
        s.max_mr = s.max_mr.max(0);
        s.min_dc = s.min_dc.max(0).min(s.max_dc.max(0));
        s.max_dc = s.max_dc.max(0);
        o.stats = s;
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
        p.life_steal = life_steal;
        p.max_bag = base.bag_weight + g(73);
        p.max_wear = base.wear_weight + g(74);
        p.max_hand = base.hand_weight + g(75);
    }

    pub(super) fn weights_of(&self, id: ObjectId) -> Option<Weights> {
        let p = self.objects.get(&id)?.player()?;
        Some(p.bag.weights(&self.data, p.max_bag, p.max_wear, p.max_hand))
    }

    pub(super) fn send_inventory(&mut self, id: ObjectId) {
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

    pub(super) fn send_changes(&mut self, id: ObjectId, changes: Changed) {
        for (grid, slot, item) in changes {
            self.send_to(id, ServerMessage::ItemChanged { grid, slot, item });
        }
        if let Some(w) = self.weights_of(id) {
            self.send_to(id, ServerMessage::WeightsChanged(w));
        }
    }

    pub(super) fn send_gold(&mut self, id: ObjectId) {
        if let Some(gold) = self
            .objects
            .get(&id)
            .and_then(|o| o.player())
            .map(|p| p.bag.gold)
        {
            self.send_to(id, ServerMessage::GoldChanged { gold });
        }
    }

    pub(super) fn send_player_stats(&mut self, id: ObjectId) {
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
    pub(super) fn refresh_appearance(&mut self, id: ObjectId) {
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
    pub(super) fn give_start_items(&mut self, id: ObjectId) {
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

    // ---- magic ----------------------------------------------------------------------

    pub(super) fn send_magics(&mut self, id: ObjectId) {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let list = p.magics.iter().map(|m| m.summary()).collect();
        self.send_to(id, ServerMessage::Magics(list));
    }

    /// Zircon `Resurrection`: back on the same cell at a percent of HP/MP.
    pub(super) fn revive_in_place(&mut self, id: ObjectId, pct: i32) {
        let (hp, dir, location) = {
            let Some(o) = self.objects.get_mut(&id) else {
                return;
            };
            o.dead = false;
            o.hp = (o.max_hp * pct / 100).max(1);
            if let Some(p) = o.player_mut() {
                p.mp = (p.max_mp * pct / 100).max(1);
                p.revive_time = 0;
            }
            (o.hp, o.direction, o.location)
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
    }
}
