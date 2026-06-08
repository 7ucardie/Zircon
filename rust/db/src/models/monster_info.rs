//! MonsterInfo — monster definitions.

use crate::{
    binary::BinReader,
    error::DbError,
    mapping::{skip_value, PropDef},
};

/// Mirrors `Library.SystemModels.MonsterInfo`.
#[derive(Debug, Default, Clone)]
pub struct MonsterInfo {
    pub index: i32,
    pub monster_name: String,
    /// MonsterImage enum (int32 underlying — no `: byte` in C#)
    pub image: i32,
    pub ai: i32,
    /// MonsterBehaviour enum (int32 underlying)
    pub behaviours: i32,
    pub level: i32,
    pub view_range: i32,
    pub cool_eye: i32,
    /// Experience decimal — [lo, mid, hi, flags] from Decimal.GetBits()
    pub experience: [i32; 4],
    pub undead: bool,
    pub can_push: bool,
    pub can_tame: bool,
    pub attack_delay: i32,
    pub move_delay: i32,
    pub is_boss: bool,
    /// MonsterFlag enum (int32 underlying)
    pub flag: i32,
    pub face_image: i32,
}

impl MonsterInfo {
    pub fn from_props(r: &mut BinReader<'_>, props: &[PropDef]) -> Result<Self, DbError> {
        let mut obj = MonsterInfo::default();
        for prop in props {
            match prop.name.as_str() {
                "Index"       => obj.index        = r.read_i32()?,
                "MonsterName" => obj.monster_name = r.read_string()?,
                "Image"       => obj.image        = r.read_i32()?,
                "AI"          => obj.ai           = r.read_i32()?,
                "Behaviours"  => obj.behaviours   = r.read_i32()?,
                "Level"       => obj.level        = r.read_i32()?,
                "ViewRange"   => obj.view_range   = r.read_i32()?,
                "CoolEye"     => obj.cool_eye     = r.read_i32()?,
                "Experience"  => obj.experience   = r.read_decimal()?,
                "Undead"      => obj.undead       = r.read_bool()?,
                "CanPush"     => obj.can_push     = r.read_bool()?,
                "CanTame"     => obj.can_tame     = r.read_bool()?,
                "AttackDelay" => obj.attack_delay = r.read_i32()?,
                "MoveDelay"   => obj.move_delay   = r.read_i32()?,
                "IsBoss"      => obj.is_boss      = r.read_bool()?,
                "Flag"        => obj.flag         = r.read_i32()?,
                "FaceImage"   => obj.face_image   = r.read_i32()?,
                _ => skip_value(&prop.type_name, r)?,
            }
        }
        Ok(obj)
    }
}
