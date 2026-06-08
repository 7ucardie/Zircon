//! ItemInfo — game item definitions.

use crate::{
    binary::BinReader,
    error::DbError,
    mapping::{skip_value, PropDef},
};

/// Mirrors `Library.SystemModels.ItemInfo`.
///
/// Enum fields are stored as their underlying primitive (u8 or i32).
/// FK fields (e.g. `set_index`) store the DBObject.Index of the related record.
/// Decimal fields (e.g. `sell_rate`) are stored as four i32s from Decimal.GetBits().
#[derive(Debug, Default, Clone)]
pub struct ItemInfo {
    pub index: i32,
    pub item_name: String,
    /// ItemType enum (byte underlying)
    pub item_type: u8,
    /// RequiredClass enum (byte underlying)
    pub required_class: u8,
    /// RequiredGender enum (byte underlying)
    pub required_gender: u8,
    /// RequiredType enum (byte underlying)
    pub required_type: u8,
    pub required_amount: i32,
    pub shape: i32,
    /// ItemEffect enum (byte, obsolete duplicate)
    pub effect: u8,
    /// ItemEffect enum (byte)
    pub item_effect: u8,
    /// ExteriorEffect enum (byte)
    pub exterior_effect: u8,
    pub image: i32,
    pub durability: i32,
    pub price: i32,
    pub weight: i32,
    pub stack_size: i32,
    pub start_item: bool,
    /// SellRate decimal — stored as [lo, mid, hi, flags] from Decimal.GetBits()
    pub sell_rate: [i32; 4],
    pub can_repair: bool,
    pub can_sell: bool,
    pub can_store: bool,
    pub can_trade: bool,
    pub can_drop: bool,
    pub can_death_drop: bool,
    pub description: String,
    /// Rarity enum (byte underlying)
    pub rarity: u8,
    pub can_auto_pot: bool,
    pub buff_icon: i32,
    pub part_count: i32,
    /// FK index into SetInfo collection (0 = no set)
    pub set_index: i32,
}

impl ItemInfo {
    /// Deserialise one object's RawData using the mapping from the file.
    pub fn from_props(r: &mut BinReader<'_>, props: &[PropDef]) -> Result<Self, DbError> {
        let mut obj = ItemInfo::default();
        for prop in props {
            match prop.name.as_str() {
                "Index"           => obj.index           = r.read_i32()?,
                "ItemName"        => obj.item_name        = r.read_string()?,
                "ItemType"        => obj.item_type        = r.read_u8()?,
                "RequiredClass"   => obj.required_class   = r.read_u8()?,
                "RequiredGender"  => obj.required_gender  = r.read_u8()?,
                "RequiredType"    => obj.required_type    = r.read_u8()?,
                "RequiredAmount"  => obj.required_amount  = r.read_i32()?,
                "Shape"           => obj.shape            = r.read_i32()?,
                "Effect"          => obj.effect           = r.read_u8()?,
                "ItemEffect"      => obj.item_effect      = r.read_u8()?,
                "ExteriorEffect"  => obj.exterior_effect  = r.read_u8()?,
                "Image"           => obj.image            = r.read_i32()?,
                "Durability"      => obj.durability       = r.read_i32()?,
                "Price"           => obj.price            = r.read_i32()?,
                "Weight"          => obj.weight           = r.read_i32()?,
                "StackSize"       => obj.stack_size       = r.read_i32()?,
                "StartItem"       => obj.start_item       = r.read_bool()?,
                "SellRate"        => obj.sell_rate        = r.read_decimal()?,
                "CanRepair"       => obj.can_repair       = r.read_bool()?,
                "CanSell"         => obj.can_sell         = r.read_bool()?,
                "CanStore"        => obj.can_store        = r.read_bool()?,
                "CanTrade"        => obj.can_trade        = r.read_bool()?,
                "CanDrop"         => obj.can_drop         = r.read_bool()?,
                "CanDeathDrop"    => obj.can_death_drop   = r.read_bool()?,
                "Description"     => obj.description      = r.read_string()?,
                "Rarity"          => obj.rarity           = r.read_u8()?,
                "CanAutoPot"      => obj.can_auto_pot     = r.read_bool()?,
                "BuffIcon"        => obj.buff_icon        = r.read_i32()?,
                "PartCount"       => obj.part_count       = r.read_i32()?,
                "Set"             => obj.set_index        = r.read_i32()?,
                _ => skip_value(&prop.type_name, r)?,
            }
        }
        Ok(obj)
    }
}
