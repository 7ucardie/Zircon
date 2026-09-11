//! Inventory, equipment and item rules (Zircon `PlayerObject.Inventory.cs`).

use std::collections::HashMap;

use mir_proto::{
    item_type, Class, Gender, Grid, ItemInstance, Weights, EQUIPMENT_SIZE, INVENTORY_SIZE,
};
use serde::{Deserialize, Serialize};

use crate::data::{GameData, ItemDef};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserItem {
    pub id: u32,
    pub info: i32,
    pub count: u32,
    pub durability: i32,
    pub max_durability: i32,
}

impl UserItem {
    pub fn instance(&self) -> ItemInstance {
        ItemInstance {
            id: self.id,
            info: self.info,
            count: self.count,
            durability: self.durability,
            max_durability: self.max_durability,
        }
    }
}

/// Persisted form: which grid and slot an item sits in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredItem {
    pub grid: Grid,
    pub slot: u8,
    pub item: UserItem,
}

#[derive(Debug, Default)]
pub struct Bag {
    pub inventory: Vec<Option<UserItem>>,
    pub equipment: Vec<Option<UserItem>>,
    pub gold: u64,
}

/// Result of putting an item somewhere: the slots that changed.
pub type Changed = Vec<(Grid, u8, Option<ItemInstance>)>;

impl Bag {
    pub fn new() -> Bag {
        Bag {
            inventory: (0..INVENTORY_SIZE).map(|_| None).collect(),
            equipment: (0..EQUIPMENT_SIZE).map(|_| None).collect(),
            gold: 0,
        }
    }

    pub fn from_stored(items: &[StoredItem], gold: u64) -> Bag {
        let mut b = Bag::new();
        b.gold = gold;
        for s in items {
            let target = match s.grid {
                Grid::Inventory => b.inventory.get_mut(s.slot as usize),
                Grid::Equipment => b.equipment.get_mut(s.slot as usize),
            };
            if let Some(cell) = target {
                if cell.is_none() {
                    *cell = Some(s.item.clone());
                }
            }
        }
        b
    }

    pub fn to_stored(&self) -> Vec<StoredItem> {
        let mut out = Vec::new();
        for (i, it) in self.inventory.iter().enumerate() {
            if let Some(item) = it {
                out.push(StoredItem {
                    grid: Grid::Inventory,
                    slot: i as u8,
                    item: item.clone(),
                });
            }
        }
        for (i, it) in self.equipment.iter().enumerate() {
            if let Some(item) = it {
                out.push(StoredItem {
                    grid: Grid::Equipment,
                    slot: i as u8,
                    item: item.clone(),
                });
            }
        }
        out
    }

    pub fn grid(&self, g: Grid) -> &Vec<Option<UserItem>> {
        match g {
            Grid::Inventory => &self.inventory,
            Grid::Equipment => &self.equipment,
        }
    }

    pub fn grid_mut(&mut self, g: Grid) -> &mut Vec<Option<UserItem>> {
        match g {
            Grid::Inventory => &mut self.inventory,
            Grid::Equipment => &mut self.equipment,
        }
    }

    pub fn max_id(&self) -> u32 {
        self.inventory
            .iter()
            .chain(self.equipment.iter())
            .flatten()
            .map(|i| i.id)
            .max()
            .unwrap_or(0)
    }

    pub fn count_of(&self, info: i32) -> u32 {
        self.inventory
            .iter()
            .flatten()
            .filter(|i| i.info == info)
            .map(|i| i.count)
            .sum()
    }

    /// Weight of everything in the bag (Zircon `RefreshWeight`).
    pub fn bag_weight(&self, data: &GameData) -> i32 {
        self.inventory
            .iter()
            .flatten()
            .map(|i| item_weight(data, i))
            .sum()
    }

    pub fn hand_weight(&self, data: &GameData) -> i32 {
        self.equipment
            .iter()
            .flatten()
            .filter(|i| {
                matches!(
                    data.items.get(&i.info).map(|d| d.item_type),
                    Some(item_type::WEAPON) | Some(item_type::TORCH)
                )
            })
            .map(|i| item_weight(data, i))
            .sum()
    }

    pub fn wear_weight(&self, data: &GameData) -> i32 {
        self.equipment
            .iter()
            .flatten()
            .filter(|i| {
                !matches!(
                    data.items.get(&i.info).map(|d| d.item_type),
                    Some(item_type::WEAPON) | Some(item_type::TORCH)
                )
            })
            .map(|i| item_weight(data, i))
            .sum()
    }

    /// Can `count` of `info` be added (stacking first, then empty slots)?
    pub fn can_gain(&self, data: &GameData, info: i32, count: u32, max_bag: i32) -> bool {
        let Some(def) = data.items.get(&info) else {
            return false;
        };
        if self.bag_weight(data) + def.weight * count as i32 > max_bag {
            return false;
        }
        let mut remaining = count;
        for it in self.inventory.iter().flatten() {
            if it.info == info && it.count < def.stack_size as u32 {
                remaining = remaining.saturating_sub(def.stack_size as u32 - it.count);
            }
        }
        let empty = self.inventory.iter().filter(|s| s.is_none()).count() as u32;
        let stack = def.stack_size.max(1) as u32;
        remaining.div_ceil(stack) <= empty
    }

