//! Game enumerations used in packet fields.
//!
//! Each enum implements WireRead / WireWrite using its underlying integer type.
//! Unknown discriminants received over the wire are treated as protocol errors.

use bytes::{Buf, BytesMut};

use crate::{error::ProtocolError, wire::{WireRead, WireWrite}};

macro_rules! wire_enum_u8 {
    ($name:ident { $first_variant:ident = $first_val:expr $(, $variant:ident = $val:expr )* $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(u8)]
        pub enum $name { $first_variant = $first_val, $( $variant = $val, )* }

        impl Default for $name {
            fn default() -> Self { Self::$first_variant }
        }
        impl WireRead for $name {
            fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
                let v = u8::wire_read(buf)?;
                match v {
                    $first_val => Ok(Self::$first_variant),
                    $( $val => Ok(Self::$variant), )*
                    _ => Err(ProtocolError::UnknownEnumValue(v as i32)),
                }
            }
        }
        impl WireWrite for $name {
            fn wire_write(&self, buf: &mut BytesMut) {
                (*self as u8).wire_write(buf);
            }
        }
    };
}

macro_rules! wire_enum_i32 {
    ($name:ident { $first_variant:ident = $first_val:expr $(, $variant:ident = $val:expr )* $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(i32)]
        pub enum $name { $first_variant = $first_val, $( $variant = $val, )* }

        impl Default for $name {
            fn default() -> Self { Self::$first_variant }
        }
        impl WireRead for $name {
            fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
                let v = i32::wire_read(buf)?;
                match v {
                    $first_val => Ok(Self::$first_variant),
                    $( $val => Ok(Self::$variant), )*
                    _ => Err(ProtocolError::UnknownEnumValue(v)),
                }
            }
        }
        impl WireWrite for $name {
            fn wire_write(&self, buf: &mut BytesMut) {
                (*self as i32).wire_write(buf);
            }
        }
    };
}

// ── Small byte-sized enums ─────────────────────────────────────────────────

wire_enum_u8!(MirGender { Male = 0, Female = 1 });

wire_enum_u8!(MirClass {
    Warrior = 0,
    Wizard  = 1,
    Taoist  = 2,
    Assassin = 3,
});

wire_enum_u8!(AttackMode {
    Peace      = 0,
    Group      = 1,
    Guild      = 2,
    WarRedBrown = 3,
    All        = 4,
});

wire_enum_u8!(PetMode {
    Both   = 0,
    Move   = 1,
    Attack = 2,
    PvP    = 3,
    None   = 4,
});

wire_enum_u8!(MirDirection {
    Up        = 0,
    UpRight   = 1,
    Right     = 2,
    DownRight = 3,
    Down      = 4,
    DownLeft  = 5,
    Left      = 6,
    UpLeft    = 7,
});

wire_enum_u8!(Rarity { Common = 0, Superior = 1, Elite = 2 });

wire_enum_u8!(HorseType {
    None       = 0,
    Brown      = 1,
    White      = 2,
    Red        = 3,
    Black      = 4,
    WhiteUnicorn = 5,
    RedUnicorn = 6,
});

wire_enum_u8!(OnlineState { Online = 0, Busy = 1, Away = 2, Offline = 3 });

wire_enum_u8!(TimeOfDay { Dawn = 0, Day = 1, Dusk = 2, Night = 3 });

wire_enum_u8!(FishingState {
    None    = 0,
    Casting = 1,
    Waiting = 2,
    Reeling = 3,
    Done    = 4,
});

wire_enum_u8!(RefineType {
    None      = 0,
    BlackIron = 1,
    Silver    = 2,
    Diamond   = 3,
    Gold      = 4,
    Corundum  = 5,
});

wire_enum_u8!(RefineQuality {
    None    = 0,
    Normal  = 1,
    Superior = 2,
    Elite   = 3,
});

wire_enum_u8!(Element {
    None    = 0,
    Fire    = 1,
    Ice     = 2,
    Lightning = 3,
    Wind    = 4,
    Holy    = 5,
    Dark    = 6,
    Phantom = 7,
});

// ExteriorEffect has many values; represent as raw u8 to stay compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ExteriorEffect(pub u8);
impl WireRead for ExteriorEffect {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(ExteriorEffect(u8::wire_read(buf)?))
    }
}
impl WireWrite for ExteriorEffect {
    fn wire_write(&self, buf: &mut BytesMut) { self.0.wire_write(buf); }
}

