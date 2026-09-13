//! Weapon refining (Zircon `PlayerObject.NPC.NPCRefine` / `NPCRefineRetrieve`):
//! at a Refine page the equipped weapon goes into the furnace with black
//! iron ore, common jewellery and an optional special for 50,000 gold; the
//! chance depends on ore durability, jewellery level and quality; the
//! weapon comes back at a RefineRetrieve page after the quality's wait,
//! improved on a successful roll.

use serde::{Deserialize, Serialize};

use super::*;
use crate::items::UserItem;
use mir_formats::mirdb::stat;
use mir_proto::{refine_quality, refine_type, slot, ChatKind, RefineSummary};

/// Zircon: `RefineCost` 50,000 gold; `ItemEffect.BlackIronOre` = 20;
/// `ItemType.RefineSpecial` = 17; `Stat.MaxRefineChance` = 81.
pub const REFINE_COST: u64 = 50_000;
pub const BLACK_IRON_ORE_EFFECT: u8 = 20;
pub const REFINE_SPECIAL_TYPE: u8 = 17;
pub const MAX_REFINE_CHANCE_STAT: i32 = 81;
/// Zircon `Stat.WeaponElement` and the first elemental attack stat
/// (`FireAttack` 20, then every second id up to `PhantomAttack` 32).
pub const WEAPON_ELEMENT_STAT: i32 = 52;
pub const FIRE_ATTACK_STAT: i32 = 20;
/// Zircon `Globals.RefineTimes` by quality, in milliseconds.
pub const REFINE_TIMES: [u64; 5] = [60_000, 1_800_000, 3_600_000, 21_600_000, 86_400_000];

/// A weapon in the furnace (Zircon `RefineInfo`), persisted per character.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredRefine {
    pub index: u32,
    pub weapon: UserItem,
    pub refine_type: u8,
    pub quality: u8,
    pub chance: i32,
    pub max_chance: i32,
    /// Unix seconds when it can be collected.
    pub ready_at: u64,
}

fn now_secs() -> u64 {
    crate::accounts::now_secs()
}

impl World {
    fn refine_line(&mut self, id: ObjectId, text: &str) {
        self.send_to(
            id,
            ServerMessage::Say {
                id: None,
                kind: ChatKind::System,
                text: text.to_string(),
            },
        );
    }

    /// The open NPC page's dialog type, if any.
    pub(super) fn npc_dialog_type(&self, id: ObjectId) -> Option<i32> {
        let (_, page) = self.objects.get(&id)?.player()?.npc?;
        Some(self.data.npc_pages.get(&page)?.dialog_type)
    }

    pub(super) fn send_refine_list(&mut self, id: ObjectId) {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let now = now_secs();
        let list: Vec<RefineSummary> = p
            .refines
            .iter()
            .map(|r| RefineSummary {
                index: r.index,
                weapon: r.weapon.instance(),
                refine_type: r.refine_type,
                quality: r.quality,
                chance: r.chance,
                max_chance: r.max_chance,
                ready_in_ms: r.ready_at.saturating_sub(now) * 1000,
            })
            .collect();
        self.send_to(id, ServerMessage::RefineList(list));
    }

