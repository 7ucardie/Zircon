//! Companions (Zircon `Companion`, `UserCompanion`, `CompanionInfo`): a pet
//! that follows its owner, picks up the owner's drops into its own bag,
//! gains a level over time and gets hungry. Adopted at a CompanionManage
//! page for the companion's price, stored and retrieved there.

use serde::{Deserialize, Serialize};

use super::*;
use crate::items::UserItem;
use mir_proto::{ChatKind, CompanionOffer, CompanionSummary};

/// Zircon `Globals.CompanionInventorySize`, `Stat.CompanionHunger` = 86,
/// companion buff tick 1 minute, `Config.CompanionRate` 0.
pub const COMPANION_BAG_SIZE: usize = 30;
pub const COMPANION_HUNGER_STAT: i32 = 86;
pub const COMPANION_TICK: u64 = 60_000;
/// How far a companion looks for the owner's drops.
const PICKUP_RANGE: i32 = 8;

/// Zircon `UserCompanion`, persisted per character.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCompanion {
    pub index: u32,
    /// `CompanionInfo` index.
    pub info: i32,
    pub name: String,
    pub level: i32,
    pub experience: i32,
    pub hunger: i32,
    pub items: Vec<UserItem>,
}

impl World {
    fn companion_line(&mut self, id: ObjectId, text: String) {
        self.send_to(
            id,
            ServerMessage::Say {
                id: None,
                kind: ChatKind::System,
                text,
            },
        );
    }

    fn level_def(&self, level: i32) -> Option<&crate::data::CompanionLevelDef> {
        self.data
            .companion_levels
            .iter()
            .filter(|l| l.level <= level)
            .max_by_key(|l| l.level)
    }

    /// Bag slots and weight allowed at a level (Zircon `CompanionLevelInfo`).
    fn bag_limits(&self, level: i32) -> (usize, i32) {
        self.level_def(level)
            .map(|l| {
                (
                    l.inventory_space.clamp(0, COMPANION_BAG_SIZE as i32) as usize,
                    l.inventory_weight,
                )
            })
            .unwrap_or((5, 100))
    }

    pub(super) fn send_companions(&mut self, id: ObjectId) {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let list: Vec<CompanionSummary> = p
            .companions
            .iter()
            .map(|c| {
                let (bag_size, max_weight) = self.bag_limits(c.level);
                CompanionSummary {
                    index: c.index,
                    kind: self
                        .data
                        .companions
                        .iter()
                        .find(|d| d.index == c.info)
                        .and_then(|d| self.data.monsters.get(&d.monster))
                        .map(|m| m.name.clone())
                        .unwrap_or_default(),
                    name: c.name.clone(),
                    level: c.level,
                    experience: c.experience,
                    max_experience: self
                        .level_def(c.level)
                        .map(|l| l.max_experience)
                        .unwrap_or(0),
                    hunger: c.hunger,
                    active: p.active_companion == Some(c.index),
                    items: c.items.iter().map(|i| i.instance()).collect(),
                    bag_size: bag_size as u32,
                    bag_weight: c
                        .items
                        .iter()
                        .map(|i| crate::items::item_weight(&self.data, i))
                        .sum(),
                    max_weight,
                }
            })
            .collect();
        self.send_to(id, ServerMessage::Companions(list));
    }

