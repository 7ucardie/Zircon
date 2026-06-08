//! MapInfo — map definitions.

use crate::{
    binary::BinReader,
    error::DbError,
    mapping::{skip_value, PropDef},
};

/// Mirrors `Library.SystemModels.MapInfo`.
#[derive(Debug, Default, Clone)]
pub struct MapInfo {
    pub index: i32,
    pub file_name: String,
    pub description: String,
    pub mini_map: i32,
    /// LightSetting enum (byte underlying)
    pub light: u8,
    /// Weather enum (int32 underlying)
    pub weather: i32,
    /// FightSetting enum (byte underlying)
    pub fight: u8,
    pub allow_rt: bool,
    pub skill_delay: i32,
    pub can_horse: bool,
    pub allow_tt: bool,
    pub can_mine: bool,
    pub can_marriage_recall: bool,
    pub allow_recall: bool,
    pub minimum_level: i32,
    pub maximum_level: i32,
    /// FK index into MapInfo collection (0 = none)
    pub reconnect_map_index: i32,
    /// SoundIndex enum (int32 underlying)
    pub music: i32,
    pub background: i32,
    pub monster_health: i32,
    pub monster_damage: i32,
    pub drop_rate: i32,
    pub experience_rate: i32,
    pub gold_rate: i32,
    pub max_monster_health: i32,
    pub max_monster_damage: i32,
    pub max_drop_rate: i32,
    pub max_experience_rate: i32,
    pub max_gold_rate: i32,
    /// FK index into InstanceInfo collection (0 = none)
    pub instance_index: i32,
    /// RequiredClass enum (byte underlying)
    pub required_class: u8,
}

impl MapInfo {
    pub fn from_props(r: &mut BinReader<'_>, props: &[PropDef]) -> Result<Self, DbError> {
        let mut obj = MapInfo::default();
        for prop in props {
            match prop.name.as_str() {
                "Index"              => obj.index              = r.read_i32()?,
                "FileName"           => obj.file_name          = r.read_string()?,
                "Description"        => obj.description        = r.read_string()?,
                "MiniMap"            => obj.mini_map           = r.read_i32()?,
                "Light"              => obj.light              = r.read_u8()?,
                "Weather"            => obj.weather            = r.read_i32()?,
                "Fight"              => obj.fight              = r.read_u8()?,
                "AllowRT"            => obj.allow_rt           = r.read_bool()?,
                "SkillDelay"         => obj.skill_delay        = r.read_i32()?,
                "CanHorse"           => obj.can_horse          = r.read_bool()?,
                "AllowTT"            => obj.allow_tt           = r.read_bool()?,
                "CanMine"            => obj.can_mine           = r.read_bool()?,
                "CanMarriageRecall"  => obj.can_marriage_recall = r.read_bool()?,
                "AllowRecall"        => obj.allow_recall       = r.read_bool()?,
                "MinimumLevel"       => obj.minimum_level      = r.read_i32()?,
                "MaximumLevel"       => obj.maximum_level      = r.read_i32()?,
                "ReconnectMap"       => obj.reconnect_map_index = r.read_i32()?,
                "Music"              => obj.music              = r.read_i32()?,
                "Background"         => obj.background         = r.read_i32()?,
                "MonsterHealth"      => obj.monster_health     = r.read_i32()?,
                "MonsterDamage"      => obj.monster_damage     = r.read_i32()?,
                "DropRate"           => obj.drop_rate          = r.read_i32()?,
                "ExperienceRate"     => obj.experience_rate    = r.read_i32()?,
                "GoldRate"           => obj.gold_rate          = r.read_i32()?,
                "MaxMonsterHealth"   => obj.max_monster_health = r.read_i32()?,
                "MaxMonsterDamage"   => obj.max_monster_damage = r.read_i32()?,
                "MaxDropRate"        => obj.max_drop_rate      = r.read_i32()?,
                "MaxExperienceRate"  => obj.max_experience_rate = r.read_i32()?,
                "MaxGoldRate"        => obj.max_gold_rate      = r.read_i32()?,
                "Instance"           => obj.instance_index     = r.read_i32()?,
                "RequiredClass"      => obj.required_class     = r.read_u8()?,
                _ => skip_value(&prop.type_name, r)?,
            }
        }
        Ok(obj)
    }
}