    /// Start a refine (Zircon `NPCRefine`): cells are (grid, slot, count).
    pub fn npc_refine(
        &mut self,
        id: ObjectId,
        refine_type: u8,
        quality: u8,
        ores: Vec<(Grid, u8, u32)>,
        items: Vec<(Grid, u8, u32)>,
        specials: Vec<(Grid, u8, u32)>,
    ) {
        if !(refine_type::DURABILITY..=refine_type::PHANTOM).contains(&refine_type)
            || quality > refine_quality::PRECISE
        {
            return;
        }
        if self.npc_dialog_type(id) != Some(3) {
            return;
        }
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let Some(p) = o.player() else {
            return;
        };
        if o.dead || ores.len() > 5 || items.len() > 3 || specials.len() > 1 {
            return;
        }
        let Some(weapon) = p.bag.equipment.get(slot::WEAPON).cloned().flatten() else {
            self.refine_line(id, "You need to be holding the weapon to refine.");
            return;
        };
        if p.bag.gold < REFINE_COST {
            self.refine_line(id, "Refining costs 50,000 gold.");
            return;
        }
        let in_safe_zone = o.in_safe_zone;
        let cell = |grid: Grid, s: u8| -> Option<UserItem> {
            if grid == Grid::Storage && !in_safe_zone {
                return None;
            }
            p.bag.grid(grid).get(s as usize).cloned().flatten()
        };
        let (mut ore, mut level_sum, mut special) = (0i64, 0i64, 0i32);
        for (g, s, c) in &ores {
            let Some(it) = cell(*g, *s) else { return };
            if *c == 0 || *c > it.count {
                return;
            }
            if self.data.items.get(&it.info).map(|d| d.effect) != Some(BLACK_IRON_ORE_EFFECT) {
                self.refine_line(id, "Only black iron ore feeds the furnace.");
                return;
            }
            ore += it.durability as i64;
        }
        let mut quality_items = 0;
        for (g, s, c) in &items {
            let Some(it) = cell(*g, *s) else { return };
            if *c == 0 || *c > it.count {
                return;
            }
            let Some(def) = self.data.items.get(&it.info) else {
                return;
            };
            if !matches!(
                def.item_type,
                item_type::NECKLACE | item_type::BRACELET | item_type::RING
            ) {
                self.refine_line(id, "Only necklaces, bracelets and rings go in.");
                return;
            }
            if def.rarity != 0 {
                quality_items += 1;
            }
            level_sum += def.required_amount as i64;
        }
        for (g, s, _) in &specials {
            let Some(it) = cell(*g, *s) else { return };
            let Some(def) = self.data.items.get(&it.info) else {
                return;
            };
            if def.item_type != REFINE_SPECIAL_TYPE || def.shape != 1 {
                return;
            }
            special += def.stat(MAX_REFINE_CHANCE_STAT);
        }
        // Zircon: max 90% - level (+special), base 60% - 5% per level, ore
        // 1% per 2000 durability, jewellery 1% per 6 levels, 25% per superior
        // item; quality shifts the ceiling.
        let mut max_chance = 90 - weapon.level + special;
        let mut chance = 60 - weapon.level * 5;
        max_chance += match quality {
            refine_quality::RUSH => -5,
            refine_quality::QUICK => 0,
            refine_quality::STANDARD => 5,
            refine_quality::CAREFUL => 10,
            _ => 20,
        };
        chance += (ore / 2000) as i32;
        chance += (level_sum / 6) as i32;
        chance += quality_items * 25;
        max_chance = max_chance.min(100);
        chance = chance.min(max_chance);

        let p = self
            .objects
            .get_mut(&id)
            .and_then(|o| o.player_mut())
            .unwrap();
        let mut changes = Changed::new();
        for (g, s, c) in ores.iter().chain(&items) {
            if let Some(ch) = p.bag.take(*g, *s, *c) {
                changes.push(ch);
            }
        }
        for (g, s, _) in &specials {
            if let Some(ch) = p.bag.take(*g, *s, 1) {
                changes.push(ch);
            }
        }
        p.bag.equipment[slot::WEAPON] = None;
        changes.push((Grid::Equipment, slot::WEAPON as u8, None));
        p.bag.gold -= REFINE_COST;
        let index = p.refines.iter().map(|r| r.index).max().unwrap_or(0) + 1;
        p.refines.push(StoredRefine {
            index,
            weapon,
            refine_type,
            quality,
            chance,
            max_chance,
            ready_at: now_secs() + REFINE_TIMES[quality as usize] / 1000,
        });
        self.send_changes(id, changes);
        self.send_gold(id);
        self.refresh_stats(id, false);
        self.refresh_appearance(id);
        self.send_player_stats(id);
        self.send_refine_list(id);
        self.refine_line(id, "Your weapon is in the furnace.");
    }

    /// Collect a refined weapon (Zircon `NPCRefineRetrieve`).
    pub fn npc_refine_retrieve(&mut self, id: ObjectId, index: u32) {
        if self.npc_dialog_type(id) != Some(4) {
            return;
        }
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let Some(r) = p.refines.iter().find(|r| r.index == index).cloned() else {
            return;
        };
        if now_secs() < r.ready_at {
            self.refine_line(id, "Your weapon is not ready yet.");
            return;
        }
        if !p.bag.inventory.iter().any(|s| s.is_none()) {
            self.refine_line(id, "You need a free bag slot for the weapon.");
            return;
        }
        let mut weapon = r.weapon.clone();
        let def = self.data.items.get(&weapon.info).cloned();
        let success = self.rng.random_range(0..100) < r.chance;
        if success {
            let mut add = |stat_id: i32, amount: i32| {
                if let Some((_, a)) = weapon.added.iter_mut().find(|(s, _)| *s == stat_id) {
                    *a += amount;
                } else {
                    weapon.added.push((stat_id, amount));
                }
            };
            match r.refine_type {
                refine_type::DURABILITY => weapon.max_durability += 2000,
                refine_type::DC => add(stat::MAX_DC, 1),
                refine_type::SPELL_POWER => {
                    let has = |s: i32| def.as_ref().map(|d| d.stat(s) > 0).unwrap_or(false);
                    let mc = has(stat::MIN_MC) || has(stat::MAX_MC);
                    let sc = has(stat::MIN_SC) || has(stat::MAX_SC);
                    if !mc && !sc {
                        add(stat::MAX_MC, 1);
                        add(stat::MAX_SC, 1);
                    } else {
                        if mc {
                            add(stat::MAX_MC, 1);
                        }
                        if sc {
                            add(stat::MAX_SC, 1);
                        }
                    }
                }
                t @ refine_type::FIRE..=refine_type::PHANTOM => {
                    let element = (t - refine_type::FIRE + 1) as i32;
                    add(FIRE_ATTACK_STAT + (element - 1) * 2, 1);
                    weapon.added.retain(|(s, _)| *s != WEAPON_ELEMENT_STAT);
                    weapon.added.push((WEAPON_ELEMENT_STAT, element));
                }
                _ => {}
            }
            weapon.level += 1;
            self.refine_line(id, "The refine succeeded.");
        } else {
            self.refine_line(id, "The refine failed.");
        }
        let p = self
            .objects
            .get_mut(&id)
            .and_then(|o| o.player_mut())
            .unwrap();
        p.refines.retain(|x| x.index != index);
        let free = p.bag.inventory.iter().position(|s| s.is_none()).unwrap();
        p.bag.inventory[free] = Some(weapon.clone());
        self.send_changes(
            id,
            vec![(Grid::Inventory, free as u8, Some(weapon.instance()))],
        );
        self.send_to(id, ServerMessage::RefineRetrieved { index });
    }
}