    /// The companion shop for a CompanionManage page.
    pub(super) fn send_companion_shop(&mut self, id: ObjectId) {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let offers: Vec<CompanionOffer> = self
            .data
            .companions
            .iter()
            .map(|d| CompanionOffer {
                index: d.index,
                name: self
                    .data
                    .monsters
                    .get(&d.monster)
                    .map(|m| m.name.clone())
                    .unwrap_or_default(),
                description: d.description.clone(),
                price: d.price,
                currency: self
                    .data
                    .currencies
                    .iter()
                    .find(|c| c.index == d.currency)
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| "Gold".into()),
                unlocked: d.available || p.companion_unlocks.contains(&d.index),
                unlock_item: d.unlock_item,
            })
            .collect();
        self.send_to(id, ServerMessage::CompanionShop(offers));
    }

    /// Spawn the active companion beside its owner (on entry / retrieve).
    pub(super) fn companion_spawn(&mut self, id: ObjectId) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let Some(p) = o.player() else {
            return;
        };
        if p.companion.is_some() {
            return;
        }
        let Some(active) = p.active_companion else {
            return;
        };
        let Some(c) = p.companions.iter().find(|c| c.index == active) else {
            return;
        };
        let Some(def) = self.data.companions.iter().find(|d| d.index == c.info) else {
            return;
        };
        if !self.data.monsters.contains_key(&def.monster) {
            return;
        }
        let (map, loc, monster, name) = (o.map, o.location, def.monster, c.name.clone());
        let cid = self.create_monster(monster, map, loc, None, Some(id), 0);
        if let Some(m) = self.objects.get_mut(&cid).and_then(|o| o.monster_mut()) {
            m.companion = Some(active);
            m.tame_until = u64::MAX;
        }
        if let Some(o) = self.objects.get_mut(&cid) {
            if let Appearance::Monster { name: n, .. } = &mut o.appearance {
                *n = name;
            }
        }
        if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
            p.companion = Some(cid);
            p.companion_tick = self.now + COMPANION_TICK;
        }
    }

    pub(super) fn companion_despawn(&mut self, id: ObjectId) {
        let cid = self
            .objects
            .get_mut(&id)
            .and_then(|o| o.player_mut())
            .and_then(|p| p.companion.take());
        if let Some(cid) = cid {
            self.remove_object(cid);
        }
    }

    /// Unlock a locked companion look with its unlock item (Zircon
    /// `CompanionUnlock`).
    pub fn companion_unlock(&mut self, id: ObjectId, index: i32) {
        let Some(def) = self
            .data
            .companions
            .iter()
            .find(|d| d.index == index)
            .cloned()
        else {
            return;
        };
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        if def.available || p.companion_unlocks.contains(&index) {
            self.companion_line(id, "That companion is already available to you.".into());
            return;
        }
        let Some(slot) = p
            .bag
            .inventory
            .iter()
            .position(|s| s.as_ref().is_some_and(|i| i.info == def.unlock_item))
        else {
            self.companion_line(id, "You do not have the item that unlocks it.".into());
            return;
        };
        let p = self
            .objects
            .get_mut(&id)
            .and_then(|o| o.player_mut())
            .unwrap();
        let change = p.bag.take(Grid::Inventory, slot as u8, 1);
        p.companion_unlocks.push(index);
        if let Some(ch) = change {
            self.send_changes(id, vec![ch]);
        }
        self.send_companion_shop(id);
    }

    /// Adopt a companion for its price (Zircon `CompanionAdopt`).
    pub fn companion_adopt(&mut self, id: ObjectId, index: i32, name: String) {
        if self.npc_dialog_type(id) != Some(5) {
            return;
        }
        let Some(def) = self
            .data
            .companions
            .iter()
            .find(|d| d.index == index)
            .cloned()
        else {
            return;
        };
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        if !def.available && !p.companion_unlocks.contains(&index) {
            self.companion_line(id, "That companion is not available to you.".into());
            return;
        }
        let valid_name = (2..=15).contains(&name.chars().count())
            && name.chars().all(|c| c.is_ascii_alphanumeric());
        if !valid_name {
            self.companion_line(id, "Companion names are 2 to 15 letters or digits.".into());
            return;
        }
        let gold =
            def.currency == 0 || !self.data.currencies.iter().any(|c| c.index == def.currency);
        let price = def.price.max(0) as u64;
        if gold {
            if p.bag.gold < price {
                self.companion_line(id, format!("Adopting costs {price} gold."));
                return;
            }
        } else if p.currencies.get(&def.currency).copied().unwrap_or(0) < price as i64 {
            self.companion_line(id, "You cannot afford that companion.".into());
            return;
        }
        let max_hunger = self.level_def(1).map(|l| l.max_hunger).unwrap_or(100);
        let p = self
            .objects
            .get_mut(&id)
            .and_then(|o| o.player_mut())
            .unwrap();
        if gold {
            p.bag.gold -= price;
        } else {
            *p.currencies.entry(def.currency).or_insert(0) -= price as i64;
        }
        let cindex = p.companions.iter().map(|c| c.index).max().unwrap_or(0) + 1;
        p.companions.push(StoredCompanion {
            index: cindex,
            info: index,
            name: name.clone(),
            level: 1,
            experience: 0,
            hunger: max_hunger,
            items: Vec::new(),
        });
        self.send_gold(id);
        self.send_currencies(id);
        self.send_companions(id);
        self.companion_line(id, format!("{name} is yours."));
    }

    /// Bring a stored companion out (Zircon `CompanionRetrieve`).
    pub fn companion_retrieve(&mut self, id: ObjectId, index: u32) {
        if self.npc_dialog_type(id) != Some(5) {
            return;
        }
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        if !p.companions.iter().any(|c| c.index == index) {
            return;
        }
        self.companion_despawn(id);
        self.objects
            .get_mut(&id)
            .and_then(|o| o.player_mut())
            .unwrap()
            .active_companion = Some(index);
        self.companion_spawn(id);
        self.send_companions(id);
    }

    /// Put the active companion away (Zircon `CompanionStore`).
    pub fn companion_store(&mut self, id: ObjectId) {
        if self.npc_dialog_type(id) != Some(5) {
            return;
        }
        self.companion_despawn(id);
        if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
            p.active_companion = None;
        }
        self.send_companions(id);
    }

    /// Let a companion go for good (its bag must be empty).
    pub fn companion_release(&mut self, id: ObjectId, index: u32) {
        if self.npc_dialog_type(id) != Some(5) {
            return;
        }
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let Some(c) = p.companions.iter().find(|c| c.index == index) else {
            return;
        };
        if !c.items.is_empty() {
            self.companion_line(id, "Empty the companion's bag first.".into());
            return;
        }
        if p.active_companion == Some(index) {
            self.companion_despawn(id);
        }
        let p = self
            .objects
            .get_mut(&id)
            .and_then(|o| o.player_mut())
            .unwrap();
        if p.active_companion == Some(index) {
            p.active_companion = None;
        }
        p.companions.retain(|c| c.index != index);
        self.send_companions(id);
    }

    /// Move one item from a companion's bag into the player's bag.
    pub fn companion_bag_take(&mut self, id: ObjectId, index: u32, slot: u8) {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let Some(item) = p
            .companions
            .iter()
            .find(|c| c.index == index)
            .and_then(|c| c.items.get(slot as usize))
            .cloned()
        else {
            return;
        };
        if !p.bag.can_gain(&self.data, item.info, item.count, p.max_bag) {
            self.companion_line(id, "You cannot carry that.".into());
            return;
        }
        let p = self
            .objects
            .get_mut(&id)
            .and_then(|o| o.player_mut())
            .unwrap();
        let mut next = p.next_item_id;
        let changes = p.bag.gain(&self.data, item.info, item.count, &mut next);
        p.next_item_id = next;
        if let Some(c) = p.companions.iter_mut().find(|c| c.index == index) {
            c.items.remove(slot as usize);
        }
        self.send_changes(id, changes);
        self.send_companions(id);
    }

    /// Per-minute companion upkeep (Zircon companion buff tick): hunger
    /// drops outside safe zones, experience grows, levels advance.
    pub(super) fn process_companions(&mut self) {
        let now = self.now;
        let ids: Vec<ObjectId> = self
            .players()
            .filter(|o| {
                o.player()
                    .is_some_and(|p| p.companion.is_some() && now >= p.companion_tick)
            })
            .map(|o| o.id)
            .collect();
        for id in ids {
            let (in_safe_zone, active, highest) = {
                let o = &self.objects[&id];
                let p = o.player().unwrap();
                (
                    o.in_safe_zone,
                    p.active_companion,
                    p.companions.iter().map(|c| c.level).max().unwrap_or(1),
                )
            };
            let Some(active) = active else { continue };
            let level = self.objects[&id]
                .player()
                .unwrap()
                .companions
                .iter()
                .find(|c| c.index == active)
                .map(|c| c.level)
                .unwrap_or(1);
            let max_exp = self.level_def(level).map(|l| l.max_experience).unwrap_or(0);
            let p = self
                .objects
                .get_mut(&id)
                .and_then(|o| o.player_mut())
                .unwrap();
            p.companion_tick = now + COMPANION_TICK;
            let Some(c) = p.companions.iter_mut().find(|c| c.index == active) else {
                continue;
            };
            if !in_safe_zone || c.level < 15 {
                c.hunger = (c.hunger - 1).max(0);
            }
            if max_exp > 0 {
                let gain = if highest <= c.level { 1 } else { highest };
                c.experience += gain;
                if c.experience >= max_exp {
                    c.experience = 0;
                    c.level += 1;
                }
            }
            self.send_companions(id);
        }
    }

    /// Feed the active companion (Zircon: consumables with `CompanionHunger`).
    pub(super) fn companion_feed(&mut self, id: ObjectId, hunger: i32) -> bool {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return false;
        };
        let Some(active) = p.active_companion else {
            return false;
        };
        let Some(level) = p
            .companions
            .iter()
            .find(|c| c.index == active)
            .map(|c| c.level)
        else {
            return false;
        };
        let max = self.level_def(level).map(|l| l.max_hunger).unwrap_or(100);
        let p = self
            .objects
            .get_mut(&id)
            .and_then(|o| o.player_mut())
            .unwrap();
        let c = p.companions.iter_mut().find(|c| c.index == active).unwrap();
        if c.hunger >= max {
            return false;
        }
        c.hunger = (c.hunger + hunger).min(max);
        self.send_companions(id);
        true
    }

    /// Companion AI: stay near the owner and collect the owner's drops.
    pub(super) fn process_companion(&mut self, id: ObjectId, owner: ObjectId) {
        let now = self.now;
        let Some(o) = self.objects.get(&owner).filter(|o| o.is_player()) else {
            self.remove_object(id);
            return;
        };
        let (omap, oloc, odir, account) =
            (o.map, o.location, o.direction, o.player().unwrap().account);
        let cindex = self.objects[&id].monster_ref().companion.unwrap_or(0);
        let (map, loc) = (self.objects[&id].map, self.objects[&id].location);
        // Out of reach: appear behind the owner.
        if omap != map || oloc.distance(loc) > MAX_VIEW_RANGE {
            let behind = oloc.step(odir.rotate(4), 1);
            let to = if self.maps[&omap].file.is_walkable(behind.x, behind.y)
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
            return;
        }
        if now < self.objects[&id].move_time {
            return;
        }
        // Hungry companions only follow.
        let (hungry, level, count, weight) = self.objects[&owner]
            .player()
            .unwrap()
            .companions
            .iter()
            .find(|c| c.index == cindex)
            .map(|c| {
                (
                    c.hunger <= 0,
                    c.level,
                    c.items.len(),
                    c.items
                        .iter()
                        .map(|i| crate::items::item_weight(&self.data, i))
                        .sum::<i32>(),
                )
            })
            .unwrap_or((true, 1, 0, 0));
        let (bag_size, max_weight) = self.bag_limits(level);
        if !hungry {
            let gold_item = self.data.gold_item;
            let mut best: Option<(ObjectId, i32)> = None;
            for other in self.on_map(map) {
                let Kind::Item(it) = &other.kind else {
                    continue;
                };
                if it.owner != Some(account) {
                    continue;
                }
                let d = other.location.distance(loc);
                if d > PICKUP_RANGE {
                    continue;
                }
                let is_gold = it.item.info == gold_item;
                if !is_gold
                    && (count >= bag_size
                        || weight + crate::items::item_weight(&self.data, &it.item) > max_weight)
                {
                    continue;
                }
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((other.id, d));
                }
            }
            if let Some((target, d)) = best {
                if d <= 1 {
                    let item = match &self.objects[&target].kind {
                        Kind::Item(it) => it.item.clone(),
                        _ => return,
                    };
                    self.remove_object(target);
                    if item.info == gold_item {
                        let gold = self.guild_tax_gold(owner, item.count as u64);
                        let p = self
                            .objects
                            .get_mut(&owner)
                            .and_then(|o| o.player_mut())
                            .unwrap();
                        p.bag.gold += gold;
                        self.send_gold(owner);
                    } else {
                        let p = self
                            .objects
                            .get_mut(&owner)
                            .and_then(|o| o.player_mut())
                            .unwrap();
                        if let Some(c) = p.companions.iter_mut().find(|c| c.index == cindex) {
                            c.items.push(item);
                        }
                        self.send_companions(owner);
                    }
                } else {
                    let goal = self.objects[&target].location;
                    self.move_toward(id, goal, false);
                }
                return;
            }
        }
        // Idle: keep within two cells of the owner.
        if oloc.distance(loc) > 2 {
            self.move_toward(id, oloc, false);
        }
    }
}
