//! Client-side item catalogue read from `System.db` (names, icons, stats).

use std::collections::HashMap;
use std::path::Path;

use mir_formats::mirdb::MirDb;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ItemDef {
    pub index: i32,
    pub name: String,
    pub item_type: u8,
    pub required_class: u8,
    pub required_type: u8,
    pub required_amount: i32,
    pub shape: i32,
    pub image: i32,
    pub durability: i32,
    pub price: i32,
    pub weight: i32,
    pub stack_size: i32,
    pub description: String,
    pub stats: Vec<(i32, i32)>,
}

pub struct ItemCatalog {
    items: HashMap<i32, ItemDef>,
}

pub fn stat_name(id: i32) -> Option<&'static str> {
    Some(match id {
        2 => "HP",
        3 => "MP",
        4 => "Min AC",
        5 => "Max AC",
        6 => "Min MR",
        7 => "Max MR",
        8 => "Min DC",
        9 => "Max DC",
        10 => "Min MC",
        11 => "Max MC",
        12 => "Min SC",
        13 => "Max SC",
        14 => "Accuracy",
        15 => "Agility",
        16 => "Attack Speed",
        18 => "Strength",
        19 => "Luck",
        73 => "Bag Weight",
        74 => "Wear Weight",
        75 => "Hand Weight",
        _ => return None,
    })
}

pub fn type_name(t: u8) -> &'static str {
    match t {
        1 => "Consumable",
        2 => "Weapon",
        3 => "Armour",
        4 => "Torch",
        5 => "Helmet",
        6 => "Necklace",
        7 => "Bracelet",
        8 => "Ring",
        9 => "Shoes",
        10 => "Poison",
        11 => "Amulet",
        12 => "Meat",
        13 => "Ore",
        14 => "Book",
        15 => "Scroll",
        16 => "Dark Stone",
        27 => "Shield",
        34 => "Currency",
        _ => "Item",
    }
}

impl ItemCatalog {
    pub fn empty() -> ItemCatalog {
        ItemCatalog {
            items: HashMap::new(),
        }
    }

    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<ItemCatalog> {
        let db = MirDb::load(path.as_ref())
            .map_err(|e| anyhow::anyhow!("loading {}: {e}", path.as_ref().display()))?;
        let mut stats: HashMap<i32, Vec<(i32, i32)>> = HashMap::new();
        if let Some(c) = db.collection("ItemInfoStat") {
            for r in &c.records {
                stats
                    .entry(c.int_or(r, "Item", 0) as i32)
                    .or_default()
                    .push((
                        c.int_or(r, "Stat", 0) as i32,
                        c.int_or(r, "Amount", 0) as i32,
                    ));
            }
        }
        let c = db
            .collection("ItemInfo")
            .ok_or_else(|| anyhow::anyhow!("no ItemInfo collection"))?;
        let items = c
            .records
            .iter()
            .map(|r| {
                let index = c.index(r);
                let d = ItemDef {
                    index,
                    name: c.str_or(r, "ItemName", "").to_string(),
                    item_type: c.int_or(r, "ItemType", 0) as u8,
                    required_class: c.int_or(r, "RequiredClass", 15) as u8,
                    required_type: c.int_or(r, "RequiredType", 0) as u8,
                    required_amount: c.int_or(r, "RequiredAmount", 0) as i32,
                    shape: c.int_or(r, "Shape", 0) as i32,
                    image: c.int_or(r, "Image", 0) as i32,
                    durability: c.int_or(r, "Durability", 0) as i32,
                    price: c.int_or(r, "Price", 0) as i32,
                    weight: c.int_or(r, "Weight", 0) as i32,
                    stack_size: c.int_or(r, "StackSize", 1) as i32,
                    description: c.str_or(r, "Description", "").to_string(),
                    stats: stats.remove(&index).unwrap_or_default(),
                };
                (index, d)
            })
            .collect();
        Ok(ItemCatalog { items })
    }

    pub fn get(&self, index: i32) -> Option<&ItemDef> {
        self.items.get(&index)
    }

    pub fn name(&self, index: i32) -> String {
        self.items
            .get(&index)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| format!("Item {index}"))
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}