wire_enum_u8!(ItemType {
    Nothing   = 0,
    Consumable = 1,
    Weapon    = 2,
    Armour    = 3,
    Helmet    = 4,
    Necklace  = 5,
    Bracelet  = 6,
    Ring      = 7,
    Shoes     = 8,
    Torch     = 9,
    Poison    = 10,
    Amulet    = 11,
    Flower    = 12,
    HorseArmour = 13,
    Emblem    = 14,
    Shield    = 15,
    Costume   = 16,
    Hook      = 17,
    Float     = 18,
    Bait      = 19,
    Finder    = 20,
    Reel      = 21,
    Currency  = 22,
    Bundle    = 23,
    LootBox   = 24,
});

// ── i32-based enums ────────────────────────────────────────────────────────

wire_enum_i32!(GridType {
    None                         = 0,
    Inventory                    = 1,
    Equipment                    = 2,
    Belt                         = 3,
    Repair                       = 4,
    Storage                      = 5,
    AutoPotion                   = 6,
    RefineBlackIronOre           = 7,
    RefineAccessory              = 8,
    RefineSpecial                = 9,
    Inspect                      = 10,
    Consign                      = 11,
    SendMail                     = 12,
    TradeUser                    = 13,
    TradePlayer                  = 14,
    GuildStorage                 = 15,
    CompanionInventory           = 16,
    CompanionEquipment           = 17,
    WeddingRing                  = 18,
    RefinementStoneIronOre       = 19,
    RefinementStoneSilverOre     = 20,
    RefinementStoneDiamond       = 21,
    RefinementStoneGoldOre       = 22,
    RefinementStoneCrystal       = 23,
    ItemFragment                 = 24,
    AccessoryRefineUpgradeTarget = 25,
    AccessoryRefineLevelTarget   = 26,
    AccessoryRefineLevelItems    = 27,
    MasterRefineFragment1        = 28,
    MasterRefineFragment2        = 29,
    MasterRefineFragment3        = 30,
    MasterRefineStone            = 31,
    MasterRefineSpecial          = 32,
    AccessoryReset               = 33,
    WeaponCraftTemplate          = 34,
    WeaponCraftYellow            = 35,
    WeaponCraftBlue              = 36,
    WeaponCraftRed               = 37,
    WeaponCraftPurple            = 38,
    WeaponCraftGreen             = 39,
    WeaponCraftGrey              = 40,
    RefineCorundumOre            = 41,
    AccessoryRefineCombTarget    = 42,
    AccessoryRefineCombItems     = 43,
    PartsStorage                 = 44,
    Bundle                       = 45,
    LootBox                      = 46,
});

// Large enums (MagicType, BuffType, Effect, PoisonType) have many variants.
// Use raw i32 wrappers for forward compatibility.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MagicType(pub i32);
impl WireRead for MagicType {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(MagicType(i32::wire_read(buf)?))
    }
}
impl WireWrite for MagicType {
    fn wire_write(&self, buf: &mut BytesMut) { self.0.wire_write(buf); }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BuffType(pub i32);
impl WireRead for BuffType {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(BuffType(i32::wire_read(buf)?))
    }
}
impl WireWrite for BuffType {
    fn wire_write(&self, buf: &mut BytesMut) { self.0.wire_write(buf); }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PoisonType(pub i32);
impl WireRead for PoisonType {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(PoisonType(i32::wire_read(buf)?))
    }
}
impl WireWrite for PoisonType {
    fn wire_write(&self, buf: &mut BytesMut) { self.0.wire_write(buf); }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Effect(pub i32);
impl WireRead for Effect {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(Effect(i32::wire_read(buf)?))
    }
}
impl WireWrite for Effect {
    fn wire_write(&self, buf: &mut BytesMut) { self.0.wire_write(buf); }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SpellEffect(pub i32);
impl WireRead for SpellEffect {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(SpellEffect(i32::wire_read(buf)?))
    }
}
impl WireWrite for SpellEffect {
    fn wire_write(&self, buf: &mut BytesMut) { self.0.wire_write(buf); }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SpellKey(pub u8);
impl WireRead for SpellKey {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(SpellKey(u8::wire_read(buf)?))
    }
}
impl WireWrite for SpellKey {
    fn wire_write(&self, buf: &mut BytesMut) { self.0.wire_write(buf); }
}

// UserItemFlags is [Flags] int
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct UserItemFlags(pub i32);
impl WireRead for UserItemFlags {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(UserItemFlags(i32::wire_read(buf)?))
    }
}
impl WireWrite for UserItemFlags {
    fn wire_write(&self, buf: &mut BytesMut) { self.0.wire_write(buf); }
}