    /// Add items, stacking where possible. Returns changed slots. `next_id`
    /// is bumped for every new stack.
    pub fn gain(
        &mut self,
        data: &GameData,
        info: i32,
        mut count: u32,
        next_id: &mut u32,
    ) -> Changed {
        let mut changed = Changed::new();
        let Some(def) = data.items.get(&info) else {
            return changed;
        };
        let stack = def.stack_size.max(1) as u32;
        for (i, it) in self.inventory.iter_mut().enumerate() {
            if count == 0 {
                break;
            }
            if let Some(item) = it {
                if item.info == info && item.count < stack {
                    let add = (stack - item.count).min(count);
                    item.count += add;
                    count -= add;
                    changed.push((Grid::Inventory, i as u8, Some(item.instance())));
                }
            }
        }
        for (i, it) in self.inventory.iter_mut().enumerate() {
            if count == 0 {
                break;
            }
            if it.is_none() {
                let add = stack.min(count);
                *next_id += 1;
                let item = UserItem {
                    id: *next_id,
                    info,
                    count: add,
                    durability: def.durability,
                    max_durability: def.durability,
                };
                changed.push((Grid::Inventory, i as u8, Some(item.instance())));
                *it = Some(item);
                count -= add;
            }
        }
        changed
    }

    /// Remove `count` from a slot; returns the change (None when emptied).
    pub fn take(
        &mut self,
        grid: Grid,
        slot: u8,
        count: u32,
    ) -> Option<(Grid, u8, Option<ItemInstance>)> {
        let cell = self.grid_mut(grid).get_mut(slot as usize)?;
        let item = cell.as_mut()?;
        if item.count <= count {
            *cell = None;
            Some((grid, slot, None))
        } else {
            item.count -= count;
            Some((grid, slot, Some(item.instance())))
        }
    }

    /// Remove `count` of `info` from anywhere in the inventory.
    pub fn take_info(&mut self, info: i32, mut count: u32) -> Changed {
        let mut changed = Changed::new();
        for i in 0..self.inventory.len() {
            if count == 0 {
                break;
            }
            let Some(item) = &self.inventory[i] else {
                continue;
            };
            if item.info != info {
                continue;
            }
            let take = item.count.min(count);
            count -= take;
            if let Some(c) = self.take(Grid::Inventory, i as u8, take) {
                changed.push(c);
            }
        }
        changed
    }

    pub fn weights(&self, data: &GameData, max_bag: i32, max_wear: i32, max_hand: i32) -> Weights {
        Weights {
            bag: self.bag_weight(data),
            max_bag,
            wear: self.wear_weight(data),
            max_wear,
            hand: self.hand_weight(data),
            max_hand,
        }
    }

    /// Sum of `ItemInfoStat`s over equipped, unbroken items.
    pub fn equipment_stats(&self, data: &GameData) -> HashMap<i32, i32> {
        let mut out = HashMap::new();
        for item in self.equipment.iter().flatten() {
            if item.max_durability > 0 && item.durability <= 0 {
                continue;
            }
            if let Some(def) = data.items.get(&item.info) {
                for (k, v) in &def.stats {
                    *out.entry(*k).or_insert(0) += v;
                }
            }
        }
        out
    }

    pub fn equipped_shape(&self, data: &GameData, slot: usize) -> Option<i32> {
        self.equipment
            .get(slot)?
            .as_ref()
            .and_then(|i| data.items.get(&i.info))
            .map(|d| d.shape)
    }
}

pub fn item_weight(data: &GameData, item: &UserItem) -> i32 {
    match data.items.get(&item.info) {
        Some(d) if matches!(d.item_type, item_type::POISON | item_type::AMULET) => d.weight,
        Some(d) => d.weight * item.count as i32,
        None => 0,
    }
}

/// Zircon `CanUseItem`: gender, class and the required-stat check.
pub fn can_use(def: &ItemDef, class: Class, gender: Gender, level: i32) -> Result<(), String> {
    let gflag = match gender {
        Gender::Male => 1,
        Gender::Female => 2,
    };
    if def.required_gender & gflag == 0 {
        return Err("Your gender cannot use this item".into());
    }
    if def.required_class & class.flag() == 0 {
        return Err("Your class cannot use this item".into());
    }
    match def.required_type {
        0 if level < def.required_amount => Err(format!("Requires level {}", def.required_amount)),
        1 if level > def.required_amount => {
            Err(format!("Requires level {} or below", def.required_amount))
        }
        _ => Ok(()),
    }
}

/// Which equipment slot an item goes to by default (left before right).
pub fn default_slot(item_type: u8, equipment: &[Option<UserItem>]) -> Option<usize> {
    let slots = item_type::slots(item_type);
    slots
        .iter()
        .copied()
        .find(|s| equipment.get(*s).map(|c| c.is_none()).unwrap_or(false))
        .or_else(|| slots.first().copied())
}

/// Sell price (Zircon `UserItem.Price` with no added stats).
pub fn sell_price(def: &ItemDef, item: &UserItem) -> u64 {
    let mut p = def.price as f64;
    if def.durability > 0 {
        let r = def.price as f64 / 2.0 / def.durability as f64;
        p = item.max_durability as f64 * r;
        let ratio = if item.max_durability > 0 {
            item.durability as f64 / item.max_durability as f64
        } else {
            0.0
        };
        p = (p / 2.0 + p / 2.0 * ratio + def.price as f64 / 2.0).floor();
    }
    (p * item.count as f64 * def.sell_rate).max(0.0) as u64
}
