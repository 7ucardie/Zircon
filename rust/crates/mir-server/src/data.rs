//! Typed view over `System.db` for the collections the server needs.
//!
//! Fields not read yet are kept so the loader stays a complete description of
//! the rows the game will need.
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{Context, Result};
use mir_formats::mirdb::{self, region_points, Collection, MirDb, Record, Value};

#[derive(Debug, Clone)]
pub struct MapDef {
    pub index: i32,
    pub file_name: String,
    pub description: String,
    pub minimum_level: i32,
    pub background: i32,
}

#[derive(Debug, Clone)]
pub struct RegionDef {
    pub index: i32,
    pub map: i32,
    pub description: String,
    bit_region: Option<Vec<u8>>,
    point_region: Option<Vec<(i32, i32)>>,
}

impl RegionDef {
    /// Decode the region's cells. Needs the map width because the bit array is
    /// indexed row-major over the map.
    pub fn points(&self, map_width: i32) -> Vec<(i32, i32)> {
        region_points(
            self.bit_region.as_deref(),
            self.point_region.as_deref(),
            map_width,
        )
    }

    /// Cells used as movement sources (Zircon attaches movements to
    /// `SourceRegion.PointRegion`).
    pub fn movement_points(&self, map_width: i32) -> Vec<(i32, i32)> {
        match &self.point_region {
            Some(p) if !p.is_empty() => p.clone(),
            _ => self.points(map_width),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RespawnDef {
    pub index: i32,
    pub monster: i32,
    pub region: i32,
    pub event_spawn: bool,
    /// Minutes (Zircon multiplies by 60 into seconds).
    pub delay: i32,
    pub count: i32,
    pub announce: bool,
    pub respawn_index: i32,
}

#[derive(Debug, Clone)]
pub struct MonsterDef {
    pub index: i32,
    pub name: String,
    pub image: u16,
    pub ai: i32,
    pub level: i32,
    pub view_range: i32,
    pub cool_eye: i32,
    pub experience: f64,
    pub undead: bool,
    pub can_push: bool,
    pub can_tame: bool,
    /// Zircon `MonsterFlag` (1 Skeleton, 2 JinSkeleton, 3 Shinsu, 6 SummonPuppet).
    pub flag: i32,
    pub attack_delay: i64,
    pub move_delay: i64,
    pub is_boss: bool,
    stats: HashMap<i32, i32>,
}

impl MonsterDef {
    pub fn stat(&self, id: i32) -> i32 {
        self.stats.get(&id).copied().unwrap_or(0)
    }
    pub fn health(&self) -> i32 {
        self.stat(mirdb::stat::HEALTH).max(1)
    }
    /// Zircon `MonsterRegistrations`: AI 1 and 2 are `Passive = true` (farm
    /// animals), as is the tree monster (AI 4).
    pub fn is_passive(&self) -> bool {
        matches!(self.ai, 1 | 2 | 4)
    }
}

/// `GuardInfo`: a fixed monster placement on a map.
#[derive(Debug, Clone)]
pub struct GuardDef {
    pub map: i32,
    pub monster: i32,
    pub x: i32,
    pub y: i32,
    pub direction: u8,
}

#[derive(Debug, Clone)]
pub struct SafeZoneDef {
    pub index: i32,
    pub region: i32,
    pub bind_region: i32,
    /// `RequiredClass` flags: Warrior=1, Wizard=2, Taoist=4, Assassin=8.
    pub start_class: u8,
    pub red_zone: bool,
}

#[derive(Debug, Clone, Default)]
pub struct BaseStatDef {
    pub class: u8,
    pub level: i32,
    pub health: i32,
    pub mana: i32,
    pub accuracy: i32,
    pub agility: i32,
    pub min_ac: i32,
    pub max_ac: i32,
    pub min_mr: i32,
    pub max_mr: i32,
    pub min_dc: i32,
    pub max_dc: i32,
    pub min_mc: i32,
    pub max_mc: i32,
    pub min_sc: i32,
    pub max_sc: i32,
    pub bag_weight: i32,
    pub wear_weight: i32,
    pub hand_weight: i32,
}

#[derive(Debug, Clone)]
pub struct NpcDef {
    pub index: i32,
    pub region: i32,
    pub name: String,
    pub image: i32,
    pub face_image: i32,
    pub entry_page: i32,
    /// `NPCRequirement`: who can see this NPC at all.
    pub requirements: Vec<NpcRequirementDef>,
}

/// Zircon `NPCRequirementType`: MinLevel 0, MaxLevel 1, Accepted 2,
/// NotAccepted 3, HaveCompleted 4, HaveNotCompleted 5, Class 6, DaysOfWeek 7.
#[derive(Debug, Clone)]
pub struct NpcRequirementDef {
    pub requirement: i32,
    pub int1: i32,
    pub quest: i32,
    pub class: u8,
    pub days: i32,
}

/// `QuestInfo` with its requirements, tasks and rewards.
#[derive(Debug, Clone)]
pub struct QuestDef {
    pub index: i32,
    pub name: String,
    /// Zircon `QuestType`: General 0, Daily 1, Weekly 2, Repeatable 3, Story 4, Account 5.
    pub quest_type: i32,
    pub start_npc: i32,
    pub finish_npc: i32,
    pub requirements: Vec<QuestRequirementDef>,
    pub tasks: Vec<QuestTaskDef>,
    pub rewards: Vec<QuestRewardDef>,
}

/// `QuestRequirementType`: MinLevel 0, MaxLevel 1, NotAccepted 2,
/// HaveCompleted 3, HaveNotCompleted 4, Class 5.
#[derive(Debug, Clone)]
pub struct QuestRequirementDef {
    pub requirement: i32,
    pub int1: i32,
    pub quest: i32,
    pub class: u8,
}

/// `QuestTaskType`: KillMonster 0, GainItem 1, Region 2.
#[derive(Debug, Clone)]
pub struct QuestTaskDef {
    pub index: i32,
    pub task: i32,
    pub item: i32,
    pub region: i32,
    pub amount: i32,
    pub monsters: Vec<QuestMonsterDef>,
}

#[derive(Debug, Clone)]
pub struct QuestMonsterDef {
    pub monster: i32,
    pub map: i32,
    /// 1-in-`chance` per kill (0/1 = always).
    pub chance: i32,
    pub amount: i32,
}

#[derive(Debug, Clone)]
pub struct QuestRewardDef {
    pub index: i32,
    pub item: i32,
    pub amount: i32,
    pub choice: bool,
    pub bound: bool,
    pub class: u8,
}

/// `CurrencyInfo`.
#[derive(Debug, Clone)]
pub struct CurrencyDef {
    pub index: i32,
    pub name: String,
    pub abbreviation: String,
    /// The `ItemInfo` that represents this currency when dropped or rewarded.
    pub drop_item: i32,
    pub exchange_rate: f64,
}

#[derive(Debug, Clone)]
pub struct ItemDef {
    pub index: i32,
    pub name: String,
    /// Zircon `ItemType` value.
    pub item_type: u8,
    /// Zircon `ItemEffect`: Experience 2, PickAxe 5 ...
    pub effect: u8,
    /// `RequiredClass` flags (Warrior=1, Wizard=2, Taoist=4, Assassin=8).
    pub required_class: u8,
    /// `RequiredGender` flags (Male=1, Female=2).
    pub required_gender: u8,
    pub required_type: u8,
    pub required_amount: i32,
    pub shape: i32,
    pub image: i32,
    pub durability: i32,
    pub price: i32,
    pub weight: i32,
    pub stack_size: i32,
    pub start_item: bool,
    pub sell_rate: f64,
    pub can_sell: bool,
    pub can_drop: bool,
    pub description: String,
    pub stats: HashMap<i32, i32>,
}

impl ItemDef {
    pub fn stat(&self, id: i32) -> i32 {
        self.stats.get(&id).copied().unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub struct DropDef {
    pub monster: i32,
    pub item: i32,
    /// Zircon: drops when `Random.Next(Chance) == 0`, i.e. 1 in `chance`.
    pub chance: i32,
    pub amount: i32,
    pub drop_set: i32,
    pub part_only: bool,
}

#[derive(Debug, Clone)]
pub struct NpcPageDef {
    pub index: i32,
    pub description: String,
    pub dialog_type: i32,
    pub say: String,
    pub success_page: i32,
    pub arguments: String,
    pub buttons: Vec<(i32, i32)>,
    pub goods: Vec<(i32, f64)>,
    pub checks: Vec<NpcCheckDef>,
    pub actions: Vec<NpcActionDef>,
    /// Item types accepted by a sell/repair page.
    pub types: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct NpcCheckDef {
    pub check_type: i32,
    pub operator: i32,
    pub string1: String,
    pub int1: i32,
    pub int2: i32,
    pub item1: i32,
    pub stat1: i32,
    pub fail_page: i32,
}

#[derive(Debug, Clone)]
pub struct NpcActionDef {
    pub action_type: i32,
    pub string1: String,
    pub int1: i32,
    pub int2: i32,
    pub item1: i32,
    pub map1: i32,
    pub stat1: i32,
}

#[derive(Debug, Clone)]
pub struct MagicDef {
    pub index: i32,
    pub name: String,
    /// Zircon `MagicType` value.
    pub magic: u16,
    /// Class flags (Warrior 1, Wizard 2, Taoist 4, Assassin 8); upstream
    /// Zircon moved from a single `Class` to `RequiredClass` flags.
    pub class: u8,
    pub school: i32,
    pub icon: i32,
    pub min_base_power: i32,
    pub max_base_power: i32,
    pub min_level_power: i32,
    pub max_level_power: i32,
    pub base_cost: i32,
    pub level_cost: i32,
    pub need_level: [i32; 3],
    pub experience: [i32; 3],
    /// Cooldown in milliseconds.
    pub delay: i32,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct MovementDef {
    pub index: i32,
    pub source_region: i32,
    pub destination_region: i32,
    pub icon: i32,
    pub need_item: i32,
    pub need_spawn: i32,
    pub effect: i32,
    pub required_class: u8,
}

/// Zircon `Globals.ExperienceList`: experience needed at each level.
pub const EXPERIENCE: [u64; 41] = [
    0, 100, 200, 300, 400, 600, 900, 1200, 1700, 2500, 6000, 8000, 10000, 15000, 30000, 40000,
    50000, 70000, 100000, 120000, 140000, 250000, 300000, 350000, 400000, 500000, 700000, 1000000,
    1400000, 1800000, 2000000, 2400000, 2800000, 3200000, 3600000, 4000000, 4800000, 5600000,
    8200000, 9000000, 11000000,
];

/// One finding of [`GameData::validate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// Collection short name (`RespawnInfo`).
    pub collection: String,
    /// Record `Index` (for `DropInfo`, which has none, the row position).
    pub index: i32,
    pub message: String,
}

impl Problem {
    fn new(collection: &str, index: i32, message: String) -> Problem {
        Problem {
            collection: collection.to_string(),
            index,
            message,
        }
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} #{}: {}", self.collection, self.index, self.message)
    }
}

#[derive(Debug)]
pub struct GameData {
    pub maps: BTreeMap<i32, MapDef>,
    pub regions: HashMap<i32, RegionDef>,
    pub respawns: Vec<RespawnDef>,
    pub monsters: HashMap<i32, MonsterDef>,
    pub safe_zones: Vec<SafeZoneDef>,
    pub guards: Vec<GuardDef>,
    pub currencies: Vec<CurrencyDef>,
    pub quests: HashMap<i32, QuestDef>,
    pub base_stats: Vec<BaseStatDef>,
    pub npcs: Vec<NpcDef>,
    pub items: HashMap<i32, ItemDef>,
    pub drops: Vec<DropDef>,
    pub npc_pages: HashMap<i32, NpcPageDef>,
    pub movements: Vec<MovementDef>,
    /// `ItemInfo.Index` of gold.
    pub gold_item: i32,
    /// Keyed by `MagicType` value.
    pub magics: HashMap<u16, MagicDef>,
}

impl GameData {
    /// Books carry `MagicInfo.Index` in their `Shape`.
    pub fn magic_by_index(&self, index: i32) -> Option<&MagicDef> {
        self.magics.values().find(|m| m.index == index)
    }
}

fn i32_of(c: &Collection, r: &Record, name: &str) -> i32 {
    c.int_or(r, name, 0) as i32
}

impl GameData {
    pub fn load(path: impl AsRef<Path>) -> Result<GameData> {
        let db = MirDb::load(path.as_ref())
            .with_context(|| format!("loading {}", path.as_ref().display()))?;
        GameData::from_db(&db)
    }

    /// Build the game data from an already parsed database (the editor
    /// validates unsaved edits this way).
    pub fn from_db(db: &MirDb) -> Result<GameData> {
        let get = |name: &str| {
            db.collection(name)
                .with_context(|| format!("System.db has no {name} collection"))
        };

        let c = get("MapInfo")?;
        let maps = c
            .records
            .iter()
            .map(|r| {
                let m = MapDef {
                    index: c.index(r),
                    file_name: c.str_or(r, "FileName", "").to_string(),
                    description: c.str_or(r, "Description", "").to_string(),
                    minimum_level: i32_of(c, r, "MinimumLevel"),
                    background: i32_of(c, r, "Background"),
                };
                (m.index, m)
            })
            .collect();

        let c = get("MapRegion")?;
        let regions = c
            .records
            .iter()
            .map(|r| {
                let bit_region = match c.get(r, "BitRegion") {
                    Some(Value::BitArray(b)) => b.clone(),
                    _ => None,
                };
                let point_region = match c.get(r, "PointRegion") {
                    Some(Value::PointArray(p)) => p.clone(),
                    _ => None,
                };
                let d = RegionDef {
                    index: c.index(r),
                    map: i32_of(c, r, "Map"),
                    description: c.str_or(r, "Description", "").to_string(),
                    bit_region,
                    point_region,
                };
                (d.index, d)
            })
            .collect();

        let c = get("RespawnInfo")?;
        let respawns = c
            .records
            .iter()
            .map(|r| RespawnDef {
                index: c.index(r),
                monster: i32_of(c, r, "Monster"),
                region: i32_of(c, r, "Region"),
                event_spawn: c.bool_or(r, "EventSpawn", false),
                delay: i32_of(c, r, "Delay"),
                count: i32_of(c, r, "Count"),
                announce: c.bool_or(r, "Announce", false),
                respawn_index: i32_of(c, r, "RespawnIndex"),
            })
            .filter(|r| r.monster != 0 && r.region != 0)
            .collect();

        let mut monster_stats: HashMap<i32, HashMap<i32, i32>> = HashMap::new();
        let c = get("MonsterInfoStat")?;
        for r in &c.records {
            monster_stats
                .entry(i32_of(c, r, "Monster"))
                .or_default()
                .insert(i32_of(c, r, "Stat"), i32_of(c, r, "Amount"));
        }
        let c = get("MonsterInfo")?;
        let monsters = c
            .records
            .iter()
            .map(|r| {
                let index = c.index(r);
                let m = MonsterDef {
                    index,
                    name: c.str_or(r, "MonsterName", "").to_string(),
                    image: i32_of(c, r, "Image") as u16,
                    ai: i32_of(c, r, "AI"),
                    level: i32_of(c, r, "Level"),
                    view_range: i32_of(c, r, "ViewRange"),
                    cool_eye: i32_of(c, r, "CoolEye"),
                    experience: c.float_or(r, "Experience", 0.0),
                    undead: c.bool_or(r, "Undead", false),
                    can_push: c.bool_or(r, "CanPush", true),
                    can_tame: c.bool_or(r, "CanTame", false),
                    flag: i32_of(c, r, "Flag"),
                    attack_delay: c.int_or(r, "AttackDelay", 0),
                    move_delay: c.int_or(r, "MoveDelay", 0),
                    is_boss: c.bool_or(r, "IsBoss", false),
                    stats: monster_stats.remove(&index).unwrap_or_default(),
                };
                (index, m)
            })
            .collect();

        let c = get("SafeZoneInfo")?;
        let safe_zones = c
            .records
            .iter()
            .map(|r| SafeZoneDef {
                index: c.index(r),
                region: i32_of(c, r, "Region"),
                bind_region: i32_of(c, r, "BindRegion"),
                start_class: i32_of(c, r, "StartClass") as u8,
                red_zone: c.bool_or(r, "RedZone", false),
            })
            .collect();

        let c = get("BaseStat")?;
        let base_stats = c
            .records
            .iter()
            .map(|r| BaseStatDef {
                class: i32_of(c, r, "Class") as u8,
                level: i32_of(c, r, "Level"),
                health: i32_of(c, r, "Health"),
                mana: i32_of(c, r, "Mana"),
                accuracy: i32_of(c, r, "Accuracy"),
                agility: i32_of(c, r, "Agility"),
                min_ac: i32_of(c, r, "MinAC"),
                max_ac: i32_of(c, r, "MaxAC"),
                min_mr: i32_of(c, r, "MinMR"),
                max_mr: i32_of(c, r, "MaxMR"),
                min_dc: i32_of(c, r, "MinDC"),
                max_dc: i32_of(c, r, "MaxDC"),
                min_mc: i32_of(c, r, "MinMC"),
                max_mc: i32_of(c, r, "MaxMC"),
                min_sc: i32_of(c, r, "MinSC"),
                max_sc: i32_of(c, r, "MaxSC"),
                bag_weight: i32_of(c, r, "BagWeight"),
                wear_weight: i32_of(c, r, "WearWeight"),
                hand_weight: i32_of(c, r, "HandWeight"),
            })
            .collect();

        let mut npcs: Vec<NpcDef> = match db.collection("NPCInfo") {
            Some(c) => c
                .records
                .iter()
                .map(|r| NpcDef {
                    index: c.index(r),
                    region: i32_of(c, r, "Region"),
                    name: c.str_or(r, "NPCName", "").to_string(),
                    image: i32_of(c, r, "Image"),
                    face_image: i32_of(c, r, "FaceImage"),
                    entry_page: i32_of(c, r, "EntryPage"),
                    requirements: Vec::new(),
                })
                .filter(|n| n.region != 0)
                .collect(),
            None => Vec::new(),
        };

        let mut item_stats: HashMap<i32, HashMap<i32, i32>> = HashMap::new();
        if let Some(c) = db.collection("ItemInfoStat") {
            for r in &c.records {
                item_stats
                    .entry(i32_of(c, r, "Item"))
                    .or_default()
                    .insert(i32_of(c, r, "Stat"), i32_of(c, r, "Amount"));
            }
        }
        let c = get("ItemInfo")?;
        let items: HashMap<i32, ItemDef> = c
            .records
            .iter()
            .map(|r| {
                let index = c.index(r);
                let d = ItemDef {
                    index,
                    name: c.str_or(r, "ItemName", "").to_string(),
                    item_type: i32_of(c, r, "ItemType") as u8,
                    effect: i32_of(c, r, "ItemEffect") as u8,
                    required_class: i32_of(c, r, "RequiredClass") as u8,
                    required_gender: i32_of(c, r, "RequiredGender") as u8,
                    required_type: i32_of(c, r, "RequiredType") as u8,
                    required_amount: i32_of(c, r, "RequiredAmount"),
                    shape: i32_of(c, r, "Shape"),
                    image: i32_of(c, r, "Image"),
                    durability: i32_of(c, r, "Durability"),
                    price: i32_of(c, r, "Price"),
                    weight: i32_of(c, r, "Weight"),
                    stack_size: i32_of(c, r, "StackSize").max(1),
                    start_item: c.bool_or(r, "StartItem", false),
                    sell_rate: c.float_or(r, "SellRate", 0.0),
                    can_sell: c.bool_or(r, "CanSell", true),
                    can_drop: c.bool_or(r, "CanDrop", true),
                    description: c.str_or(r, "Description", "").to_string(),
                    stats: item_stats.remove(&index).unwrap_or_default(),
                };
                (index, d)
            })
            .collect();

        let drops = match db.collection("DropInfo") {
            Some(c) => c
                .records
                .iter()
                .map(|r| DropDef {
                    monster: i32_of(c, r, "Monster"),
                    item: i32_of(c, r, "Item"),
                    chance: i32_of(c, r, "Chance"),
                    amount: i32_of(c, r, "Amount"),
                    drop_set: i32_of(c, r, "DropSet"),
                    part_only: c.bool_or(r, "PartOnly", false),
                })
                .filter(|d| d.monster != 0 && d.item != 0)
                .collect(),
            None => Vec::new(),
        };

        let mut npc_pages: HashMap<i32, NpcPageDef> = match db.collection("NPCPage") {
            Some(c) => c
                .records
                .iter()
                .map(|r| {
                    let index = c.index(r);
                    let p = NpcPageDef {
                        index,
                        description: c.str_or(r, "Description", "").to_string(),
                        dialog_type: i32_of(c, r, "DialogType"),
                        say: c.str_or(r, "Say", "").to_string(),
                        success_page: i32_of(c, r, "SuccessPage"),
                        arguments: c.str_or(r, "Arguments", "").to_string(),
                        buttons: Vec::new(),
                        goods: Vec::new(),
                        checks: Vec::new(),
                        actions: Vec::new(),
                        types: Vec::new(),
                    };
                    (index, p)
                })
                .collect(),
            None => HashMap::new(),
        };
        if let Some(c) = db.collection("NPCButton") {
            for r in &c.records {
                if let Some(p) = npc_pages.get_mut(&i32_of(c, r, "Page")) {
                    p.buttons
                        .push((i32_of(c, r, "ButtonID"), i32_of(c, r, "DestinationPage")));
                }
            }
        }
        if let Some(c) = db.collection("NPCGood") {
            for r in &c.records {
                if let Some(p) = npc_pages.get_mut(&i32_of(c, r, "Page")) {
                    p.goods
                        .push((i32_of(c, r, "Item"), c.float_or(r, "Rate", 1.0)));
                }
            }
        }
        if let Some(c) = db.collection("NPCType") {
            for r in &c.records {
                if let Some(p) = npc_pages.get_mut(&i32_of(c, r, "Page")) {
                    p.types.push(i32_of(c, r, "ItemType") as u8);
                }
            }
        }
        if let Some(c) = db.collection("NPCCheck") {
            for r in &c.records {
                if let Some(p) = npc_pages.get_mut(&i32_of(c, r, "Page")) {
                    p.checks.push(NpcCheckDef {
                        check_type: i32_of(c, r, "CheckType"),
                        operator: i32_of(c, r, "Operator"),
                        string1: c.str_or(r, "StringParameter1", "").to_string(),
                        int1: i32_of(c, r, "IntParameter1"),
                        int2: i32_of(c, r, "IntParameter2"),
                        item1: i32_of(c, r, "ItemParameter1"),
                        stat1: i32_of(c, r, "StatParameter1"),
                        fail_page: i32_of(c, r, "FailPage"),
                    });
                }
            }
        }
        if let Some(c) = db.collection("NPCAction") {
            for r in &c.records {
                if let Some(p) = npc_pages.get_mut(&i32_of(c, r, "Page")) {
                    p.actions.push(NpcActionDef {
                        action_type: i32_of(c, r, "ActionType"),
                        string1: c.str_or(r, "StringParameter1", "").to_string(),
                        int1: i32_of(c, r, "IntParameter1"),
                        int2: i32_of(c, r, "IntParameter2"),
                        item1: i32_of(c, r, "ItemParameter1"),
                        map1: i32_of(c, r, "MapParameter1"),
                        stat1: i32_of(c, r, "StatParameter1"),
                    });
                }
            }
        }

        let movements = match db.collection("MovementInfo") {
            Some(c) => c
                .records
                .iter()
                .map(|r| MovementDef {
                    index: c.index(r),
                    source_region: i32_of(c, r, "SourceRegion"),
                    destination_region: i32_of(c, r, "DestinationRegion"),
                    icon: i32_of(c, r, "Icon"),
                    need_item: i32_of(c, r, "NeedItem"),
                    need_spawn: i32_of(c, r, "NeedSpawn"),
                    effect: i32_of(c, r, "Effect"),
                    required_class: i32_of(c, r, "RequiredClass") as u8,
                })
                .filter(|m| m.source_region != 0 && m.destination_region != 0)
                .collect(),
            None => Vec::new(),
        };

        let magics: HashMap<u16, MagicDef> = match db.collection("MagicInfo") {
            Some(c) => c
                .records
                .iter()
                .map(|r| {
                    let m = MagicDef {
                        index: c.index(r),
                        name: c.str_or(r, "Name", "").to_string(),
                        magic: i32_of(c, r, "Magic") as u16,
                        class: magic_class_flags(c, r),
                        school: i32_of(c, r, "School"),
                        icon: i32_of(c, r, "Icon"),
                        min_base_power: i32_of(c, r, "MinBasePower"),
                        max_base_power: i32_of(c, r, "MaxBasePower"),
                        min_level_power: i32_of(c, r, "MinLevelPower"),
                        max_level_power: i32_of(c, r, "MaxLevelPower"),
                        base_cost: i32_of(c, r, "BaseCost"),
                        level_cost: i32_of(c, r, "LevelCost"),
                        need_level: [
                            i32_of(c, r, "NeedLevel1"),
                            i32_of(c, r, "NeedLevel2"),
                            i32_of(c, r, "NeedLevel3"),
                        ],
                        experience: [
                            i32_of(c, r, "Experience1"),
                            i32_of(c, r, "Experience2"),
                            i32_of(c, r, "Experience3"),
                        ],
                        delay: i32_of(c, r, "Delay"),
                        description: c.str_or(r, "Description", "").to_string(),
                    };
                    (m.magic, m)
                })
                .collect(),
            None => HashMap::new(),
        };

        let gold_item = items
            .values()
            .find(|i: &&ItemDef| i.item_type == 34 && i.name.eq_ignore_ascii_case("Gold"))
            .map(|i| i.index)
            .unwrap_or(0);

        let guards = match db.collection("GuardInfo") {
            Some(c) => c
                .records
                .iter()
                .map(|r| GuardDef {
                    map: i32_of(c, r, "Map"),
                    monster: i32_of(c, r, "Monster"),
                    x: i32_of(c, r, "X"),
                    y: i32_of(c, r, "Y"),
                    direction: i32_of(c, r, "Direction") as u8,
                })
                .collect(),
            None => Vec::new(),
        };

        let currencies = match db.collection("CurrencyInfo") {
            Some(c) => c
                .records
                .iter()
                .map(|r| CurrencyDef {
                    index: c.index(r),
                    name: c.str_or(r, "Name", "").to_string(),
                    abbreviation: c.str_or(r, "Abbreviation", "").to_string(),
                    drop_item: i32_of(c, r, "DropItem"),
                    exchange_rate: c.float_or(r, "ExchangeRate", 1.0),
                })
                .collect(),
            None => Vec::new(),
        };
        let mut quests: HashMap<i32, QuestDef> = match db.collection("QuestInfo") {
            Some(c) => c
                .records
                .iter()
                .map(|r| {
                    let q = QuestDef {
                        index: c.index(r),
                        name: c.str_or(r, "QuestName", "").to_string(),
                        quest_type: i32_of(c, r, "QuestType"),
                        start_npc: i32_of(c, r, "StartNPC"),
                        finish_npc: i32_of(c, r, "FinishNPC"),
                        requirements: Vec::new(),
                        tasks: Vec::new(),
                        rewards: Vec::new(),
                    };
                    (q.index, q)
                })
                .collect(),
            None => HashMap::new(),
        };
        if let Some(c) = db.collection("QuestRequirement") {
            for r in &c.records {
                if let Some(q) = quests.get_mut(&i32_of(c, r, "Quest")) {
                    q.requirements.push(QuestRequirementDef {
                        requirement: i32_of(c, r, "Requirement"),
                        int1: i32_of(c, r, "IntParameter1"),
                        quest: i32_of(c, r, "QuestParameter"),
                        class: i32_of(c, r, "Class") as u8,
                    });
                }
            }
        }
        if let Some(c) = db.collection("QuestTask") {
            for r in &c.records {
                if let Some(q) = quests.get_mut(&i32_of(c, r, "Quest")) {
                    q.tasks.push(QuestTaskDef {
                        index: c.index(r),
                        task: i32_of(c, r, "Task"),
                        item: i32_of(c, r, "ItemParameter"),
                        region: i32_of(c, r, "RegionParameter"),
                        amount: i32_of(c, r, "Amount"),
                        monsters: Vec::new(),
                    });
                }
            }
        }
        if let Some(c) = db.collection("QuestTaskMonsterDetails") {
            for r in &c.records {
                let task = i32_of(c, r, "Task");
                for q in quests.values_mut() {
                    if let Some(t) = q.tasks.iter_mut().find(|t| t.index == task) {
                        t.monsters.push(QuestMonsterDef {
                            monster: i32_of(c, r, "Monster"),
                            map: i32_of(c, r, "Map"),
                            chance: i32_of(c, r, "Chance"),
                            amount: i32_of(c, r, "Amount"),
                        });
                    }
                }
            }
        }
        if let Some(c) = db.collection("QuestReward") {
            for r in &c.records {
                if let Some(q) = quests.get_mut(&i32_of(c, r, "Quest")) {
                    q.rewards.push(QuestRewardDef {
                        index: c.index(r),
                        item: i32_of(c, r, "Item"),
                        amount: i32_of(c, r, "Amount"),
                        choice: c.bool_or(r, "Choice", false),
                        bound: c.bool_or(r, "Bound", false),
                        class: c.int_or(r, "Class", 15) as u8,
                    });
                }
            }
        }
        if let Some(c) = db.collection("NPCRequirement") {
            for r in &c.records {
                let npc = i32_of(c, r, "NPC");
                if let Some(n) = npcs.iter_mut().find(|n| n.index == npc) {
                    n.requirements.push(NpcRequirementDef {
                        requirement: i32_of(c, r, "Requirement"),
                        int1: i32_of(c, r, "IntParameter1"),
                        quest: i32_of(c, r, "QuestParameter"),
                        class: i32_of(c, r, "Class") as u8,
                        days: i32_of(c, r, "DayOfWeek"),
                    });
                }
            }
        }

        Ok(GameData {
            maps,
            regions,
            respawns,
            monsters,
            safe_zones,
            guards,
            currencies,
            quests,
            base_stats,
            npcs,
            items,
            drops,
            npc_pages,
            movements,
            gold_item,
            magics,
        })
    }

    /// Consistency problems the loader tolerates but the game would trip on:
    /// dangling references and spawn regions with no walkable cell. `map_dir`
    /// is the client `Map/` folder; map files that fail to load are reported.
    pub fn validate(&self, map_dir: &Path) -> Vec<Problem> {
        let mut out = Vec::new();
        let mut map_files: HashMap<i32, Option<mir_formats::MapFile>> = HashMap::new();
        fn map_file<'a>(
            cache: &'a mut HashMap<i32, Option<mir_formats::MapFile>>,
            data: &GameData,
            map_dir: &Path,
            index: i32,
            out: &mut Vec<Problem>,
        ) -> Option<&'a mir_formats::MapFile> {
            if let std::collections::hash_map::Entry::Vacant(slot) = cache.entry(index) {
                let loaded = data.maps.get(&index).and_then(|m| {
                    match mir_formats::MapFile::load(map_dir.join(format!("{}.map", m.file_name))) {
                        Ok(f) => Some(f),
                        Err(e) => {
                            out.push(Problem::new(
                                "MapInfo",
                                index,
                                format!("map file {}.map: {e}", m.file_name),
                            ));
                            None
                        }
                    }
                });
                slot.insert(loaded);
            }
            cache[&index].as_ref()
        }
        for r in self.regions.values() {
            if !self.maps.contains_key(&r.map) {
                out.push(Problem::new(
                    "MapRegion",
                    r.index,
                    format!("map {} does not exist", r.map),
                ));
            }
        }
        for sp in &self.respawns {
            if !self.monsters.contains_key(&sp.monster) {
                out.push(Problem::new(
                    "RespawnInfo",
                    sp.index,
                    format!("monster {} does not exist", sp.monster),
                ));
            }
            let Some(region) = self.regions.get(&sp.region) else {
                out.push(Problem::new(
                    "RespawnInfo",
                    sp.index,
                    format!("region {} does not exist", sp.region),
                ));
                continue;
            };
            let Some(file) = map_file(&mut map_files, self, map_dir, region.map, &mut out) else {
                continue;
            };
            let walkable = region
                .points(file.width as i32)
                .iter()
                .any(|(x, y)| file.is_walkable(*x, *y));
            if !walkable {
                out.push(Problem::new(
                    "RespawnInfo",
                    sp.index,
                    format!(
                        "region {} ({}) has no walkable cell",
                        sp.region, region.description
                    ),
                ));
            }
        }
        for sz in &self.safe_zones {
            if !self.regions.contains_key(&sz.region) {
                out.push(Problem::new(
                    "SafeZoneInfo",
                    sz.index,
                    format!("region {} does not exist", sz.region),
                ));
            }
        }
        for mv in &self.movements {
            for (what, region) in [
                ("source", mv.source_region),
                ("destination", mv.destination_region),
            ] {
                if !self.regions.contains_key(&region) {
                    out.push(Problem::new(
                        "MovementInfo",
                        mv.index,
                        format!("{what} region {region} does not exist"),
                    ));
                }
            }
        }
        for (i, d) in self.drops.iter().enumerate() {
            if !self.monsters.contains_key(&d.monster) {
                out.push(Problem::new(
                    "DropInfo",
                    i as i32,
                    format!("monster {} does not exist", d.monster),
                ));
            }
            if !self.items.contains_key(&d.item) {
                out.push(Problem::new(
                    "DropInfo",
                    i as i32,
                    format!("item {} does not exist", d.item),
                ));
            }
        }
        for n in &self.npcs {
            if !self.regions.contains_key(&n.region) {
                out.push(Problem::new(
                    "NPCInfo",
                    n.index,
                    format!("region {} does not exist", n.region),
                ));
            }
        }
        for item in self.items.values() {
            if item.item_type == 14 && self.magic_by_index(item.shape).is_none() {
                out.push(Problem::new(
                    "ItemInfo",
                    item.index,
                    format!("book points at missing magic {}", item.shape),
                ));
            }
        }
        out
    }

    /// Highest base-stat row with `level <= wanted` for the class (Zircon `AddBaseStats`).
    pub fn base_stat(&self, class: u8, level: i32) -> Option<&BaseStatDef> {
        self.base_stats
            .iter()
            .filter(|b| b.class == class && b.level <= level)
            .max_by_key(|b| b.level)
    }

    /// Safe zones a new character of this class may start in.
    pub fn start_zones(&self, class_flag: u8) -> Vec<&SafeZoneDef> {
        self.safe_zones
            .iter()
            .filter(|z| z.start_class & class_flag != 0 && z.bind_region != 0)
            .collect()
    }

    pub fn map_by_file(&self, file: &str) -> Option<&MapDef> {
        self.maps.values().find(|m| m.file_name == file)
    }

    pub fn max_experience(level: i32) -> u64 {
        EXPERIENCE.get(level.max(0) as usize).copied().unwrap_or(0)
    }
}

/// Class flags of a magic: new databases carry `RequiredClass` flags, old
/// ones a `Class` byte (0 Warrior .. 3 Assassin, 4 All).
fn magic_class_flags(c: &mir_formats::mirdb::Collection, r: &mir_formats::mirdb::Record) -> u8 {
    if c.has_property("RequiredClass") {
        return i32_of(c, r, "RequiredClass") as u8;
    }
    match i32_of(c, r, "Class") {
        0 => 1,
        1 => 2,
        2 => 4,
        3 => 8,
        _ => 15,
    }
}
