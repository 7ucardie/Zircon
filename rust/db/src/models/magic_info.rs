//! MagicInfo — spell / magic definitions.

use crate::{
    binary::BinReader,
    error::DbError,
    mapping::{skip_value, PropDef},
};

/// Mirrors `Library.SystemModels.MagicInfo`.
#[derive(Debug, Default, Clone)]
pub struct MagicInfo {
    pub index: i32,
    pub name: String,
    /// MagicType enum (int32 underlying — no `: byte`)
    pub magic: i32,
    /// MirClass enum (byte underlying)
    pub class: u8,
    /// MagicSchool enum — stored as i32 (assumed int32)
    pub school: i32,
    /// MagicProperty enum — stored as i32 (assumed int32)
    pub property: i32,
    pub icon: i32,
    pub min_base_power: i32,
    pub max_base_power: i32,
    pub min_level_power: i32,
    pub max_level_power: i32,
    pub base_cost: i32,
    pub level_cost: i32,
    pub need_level1: i32,
    pub need_level2: i32,
    pub need_level3: i32,
    pub experience1: i32,
    pub experience2: i32,
    pub experience3: i32,
    pub delay: i32,
    pub description: String,
}

impl MagicInfo {
    pub fn from_props(r: &mut BinReader<'_>, props: &[PropDef]) -> Result<Self, DbError> {
        let mut obj = MagicInfo::default();
        for prop in props {
            match prop.name.as_str() {
                "Index"         => obj.index          = r.read_i32()?,
                "Name"          => obj.name           = r.read_string()?,
                "Magic"         => obj.magic          = r.read_i32()?,
                "Class"         => obj.class          = r.read_u8()?,
                "School"        => obj.school         = r.read_i32()?,
                "Property"      => obj.property       = r.read_i32()?,
                "Icon"          => obj.icon           = r.read_i32()?,
                "MinBasePower"  => obj.min_base_power  = r.read_i32()?,
                "MaxBasePower"  => obj.max_base_power  = r.read_i32()?,
                "MinLevelPower" => obj.min_level_power = r.read_i32()?,
                "MaxLevelPower" => obj.max_level_power = r.read_i32()?,
                "BaseCost"      => obj.base_cost      = r.read_i32()?,
                "LevelCost"     => obj.level_cost     = r.read_i32()?,
                "NeedLevel1"    => obj.need_level1    = r.read_i32()?,
                "NeedLevel2"    => obj.need_level2    = r.read_i32()?,
                "NeedLevel3"    => obj.need_level3    = r.read_i32()?,
                "Experience1"   => obj.experience1    = r.read_i32()?,
                "Experience2"   => obj.experience2    = r.read_i32()?,
                "Experience3"   => obj.experience3    = r.read_i32()?,
                "Delay"         => obj.delay          = r.read_i32()?,
                "Description"   => obj.description    = r.read_string()?,
                _ => skip_value(&prop.type_name, r)?,
            }
        }
        Ok(obj)
    }
}
