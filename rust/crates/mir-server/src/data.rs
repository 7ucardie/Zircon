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
}

#[derive(Debug, Clone)]
pub struct NpcDef {
    pub index: i32,
    pub region: i32,
    pub name: String,
    pub image: i32,
}

/// Zircon `Globals.ExperienceList`: experience needed at each level.
pub const EXPERIENCE: [u64; 41] = [
    0, 100, 200, 300, 400, 600, 900, 1200, 1700, 2500, 6000, 8000, 10000, 15000, 30000, 40000,
    50000, 70000, 100000, 120000, 140000, 250000, 300000, 350000, 400000, 500000, 700000, 1000000,
    1400000, 1800000, 2000000, 2400000, 2800000, 3200000, 3600000, 4000000, 4800000, 5600000,
    8200000, 9000000, 11000000,
];

#[derive(Debug)]
pub struct GameData {
    pub maps: BTreeMap<i32, MapDef>,
    pub regions: HashMap<i32, RegionDef>,
    pub respawns: Vec<RespawnDef>,
    pub monsters: HashMap<i32, MonsterDef>,
    pub safe_zones: Vec<SafeZoneDef>,
    pub base_stats: Vec<BaseStatDef>,
    pub npcs: Vec<NpcDef>,
}

fn i32_of(c: &Collection, r: &Record, name: &str) -> i32 {
    c.int_or(r, name, 0) as i32
}

impl GameData {
    pub fn load(path: impl AsRef<Path>) -> Result<GameData> {
        let db = MirDb::load(path.as_ref())
            .with_context(|| format!("loading {}", path.as_ref().display()))?;
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
            })
            .collect();

        let npcs = match db.collection("NPCInfo") {
            Some(c) => c
                .records
                .iter()
                .map(|r| NpcDef {
                    index: c.index(r),
                    region: i32_of(c, r, "Region"),
                    name: c.str_or(r, "NPCName", "").to_string(),
                    image: i32_of(c, r, "Image"),
                })
                .collect(),
            None => Vec::new(),
        };

        Ok(GameData {
            maps,
            regions,
            respawns,
            monsters,
            safe_zones,
            base_stats,
            npcs,
        })
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