// GuildPermission is [Flags] int
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GuildPermission(pub i32);
impl WireRead for GuildPermission {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(GuildPermission(i32::wire_read(buf)?))
    }
}
impl WireWrite for GuildPermission {
    fn wire_write(&self, buf: &mut BytesMut) { self.0.wire_write(buf); }
}

// RequiredClass is [Flags] byte
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct RequiredClass(pub u8);
impl WireRead for RequiredClass {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(RequiredClass(u8::wire_read(buf)?))
    }
}
impl WireWrite for RequiredClass {
    fn wire_write(&self, buf: &mut BytesMut) { self.0.wire_write(buf); }
}

// MessageType (i32)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MessageType(pub i32);
impl WireRead for MessageType {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(MessageType(i32::wire_read(buf)?))
    }
}
impl WireWrite for MessageType {
    fn wire_write(&self, buf: &mut BytesMut) { self.0.wire_write(buf); }
}

// MirAction (byte)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MirAction(pub u8);
impl WireRead for MirAction {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(MirAction(u8::wire_read(buf)?))
    }
}
impl WireWrite for MirAction {
    fn wire_write(&self, buf: &mut BytesMut) { self.0.wire_write(buf); }
}

// MarketPlaceSort (i32)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MarketPlaceSort(pub i32);
impl WireRead for MarketPlaceSort {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(MarketPlaceSort(i32::wire_read(buf)?))
    }
}
impl WireWrite for MarketPlaceSort {
    fn wire_write(&self, buf: &mut BytesMut) { self.0.wire_write(buf); }
}

// ── Result enums (all byte-sized) ─────────────────────────────────────────

wire_enum_u8!(DisconnectReason {
    Unknown              = 0,
    ServerShutdown       = 1,
    ConnectionLost       = 2,
    DuplicateLogin       = 3,
    BadVersion           = 4,
    TempBan              = 5,
    PermBan              = 6,
    Maintenance          = 7,
    InternalError        = 8,
    CharacterNotFound    = 9,
});

wire_enum_u8!(NewAccountResult {
    Success             = 0,
    DuplicateAddress    = 1,
    Banned              = 2,
    BadEMail            = 3,
    BadPassword         = 4,
    BadRealName         = 5,
    BadReferral         = 6,
    Error               = 7,
});

wire_enum_u8!(ChangePasswordResult {
    Success             = 0,
    AccountNotFound     = 1,
    WrongPassword       = 2,
    BadNewPassword      = 3,
    Delay               = 4,
    Error               = 5,
});

wire_enum_u8!(RequestPasswordResetResult {
    Success             = 0,
    AccountNotFound     = 1,
    Delay               = 2,
    Error               = 3,
});

wire_enum_u8!(ResetPasswordResult {
    Success             = 0,
    BadKey              = 1,
    BadPassword         = 2,
    Error               = 3,
});

wire_enum_u8!(ActivationResult {
    Success             = 0,
    BadKey              = 1,
    AlreadyActivated    = 2,
    Error               = 3,
});

wire_enum_u8!(RequestActivationKeyResult {
    Success             = 0,
    AccountNotFound     = 1,
    AlreadyActivated    = 2,
    Delay               = 3,
    Error               = 4,
});

wire_enum_u8!(LoginResult {
    Success             = 0,
    AccountNotFound     = 1,
    WrongPassword       = 2,
    NotActivated        = 3,
    AlreadyLoggedIn     = 4,
    Banned              = 5,
    Delay               = 6,
    Error               = 7,
    CheckSum            = 8,
});

wire_enum_u8!(NewCharacterResult {
    Success             = 0,
    Disabled            = 1,
    EMailNotVerified    = 2,
    BadCharacterName    = 3,
    DuplicateCharacterName = 4,
    MaxCharacters       = 5,
    Error               = 6,
});

wire_enum_u8!(DeleteCharacterResult {
    Success             = 0,
    CharacterNotFound   = 1,
    Error               = 2,
});

wire_enum_u8!(StartGameResult {
    Success             = 0,
    CharacterNotFound   = 1,
    Blocked             = 2,
    Delay               = 3,
    Error               = 4,
});

// InstanceResult has many values; raw i32 wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct InstanceResult(pub i32);
impl WireRead for InstanceResult {
    fn wire_read(buf: &mut impl Buf) -> Result<Self, ProtocolError> {
        Ok(InstanceResult(i32::wire_read(buf)?))
    }
}
impl WireWrite for InstanceResult {
    fn wire_write(&self, buf: &mut BytesMut) { self.0.wire_write(buf); }
}

// Stat (i32 underlying) — used as BTreeMap keys in Stats
pub type StatKey = i32;
