use super::*;

impl World {
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

    pub(super) fn item_move_inner(
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

    pub(super) fn item_use_inner(&mut self, id: ObjectId, slot: u8) -> Result<Changed, String> {
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
    pub(super) fn use_consumable(
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
                self.teleport_with_effects(id, to);
            }
            _ => return Err(format!("{} cannot be used", def.name)),
        }
        let o = self.objects.get_mut(&id).unwrap();
        let p = o.player_mut().unwrap();
        p.use_item_time = self.now + def.durability.max(0) as u64;
        let change = p.bag.take(Grid::Inventory, slot, 1);
        Ok(change.into_iter().collect())
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

    /// Zircon book use: learn the magic the book's `Shape` points at.
    pub(super) fn learn_book(
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
        if magic.class & (1 << p.class.mir_class()) == 0 {
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
}
