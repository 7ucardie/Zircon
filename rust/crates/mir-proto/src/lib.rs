//! Wire protocol shared by the Rust Zircon client and server.
//!
//! Every frame on the TCP stream is `u32 little-endian payload length` followed
//! by a [postcard](https://docs.rs/postcard) encoded [`ClientMessage`] or
//! [`ServerMessage`]. Both ends own this crate, so there is no need to match
//! the C# reflection-ordered packet ids.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;
pub const DEFAULT_PORT: u16 = 7000;
pub const MAX_FRAME_LEN: usize = 1 << 20;

/// Eight-way direction, in the same order Zircon's sprite sheets use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Direction {
    Up = 0,
    UpRight = 1,
    Right = 2,
    DownRight = 3,
    Down = 4,
    DownLeft = 5,
    Left = 6,
    UpLeft = 7,
}

impl Direction {
    pub const ALL: [Direction; 8] = [
        Direction::Up,
        Direction::UpRight,
        Direction::Right,
        Direction::DownRight,
        Direction::Down,
        Direction::DownLeft,
        Direction::Left,
        Direction::UpLeft,
    ];

    pub fn from_index(i: u8) -> Direction {
        Direction::ALL[(i & 7) as usize]
    }

    pub fn index(self) -> u8 {
        self as u8
    }

    /// Cell delta for one step in this direction (x right, y down).
    pub fn delta(self) -> (i32, i32) {
        match self {
            Direction::Up => (0, -1),
            Direction::UpRight => (1, -1),
            Direction::Right => (1, 0),
            Direction::DownRight => (1, 1),
            Direction::Down => (0, 1),
            Direction::DownLeft => (-1, 1),
            Direction::Left => (-1, 0),
            Direction::UpLeft => (-1, -1),
        }
    }

    pub fn opposite(self) -> Direction {
        Direction::from_index(self.index().wrapping_add(4))
    }

    pub fn rotate(self, steps: i8) -> Direction {
        Direction::from_index((self.index() as i8).wrapping_add(steps) as u8 & 7)
    }

    /// Direction from `from` toward `to` (Zircon's `Functions.DirectionFromPoint`).
    pub fn from_points(from: Point, to: Point) -> Direction {
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        if dx == 0 && dy == 0 {
            return Direction::Down;
        }
        // Angle in screen space; classify into 8 sectors.
        let angle = (dy as f64).atan2(dx as f64); // 0 = right, pi/2 = down
        let sector = ((angle / (std::f64::consts::PI / 4.0)).round() as i32).rem_euclid(8);
        match sector {
            0 => Direction::Right,
            1 => Direction::DownRight,
            2 => Direction::Down,
            3 => Direction::DownLeft,
            4 => Direction::Left,
            5 => Direction::UpLeft,
            6 => Direction::Up,
            _ => Direction::UpRight,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Point {
        Point { x, y }
    }
    pub fn step(self, dir: Direction, distance: i32) -> Point {
        let (dx, dy) = dir.delta();
        Point::new(self.x + dx * distance, self.y + dy * distance)
    }
    /// Chebyshev distance: the number of 8-way steps between two cells.
    pub fn distance(self, other: Point) -> i32 {
        (self.x - other.x).abs().max((self.y - other.y).abs())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObjectId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Gender {
    Male,
    Female,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Class {
    Warrior,
    Wizard,
    Taoist,
    Assassin,
}

impl Class {
    pub const ALL: [Class; 4] = [
        Class::Warrior,
        Class::Wizard,
        Class::Taoist,
        Class::Assassin,
    ];
    /// Zircon `MirClass` value (BaseStat.Class).
    pub fn mir_class(self) -> u8 {
        match self {
            Class::Warrior => 0,
            Class::Wizard => 1,
            Class::Taoist => 2,
            Class::Assassin => 3,
        }
    }
    /// Zircon `RequiredClass` flag (SafeZoneInfo.StartClass).
    pub fn flag(self) -> u8 {
        1 << self.mir_class()
    }
    pub fn name(self) -> &'static str {
        match self {
            Class::Warrior => "Warrior",
            Class::Wizard => "Wizard",
            Class::Taoist => "Taoist",
            Class::Assassin => "Assassin",
        }
    }
}

/// What an object looks like; enough for the client to pick sprites.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Appearance {
    Player {
        name: String,
        gender: Gender,
        class: Class,
        /// Equipped armour `ItemInfo.Shape` (0 = basic clothes).
        armour: u16,
        /// Equipped weapon `ItemInfo.Shape`, if any.
        weapon: Option<u16>,
        hair: u8,
        /// Equipped helmet `ItemInfo.Shape` (1-based; 0 = none, hair shows).
        helmet: u16,
        /// Equipped shield `ItemInfo.Shape`, if any.
        shield: Option<u16>,
    },
    Monster {
        name: String,
        /// Zircon `MonsterImage` value; the client maps it to a library + base index.
        image: u16,
        /// Name of the player this pet fights for.
        owner: Option<String>,
    },
    Npc {
        name: String,
        /// `NPCInfo.Image`; sprite index = image * 100 + frame in NPC.Zl.
        image: u16,
    },
    /// A spell object on a cell (fire wall, cloud); drawn on the floor.
    Spell { effect: u8 },
    /// An item lying on the ground.
    Item {
        /// `ItemInfo.Index`.
        info: i32,
        count: u32,
    },
}

/// An item instance as the client sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemInstance {
    pub id: u32,
    /// `ItemInfo.Index`.
    pub info: i32,
    pub count: u32,
    pub durability: i32,
    pub max_durability: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Grid {
    Inventory,
    Equipment,
}

pub const INVENTORY_SIZE: usize = 48;
pub const EQUIPMENT_SIZE: usize = 22;

/// Zircon `EquipmentSlot`.
pub mod slot {
    pub const WEAPON: usize = 0;
    pub const ARMOUR: usize = 1;
    pub const HELMET: usize = 2;
    pub const TORCH: usize = 3;
    pub const NECKLACE: usize = 4;
    pub const BRACELET_L: usize = 5;
    pub const BRACELET_R: usize = 6;
    pub const RING_L: usize = 7;
    pub const RING_R: usize = 8;
    pub const SHOES: usize = 9;
    pub const POISON: usize = 10;
    pub const AMULET: usize = 11;
    pub const SHIELD: usize = 15;
}

/// Zircon `ItemType` values.
pub mod item_type {
    pub const NOTHING: u8 = 0;
    pub const CONSUMABLE: u8 = 1;
    pub const WEAPON: u8 = 2;
    pub const ARMOUR: u8 = 3;
    pub const TORCH: u8 = 4;
    pub const HELMET: u8 = 5;
    pub const NECKLACE: u8 = 6;
    pub const BRACELET: u8 = 7;
    pub const RING: u8 = 8;
    pub const SHOES: u8 = 9;
    pub const POISON: u8 = 10;
    pub const AMULET: u8 = 11;
    pub const MEAT: u8 = 12;
    pub const ORE: u8 = 13;
    pub const BOOK: u8 = 14;
    pub const SCROLL: u8 = 15;
    pub const DARK_STONE: u8 = 16;
    pub const SHIELD: u8 = 27;
    pub const CURRENCY: u8 = 34;

    /// Equipment slots an item type may occupy (Zircon `Functions.CorrectSlot`).
    pub fn slots(item_type: u8) -> &'static [usize] {
        match item_type {
            WEAPON => &[super::slot::WEAPON],
            ARMOUR => &[super::slot::ARMOUR],
            HELMET => &[super::slot::HELMET],
            TORCH => &[super::slot::TORCH],
            NECKLACE => &[super::slot::NECKLACE],
            BRACELET => &[super::slot::BRACELET_L, super::slot::BRACELET_R],
            RING => &[super::slot::RING_L, super::slot::RING_R],
            SHOES => &[super::slot::SHOES],
            POISON => &[super::slot::POISON],
            AMULET | DARK_STONE => &[super::slot::AMULET],
            SHIELD => &[super::slot::SHIELD],
            _ => &[],
        }
    }
}

/// A learned skill as the client sees it (`MagicInfo` comes from System.db).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MagicSummary {
    /// Zircon `MagicType` value.
    pub magic: u16,
    pub level: u8,
    pub experience: u64,
    /// Hotkey 1..=12 (F1..F12), 0 = none.
    pub key: u8,
}

/// Zircon `Globals.MaxBeltCount`: belt slots 0..=9 (keys 1..9, 0).
pub const MAX_BELT: usize = 10;

/// One belt slot (Zircon `ClientBeltLink`): links a stackable/consumable by
/// `ItemInfo` index, or a specific item by its id. Never both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeltLink {
    pub slot: u8,
    pub info: Option<i32>,
    pub item: Option<u32>,
}

impl BeltLink {
    pub fn empty(slot: u8) -> BeltLink {
        BeltLink {
            slot,
            info: None,
            item: None,
        }
    }
}

pub const MAGIC_RANGE: i32 = 10;
pub const MAGIC_DELAY: u64 = 2000;
pub const CAST_TIME: u64 = 600;

/// A shop entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Good {
    pub info: i32,
    pub price: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Weights {
    pub bag: i32,
    pub max_bag: i32,
    pub wear: i32,
    pub max_wear: i32,
    pub hand: i32,
    pub max_hand: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Standing,
    Walking,
    Running,
    Attack,
    /// Zircon `Combat4` (Half Moon and other backhand swings).
    Attack2,
    /// Zircon `Combat1`: projectile spell cast.
    Cast1,
    /// Zircon `Combat2`: targeted spell cast.
    Cast2,
    /// Zircon `Combat5`: Dragon Rise / one-handed lotus swing.
    Attack5,
    /// Zircon `Combat6`: Blade Storm.
    Attack6,
    /// Zircon `Combat8`: Shoulder Dash step.
    Dash,
    /// Zircon `Combat15`: self-buff stance cast.
    Stance,
    Struck,
    Die,
    Dead,
}

/// Zircon `Element`.
pub mod element {
    pub const NONE: u8 = 0;
    pub const FIRE: u8 = 1;
    pub const ICE: u8 = 2;
    pub const LIGHTNING: u8 = 3;
    pub const WIND: u8 = 4;
    pub const HOLY: u8 = 5;
    pub const DARK: u8 = 6;
    pub const PHANTOM: u8 = 7;
}

/// Zircon `MagicType` values used by the prototype.
pub mod magic_type {
    pub const SWORDSMANSHIP: u16 = 100;
    pub const POTION_MASTERY: u16 = 101;
    pub const SLAYING: u16 = 102;
    pub const THRUSTING: u16 = 103;
    pub const HALF_MOON: u16 = 104;
    pub const SHOULDER_DASH: u16 = 105;
    pub const FLAMING_SWORD: u16 = 106;
    pub const DRAGON_RISE: u16 = 107;
    pub const BLADE_STORM: u16 = 108;
    pub const DESTRUCTIVE_SURGE: u16 = 109;
    pub const INTERCHANGE: u16 = 110;
    pub const DEFIANCE: u16 = 111;
    pub const BECKON: u16 = 112;
    pub const MIGHT: u16 = 113;
    pub const SWIFT_BLADE: u16 = 114;
    pub const ASSAULT: u16 = 115;
    pub const ENDURANCE: u16 = 116;
    pub const REFLECT_DAMAGE: u16 = 117;
    pub const FETTER: u16 = 118;
    pub const MASS_BECKON: u16 = 123;
    pub const FIRE_BALL: u16 = 201;
    pub const LIGHTNING_BALL: u16 = 202;
    pub const ICE_BOLT: u16 = 203;
    pub const GUST_BLAST: u16 = 204;
    pub const REPULSION: u16 = 205;
    pub const ELECTRIC_SHOCK: u16 = 206;
    pub const TELEPORTATION: u16 = 207;
    pub const ADAMANTINE_FIRE_BALL: u16 = 208;
    pub const THUNDER_BOLT: u16 = 209;
    pub const ICE_BLADES: u16 = 210;
    pub const CYCLONE: u16 = 211;
    pub const SCORCHED_EARTH: u16 = 212;
    pub const LIGHTNING_BEAM: u16 = 213;
    pub const FROZEN_EARTH: u16 = 214;
    pub const BLOW_EARTH: u16 = 215;
    pub const FIRE_WALL: u16 = 216;
    pub const EXPEL_UNDEAD: u16 = 217;
    pub const GEO_MANIPULATION: u16 = 218;
    pub const MAGIC_SHIELD: u16 = 219;
    pub const FIRE_STORM: u16 = 220;
    pub const LIGHTNING_WAVE: u16 = 221;
    pub const ICE_STORM: u16 = 222;
    pub const DRAGON_TORNADO: u16 = 223;
    pub const GREATER_FROZEN_EARTH: u16 = 224;
    pub const CHAIN_LIGHTNING: u16 = 225;
    pub const METEOR_SHOWER: u16 = 226;
    pub const RENOUNCE: u16 = 227;
    pub const TEMPEST: u16 = 228;
    pub const HEAL: u16 = 300;
    pub const SPIRIT_SWORD: u16 = 301;
    pub const POISON_DUST: u16 = 302;
    pub const EXPLOSIVE_TALISMAN: u16 = 303;
    pub const EVIL_SLAYER: u16 = 304;
    pub const INVISIBILITY: u16 = 305;
    pub const MAGIC_RESISTANCE: u16 = 306;
    pub const MASS_INVISIBILITY: u16 = 307;
    pub const GREATER_EVIL_SLAYER: u16 = 308;
    pub const RESILIENCE: u16 = 309;
    pub const TRAP_OCTAGON: u16 = 310;
    pub const COMBAT_KICK: u16 = 311;
    pub const ELEMENTAL_SUPERIORITY: u16 = 312;
    pub const MASS_HEAL: u16 = 313;
    pub const BLOOD_LUST: u16 = 314;
    pub const RESURRECTION: u16 = 315;
    pub const PURIFICATION: u16 = 316;
    pub const TRANSPARENCY: u16 = 317;
    pub const CELESTIAL_LIGHT: u16 = 318;
    pub const SUMMON_SKELETON: u16 = 332;
    pub const SUMMON_SHINSU: u16 = 333;
    pub const SUMMON_JIN_SKELETON: u16 = 334;
    pub const STRENGTH_OF_FAITH: u16 = 335;
    pub const WILLOW_DANCE: u16 = 401;
    pub const VINE_TREE_DANCE: u16 = 402;
    pub const DISCIPLINE: u16 = 403;
    pub const POISONOUS_CLOUD: u16 = 404;
    pub const FULL_BLOOM: u16 = 405;
    pub const CLOAK: u16 = 406;
    pub const WHITE_LOTUS: u16 = 407;
    pub const CALAMITY_OF_FULL_MOON: u16 = 408;
    pub const WRAITH_GRIP: u16 = 409;
    pub const RED_LOTUS: u16 = 410;
    pub const HELL_FIRE: u16 = 411;
    pub const PLEDGE_OF_BLOOD: u16 = 412;
    pub const RAKE: u16 = 413;
    pub const SWEET_BRIER: u16 = 414;
    pub const SUMMON_PUPPET: u16 = 415;
    pub const KARMA: u16 = 416;
    pub const TOUCH_OF_THE_DEPARTED: u16 = 417;
    pub const WANING_MOON: u16 = 418;
    pub const GHOST_WALK: u16 = 419;
    pub const ELEMENTAL_PUPPET: u16 = 420;
    pub const REJUVENATION: u16 = 421;
    pub const RESOLUTION: u16 = 422;
    pub const RELEASE: u16 = 424;
    pub const FLAME_SPLASH: u16 = 425;
    pub const BLOODY_FLOWER: u16 = 426;
    // Monster-only spells (Zircon 501+; 550+ are this port's own tags).
    pub const MONSTER_SCORCHED_EARTH: u16 = 501;
    pub const MONSTER_ICE_STORM: u16 = 502;
    pub const MONSTER_DEATH_CLOUD: u16 = 503;
    pub const MONSTER_THUNDER_STORM: u16 = 504;
    pub const SAMA_GUARDIAN_FIRE: u16 = 505;
    pub const SAMA_GUARDIAN_ICE: u16 = 506;
    pub const SAMA_GUARDIAN_LIGHTNING: u16 = 507;
    pub const SAMA_GUARDIAN_WIND: u16 = 508;
    pub const PINK_FIRE_BALL: u16 = 530;
    pub const GREEN_SLUDGE_BALL: u16 = 540;
    pub const MONSTER_SPLASH: u16 = 550;
    pub const MONSTER_DARK_BEAM: u16 = 551;
    pub const FLAMING_DAGGERS: u16 = 454;
    pub const SHREDDING: u16 = 455;

    /// Skills that are never cast: they act on every melee swing or as stats.
    pub fn is_passive(m: u16) -> bool {
        matches!(
            m,
            SWORDSMANSHIP
                | POTION_MASTERY
                | SLAYING
                | SPIRIT_SWORD
                | WILLOW_DANCE
                | VINE_TREE_DANCE
                | DISCIPLINE
                | BLOODY_FLOWER
                | ASSAULT
                | PLEDGE_OF_BLOOD
                | TOUCH_OF_THE_DEPARTED
                | GHOST_WALK
                | ELEMENTAL_PUPPET
                | REJUVENATION
                | RESOLUTION
                | RELEASE
                | CALAMITY_OF_FULL_MOON
                | WANING_MOON
        )
    }
    /// Stance skills switched with a hotkey and applied on melee swings.
    pub fn is_toggle(m: u16) -> bool {
        matches!(m, THRUSTING | HALF_MOON | DESTRUCTIVE_SURGE | FLAME_SPLASH)
    }
    /// Warrior power attacks charged with a hotkey for 12 s (Zircon `Toggle`).
    pub fn is_charge(m: u16) -> bool {
        matches!(m, FLAMING_SWORD | DRAGON_RISE | BLADE_STORM)
    }
    /// Assassin lotus combo: the hotkey arms the next swing.
    pub fn is_lotus(m: u16) -> bool {
        matches!(m, FULL_BLOOM | WHITE_LOTUS | RED_LOTUS)
    }
    /// Skills armed with a hotkey for the next swing (lotus chain, Sweetbrier, Karma).
    pub fn is_armed(m: u16) -> bool {
        is_lotus(m) || matches!(m, SWEET_BRIER | KARMA)
    }
    /// Moon charges that arm themselves while swinging.
    pub fn is_auto_charge(m: u16) -> bool {
        matches!(m, CALAMITY_OF_FULL_MOON | WANING_MOON)
    }
    /// Spells with the projectile cast animation (Zircon `Combat1`).
    pub fn is_projectile_cast(m: u16) -> bool {
        matches!(
            m,
            FIRE_BALL
                | ICE_BOLT
                | FLAMING_DAGGERS
                | SHREDDING
                | LIGHTNING_BALL
                | GUST_BLAST
                | ADAMANTINE_FIRE_BALL
                | ICE_BLADES
                | EXPLOSIVE_TALISMAN
                | EVIL_SLAYER
                | GREATER_EVIL_SLAYER
                | MAGIC_RESISTANCE
                | RESILIENCE
                | BECKON
                | MASS_BECKON
                | METEOR_SHOWER
                | GREATER_FROZEN_EARTH
        )
    }
    /// Spells that fly at or strike a chosen monster.
    pub fn needs_target(m: u16) -> bool {
        matches!(
            m,
            FIRE_BALL
                | ICE_BOLT
                | THUNDER_BOLT
                | POISON_DUST
                | FLAMING_DAGGERS
                | SHREDDING
                | LIGHTNING_BALL
                | GUST_BLAST
                | ADAMANTINE_FIRE_BALL
                | ICE_BLADES
                | CYCLONE
                | EXPLOSIVE_TALISMAN
                | EVIL_SLAYER
                | GREATER_EVIL_SLAYER
                | INTERCHANGE
                | BECKON
                | EXPEL_UNDEAD
                | CHAIN_LIGHTNING
                | ELECTRIC_SHOCK
                | PURIFICATION
                | WRAITH_GRIP
                | HELL_FIRE
        )
    }
    /// Spells cast on a ground cell.
    pub fn needs_cell(m: u16) -> bool {
        matches!(
            m,
            FIRE_WALL
                | MAGIC_RESISTANCE
                | RESILIENCE
                | MASS_HEAL
                | SWIFT_BLADE
                | GEO_MANIPULATION
                | FIRE_STORM
                | LIGHTNING_WAVE
                | ICE_STORM
                | DRAGON_TORNADO
                | METEOR_SHOWER
                | TEMPEST
                | MASS_INVISIBILITY
                | TRAP_OCTAGON
                | ELEMENTAL_SUPERIORITY
                | BLOOD_LUST
                | RESURRECTION
        )
    }
    /// Spells cast on oneself (no target, own cell).
    pub fn is_self_cast(m: u16) -> bool {
        matches!(
            m,
            TELEPORTATION
                | MAGIC_SHIELD
                | DEFIANCE
                | MIGHT
                | POISONOUS_CLOUD
                | SHOULDER_DASH
                | MASS_BECKON
                | ENDURANCE
                | REFLECT_DAMAGE
                | FETTER
                | RENOUNCE
                | SUMMON_SKELETON
                | SUMMON_SHINSU
                | SUMMON_JIN_SKELETON
                | STRENGTH_OF_FAITH
                | INVISIBILITY
                | TRANSPARENCY
                | CELESTIAL_LIGHT
                | COMBAT_KICK
                | CLOAK
                | RAKE
                | SUMMON_PUPPET
        )
    }
    /// Spells that only use the facing direction.
    pub fn is_line(m: u16) -> bool {
        matches!(
            m,
            SCORCHED_EARTH | LIGHTNING_BEAM | FROZEN_EARTH | BLOW_EARTH | GREATER_FROZEN_EARTH
        )
    }
    /// Self buffs cast facing down (Zircon `Combat15`).
    pub fn is_stance_cast(m: u16) -> bool {
        matches!(
            m,
            DEFIANCE | MIGHT | ENDURANCE | REFLECT_DAMAGE | MASS_BECKON
        )
    }
    /// Casts played with the stance frames although not facing down.
    pub fn is_stance_anim(m: u16) -> bool {
        matches!(m, DEFIANCE | MIGHT | ENDURANCE | REFLECT_DAMAGE | FETTER)
    }
    /// Spells the prototype can cast.
    pub fn is_castable(m: u16) -> bool {
        matches!(
            m,
            FIRE_BALL
                | ICE_BOLT
                | REPULSION
                | THUNDER_BOLT
                | HEAL
                | POISON_DUST
                | FLAMING_DAGGERS
                | SHREDDING
                | SHOULDER_DASH
                | DEFIANCE
                | MIGHT
                | LIGHTNING_BALL
                | GUST_BLAST
                | TELEPORTATION
                | ADAMANTINE_FIRE_BALL
                | ICE_BLADES
                | CYCLONE
                | SCORCHED_EARTH
                | LIGHTNING_BEAM
                | FROZEN_EARTH
                | BLOW_EARTH
                | FIRE_WALL
                | MAGIC_SHIELD
                | EXPLOSIVE_TALISMAN
                | EVIL_SLAYER
                | MAGIC_RESISTANCE
                | GREATER_EVIL_SLAYER
                | RESILIENCE
                | MASS_HEAL
                | POISONOUS_CLOUD
                | INTERCHANGE
                | BECKON
                | MASS_BECKON
                | SWIFT_BLADE
                | ENDURANCE
                | REFLECT_DAMAGE
                | FETTER
                | EXPEL_UNDEAD
                | GEO_MANIPULATION
                | FIRE_STORM
                | LIGHTNING_WAVE
                | ICE_STORM
                | DRAGON_TORNADO
                | GREATER_FROZEN_EARTH
                | CHAIN_LIGHTNING
                | METEOR_SHOWER
                | RENOUNCE
                | TEMPEST
                | ELECTRIC_SHOCK
                | SUMMON_SKELETON
                | SUMMON_SHINSU
                | SUMMON_JIN_SKELETON
                | STRENGTH_OF_FAITH
                | INVISIBILITY
                | MASS_INVISIBILITY
                | TRAP_OCTAGON
                | COMBAT_KICK
                | ELEMENTAL_SUPERIORITY
                | BLOOD_LUST
                | RESURRECTION
                | PURIFICATION
                | TRANSPARENCY
                | CELESTIAL_LIGHT
                | CLOAK
                | WRAITH_GRIP
                | HELL_FIRE
                | RAKE
                | SUMMON_PUPPET
        )
    }
}

/// Zircon `BuffType` values used by the prototype.
pub mod buff_type {
    pub const DEFIANCE: u16 = 100;
    pub const MIGHT: u16 = 101;
    pub const ENDURANCE: u16 = 102;
    pub const REFLECT_DAMAGE: u16 = 103;
    pub const RENOUNCE: u16 = 200;
    pub const MAGIC_SHIELD: u16 = 201;
    pub const HEAL: u16 = 300;
    pub const INVISIBILITY: u16 = 301;
    pub const MAGIC_RESISTANCE: u16 = 302;
    pub const RESILIENCE: u16 = 303;
    pub const ELEMENTAL_SUPERIORITY: u16 = 304;
    pub const BLOOD_LUST: u16 = 305;
    pub const STRENGTH_OF_FAITH: u16 = 306;
    pub const CELESTIAL_LIGHT: u16 = 307;
    pub const TRANSPARENCY: u16 = 308;
    pub const POISONOUS_CLOUD: u16 = 400;
    pub const FULL_BLOOM: u16 = 401;
    pub const WHITE_LOTUS: u16 = 402;
    pub const RED_LOTUS: u16 = 403;
    pub const CLOAK: u16 = 404;
    pub const GHOST_WALK: u16 = 405;
    /// Buffs other players can see (Zircon `visible: true`).
    pub fn is_visible(b: u16) -> bool {
        matches!(
            b,
            MAGIC_SHIELD
                | MAGIC_RESISTANCE
                | RESILIENCE
                | REFLECT_DAMAGE
                | STRENGTH_OF_FAITH
                | INVISIBILITY
                | ELEMENTAL_SUPERIORITY
                | BLOOD_LUST
                | CELESTIAL_LIGHT
                | TRANSPARENCY
                | CLOAK
                | GHOST_WALK
        )
    }
}

/// Zircon `SpellEffect` values.
pub mod spell_effect {
    pub const FIRE_WALL: u8 = 2;
    pub const TEMPEST: u8 = 3;
    pub const TRAP_OCTAGON: u8 = 5;
    pub const POISONOUS_CLOUD: u8 = 7;
}

/// Zircon `Effect` values sent with `ObjectEffect` / `MapEffect`.
pub mod effect {
    pub const TELEPORT_OUT: u8 = 1;
    pub const TELEPORT_IN: u8 = 2;
    pub const FULL_BLOOM: u8 = 3;
    pub const WHITE_LOTUS: u8 = 4;
    pub const RED_LOTUS: u8 = 5;
    pub const FIRE_WALL_SMOKE: u8 = 6;
    pub const SWEET_BRIER: u8 = 7;
    pub const KARMA: u8 = 8;
    pub const PUPPET: u8 = 9;
}

/// A buff as the client sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuffSummary {
    pub kind: u16,
    /// Milliseconds left; `u64::MAX` for permanent.
    pub remaining_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectState {
    pub id: ObjectId,
    pub appearance: Appearance,
    pub location: Point,
    pub direction: Direction,
    pub hp: i32,
    pub max_hp: i32,
    pub dead: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapDescriptor {
    /// File stem under `Map/`, e.g. "0" for `Map/0.map`.
    pub file: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerStats {
    pub level: u8,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub experience: u64,
    pub max_experience: u64,
    pub min_dc: i32,
    pub max_dc: i32,
    pub min_ac: i32,
    pub max_ac: i32,
    pub accuracy: i32,
    pub agility: i32,
    pub attack_speed: i32,
}

/// Summary of a character on the select screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub id: u32,
    pub name: String,
    pub class: Class,
    pub gender: Gender,
    pub hair: u8,
    pub level: u8,
    /// Unix seconds of the last login, 0 if never played.
    pub last_login: u64,
    /// Name of the map the character is on ("" for a new character).
    pub location: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LoginResult {
    Success { characters: Vec<CharacterSummary> },
    Failed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NewAccountResult {
    Success,
    Failed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NewCharacterResult {
    Success { character: CharacterSummary },
    Failed { reason: String },
}

/// Account rules shared by client-side validation and the server.
pub mod rules {
    /// Zircon `Globals` timings (ms).
    pub const MOVE_TIME: u64 = 600;
    pub const TURN_TIME: u64 = 300;
    pub const ATTACK_TIME: u64 = 600;
    pub const ATTACK_DELAY: u64 = 1500;
    pub const ASPEED_RATE: u64 = 47;

    /// Time between swings: `max(800, 1500 - AttackSpeed * 47)` ms.
    pub fn attack_delay(attack_speed: i64) -> u64 {
        (ATTACK_DELAY as i64 - attack_speed * ASPEED_RATE as i64).max(800) as u64
    }

    /// Zircon `UserMagic.Cost`: `BaseCost + Level * LevelCost / 3`.
    pub fn magic_cost(base_cost: i32, level_cost: i32, level: u8) -> i32 {
        base_cost + level as i32 * level_cost / 3
    }

    /// Zircon `UserMagic.GetPower` bounds for a level.
    pub fn magic_power(
        min_base: i32,
        max_base: i32,
        min_level: i32,
        max_level: i32,
        level: u8,
    ) -> (i32, i32) {
        let min = (min_base + level as i32 * min_level / 3).max(0);
        let max = max_base + level as i32 * max_level / 3;
        (min, max)
    }

    /// Client-side consumable lock: `max(250, Durability)` ms.
    pub fn use_item_lock(durability: i32) -> u64 {
        durability.max(250) as u64
    }

    pub const EMAIL_MIN: usize = 3;
    pub const EMAIL_MAX: usize = 50;
    pub const PASSWORD_MIN: usize = 6;
    pub const PASSWORD_MAX: usize = 30;
    pub const NAME_MIN: usize = 3;
    pub const NAME_MAX: usize = 15;
    pub const MAX_CHARACTERS: usize = 4;
    pub const HAIR_TYPES: u8 = 10;

    pub fn valid_email(s: &str) -> bool {
        let n = s.chars().count();
        (EMAIL_MIN..=EMAIL_MAX).contains(&n)
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "@._-+".contains(c))
    }
    pub fn valid_password(s: &str) -> bool {
        let n = s.chars().count();
        (PASSWORD_MIN..=PASSWORD_MAX).contains(&n) && !s.chars().any(char::is_control)
    }
    /// Character names: letters and digits, must start with a letter.
    pub fn valid_name(s: &str) -> bool {
        let n = s.chars().count();
        (NAME_MIN..=NAME_MAX).contains(&n)
            && s.chars()
                .next()
                .map(|c| c.is_ascii_alphabetic())
                .unwrap_or(false)
            && s.chars().all(|c| c.is_ascii_alphanumeric())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientMessage {
    Hello {
        version: u16,
    },
    NewAccount {
        email: String,
        password: String,
    },
    Login {
        email: String,
        password: String,
    },
    NewCharacter {
        name: String,
        class: Class,
        gender: Gender,
        hair: u8,
    },
    DeleteCharacter {
        id: u32,
    },
    StartGame {
        id: u32,
    },
    /// Leave the map and return to the character list.
    Logout,
    Turn {
        direction: Direction,
    },
    Move {
        direction: Direction,
        run: bool,
    },
    Attack {
        direction: Direction,
        /// Melee-augment skill in effect (Slaying, Thrusting, Half Moon...).
        attack_magic: Option<u16>,
    },
    /// Cast a spell (`MirAction.Spell`).
    Magic {
        magic: u16,
        direction: Direction,
        target: Option<ObjectId>,
        location: Point,
    },
    /// Assign a hotkey (0 clears).
    MagicKey {
        magic: u16,
        key: u8,
    },
    /// Toggle a stance skill (Thrusting, Half Moon, Slaying auto).
    MagicToggle {
        magic: u16,
        on: bool,
    },
    /// Move an item between grid slots (equip/unequip/reorder).
    ItemMove {
        from: Grid,
        from_slot: u8,
        to: Grid,
        to_slot: u8,
    },
    /// Use a consumable or equip an item from the inventory.
    ItemUse {
        slot: u8,
    },
    /// Link (or clear, with both `None`) a belt slot.
    BeltLink {
        slot: u8,
        info: Option<i32>,
        item: Option<u32>,
    },
    /// Drop an inventory item on the ground.
    ItemDrop {
        slot: u8,
        count: u32,
    },
    /// Pick up whatever lies on or next to the player.
    PickUp,
    /// Return to town while dead (Zircon `C.TownRevive`).
    TownRevive,
    NpcCall {
        id: ObjectId,
    },
    NpcButton {
        button: i32,
    },
    NpcBuy {
        info: i32,
        count: u32,
    },
    /// Sell inventory slots to the open shop.
    NpcSell {
        slots: Vec<u8>,
    },
    NpcClose,
    Ping {
        nonce: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Sent once after `Hello` is accepted.
    Connected,
    NewAccountResult(NewAccountResult),
    LoginResult(LoginResult),
    NewCharacterResult(NewCharacterResult),
    DeleteCharacterResult {
        id: u32,
        ok: bool,
        reason: String,
    },
    /// The player left the map; the client should show the character list.
    LoggedOut {
        characters: Vec<CharacterSummary>,
    },
    Welcome {
        id: ObjectId,
        map: MapDescriptor,
        location: Point,
        direction: Direction,
        stats: PlayerStats,
    },
    Rejected {
        reason: String,
    },
    ObjectShow(ObjectState),
    ObjectRemove {
        id: ObjectId,
    },
    ObjectTurn {
        id: ObjectId,
        direction: Direction,
    },
    ObjectMove {
        id: ObjectId,
        from: Point,
        to: Point,
        direction: Direction,
        run: bool,
    },
    /// The server refused the requester's own move; resync to `location`.
    MoveDenied {
        location: Point,
        direction: Direction,
    },
    ObjectAttack {
        id: ObjectId,
        direction: Direction,
        attack_magic: Option<u16>,
    },
    /// A monster shot something at `target` (or `location`); `magic` picks
    /// the projectile look (0 = plain).
    ObjectRangeAttack {
        id: ObjectId,
        direction: Direction,
        target: Option<ObjectId>,
        location: Point,
        magic: u16,
    },
    /// Someone cast a spell: play the cast animation and, when `cast`, the
    /// payload effects on `targets` / `locations` afterwards.
    ObjectMagic {
        id: ObjectId,
        direction: Direction,
        location: Point,
        magic: u16,
        targets: Vec<ObjectId>,
        locations: Vec<Point>,
        cast: bool,
    },
    /// The player's full skill list (on entry).
    Magics(Vec<MagicSummary>),
    /// Belt links on entry (after `Inventory`).
    BeltLinks(Vec<BeltLink>),
    NewMagic(MagicSummary),
    MagicLeveled {
        magic: u16,
        level: u8,
        experience: u64,
    },
    MagicCooldown {
        magic: u16,
        delay_ms: u32,
    },
    MagicToggle {
        magic: u16,
        on: bool,
    },
    /// Poison applied/cleared on an object (client tint only).
    ObjectPoisoned {
        id: ObjectId,
        poisoned: bool,
    },
    ObjectStruck {
        id: ObjectId,
        attacker: ObjectId,
        damage: i32,
        /// `element::*` of the hit; `None` for plain melee.
        element: u8,
        /// True when a spell (not a swing) caused the hit.
        magic: bool,
    },
    HealthChanged {
        id: ObjectId,
        hp: i32,
        max_hp: i32,
    },
    ObjectDie {
        id: ObjectId,
    },
    /// An object jumped to a cell (teleport, swap, pull); sent to itself too.
    ObjectTeleport {
        id: ObjectId,
        location: Point,
        direction: Direction,
    },
    /// One Shoulder Dash step (Zircon `S.ObjectDash`); sent to the dasher too.
    ObjectDash {
        id: ObjectId,
        direction: Direction,
        /// Cell the dasher now stands on.
        location: Point,
        distance: u8,
        magic: u16,
    },
    /// A one-off effect on an object (teleport in/out, lotus hits).
    ObjectEffect {
        id: ObjectId,
        effect: u8,
        location: Point,
    },
    /// A one-off effect on a cell (fire wall smoke).
    MapEffect {
        location: Point,
        effect: u8,
    },
    BuffAdd(BuffSummary),
    BuffRemove {
        kind: u16,
    },
    BuffTime {
        kind: u16,
        remaining_ms: u64,
    },
    /// A visible buff started or ended on someone (Zircon `ObjectBuffAdd/Remove`).
    ObjectBuff {
        id: ObjectId,
        kind: u16,
        on: bool,
    },
    ObjectRevive {
        id: ObjectId,
        location: Point,
        direction: Direction,
        hp: i32,
    },
    StatsChanged(PlayerStats),
    /// Full inventory sync on entering the world.
    Inventory {
        inventory: Vec<(u8, ItemInstance)>,
        equipment: Vec<(u8, ItemInstance)>,
        gold: u64,
        weights: Weights,
    },
    /// A slot's content changed (`None` = now empty).
    ItemChanged {
        grid: Grid,
        slot: u8,
        item: Option<ItemInstance>,
    },
    GoldChanged {
        gold: u64,
    },
    WeightsChanged(Weights),
    /// An object's look changed (equipment).
    ObjectAppearance {
        id: ObjectId,
        appearance: Appearance,
    },
    /// The player was moved to another map; forget every object.
    MapChanged {
        map: MapDescriptor,
        location: Point,
        direction: Direction,
    },
    NpcResponse {
        npc: ObjectId,
        page: i32,
        say: String,
        /// Zircon `NPCDialogType` (1 = BuySell).
        dialog_type: i32,
        goods: Vec<Good>,
        /// Item types this shop buys.
        sell_types: Vec<u8>,
    },
    NpcClose,
    Chat {
        text: String,
    },
    Pong {
        nonce: u32,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ProtoError {
    #[error("frame too large: {0} bytes")]
    FrameTooLarge(usize),
    #[error("encode error: {0}")]
    Encode(postcard::Error),
    #[error("decode error: {0}")]
    Decode(postcard::Error),
}

/// Encode a message into a length-prefixed frame.
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, ProtoError> {
    let body = postcard::to_stdvec(msg).map_err(ProtoError::Encode)?;
    if body.len() > MAX_FRAME_LEN {
        return Err(ProtoError::FrameTooLarge(body.len()));
    }
    let mut frame = Vec::with_capacity(body.len() + 4);
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// Try to take one complete frame from the front of `buf`. Returns the decoded
/// message and drains the consumed bytes. `Ok(None)` means more data is needed.
pub fn decode_frame<T: for<'de> Deserialize<'de>>(
    buf: &mut Vec<u8>,
) -> Result<Option<T>, ProtoError> {
    if buf.len() < 4 {
        return Ok(None);
    }
    let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if len > MAX_FRAME_LEN {
        return Err(ProtoError::FrameTooLarge(len));
    }
    if buf.len() < 4 + len {
        return Ok(None);
    }
    let msg = postcard::from_bytes(&buf[4..4 + len]).map_err(ProtoError::Decode)?;
    buf.drain(..4 + len);
    Ok(Some(msg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let msg = ServerMessage::ObjectMove {
            id: ObjectId(7),
            from: Point::new(1, 2),
            to: Point::new(2, 3),
            direction: Direction::DownRight,
            run: false,
        };
        let mut buf = encode(&msg).unwrap();
        buf.extend_from_slice(&encode(&ServerMessage::Pong { nonce: 9 }).unwrap());
        let a: ServerMessage = decode_frame(&mut buf).unwrap().unwrap();
        assert_eq!(a, msg);
        let b: ServerMessage = decode_frame(&mut buf).unwrap().unwrap();
        assert_eq!(b, ServerMessage::Pong { nonce: 9 });
        assert!(decode_frame::<ServerMessage>(&mut buf).unwrap().is_none());
    }

    #[test]
    fn direction_from_points() {
        let o = Point::new(5, 5);
        assert_eq!(Direction::from_points(o, Point::new(5, 4)), Direction::Up);
        assert_eq!(
            Direction::from_points(o, Point::new(6, 4)),
            Direction::UpRight
        );
        assert_eq!(
            Direction::from_points(o, Point::new(6, 5)),
            Direction::Right
        );
        assert_eq!(
            Direction::from_points(o, Point::new(6, 6)),
            Direction::DownRight
        );
        assert_eq!(Direction::from_points(o, Point::new(5, 6)), Direction::Down);
        assert_eq!(
            Direction::from_points(o, Point::new(4, 6)),
            Direction::DownLeft
        );
        assert_eq!(Direction::from_points(o, Point::new(4, 5)), Direction::Left);
        assert_eq!(
            Direction::from_points(o, Point::new(4, 4)),
            Direction::UpLeft
        );
        assert_eq!(
            Direction::from_points(o, Point::new(15, 6)),
            Direction::Right
        );
    }
}

/// One piece of an NPC dialog page: plain text or a clickable button link.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DialogPart {
    Text(String),
    Button { label: String, id: i32 },
    NewLine,
}

/// Parse Zircon NPC page text: `[Label:ID]` becomes a button, `\r\n` a line
/// break, everything else plain text.
pub fn parse_dialog(say: &str) -> Vec<DialogPart> {
    let mut parts = Vec::new();
    let mut text = String::new();
    // Normalise line endings so the loop only deals with `\n`.
    let rest = say.replace("\r\n", "\n").replace('\r', "\n");
    let mut chars = rest.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\n' => {
                if !text.is_empty() {
                    parts.push(DialogPart::Text(std::mem::take(&mut text)));
                }
                parts.push(DialogPart::NewLine);
            }
            '[' => {
                let mut inner = String::new();
                let mut closed = false;
                for d in chars.by_ref() {
                    if d == ']' {
                        closed = true;
                        break;
                    }
                    inner.push(d);
                }
                match (closed, inner.rsplit_once(':')) {
                    (true, Some((label, id))) if id.trim().parse::<i32>().is_ok() => {
                        if !text.is_empty() {
                            parts.push(DialogPart::Text(std::mem::take(&mut text)));
                        }
                        parts.push(DialogPart::Button {
                            label: label.to_string(),
                            id: id.trim().parse().unwrap(),
                        });
                    }
                    _ => {
                        text.push('[');
                        text.push_str(&inner);
                        if closed {
                            text.push(']');
                        }
                    }
                }
            }
            other => text.push(other),
        }
    }
    if !text.is_empty() {
        parts.push(DialogPart::Text(text));
    }
    parts
}

#[cfg(test)]
mod dialog_tests {
    use super::*;

    #[test]
    fn parses_buttons_and_lines() {
        let p = parse_dialog("Hi there,\r\n[Browse:1]\r\n\r\n[Exit:0] bye [x]");
        assert_eq!(
            p,
            vec![
                DialogPart::Text("Hi there,".into()),
                DialogPart::NewLine,
                DialogPart::Button {
                    label: "Browse".into(),
                    id: 1
                },
                DialogPart::NewLine,
                DialogPart::NewLine,
                DialogPart::Button {
                    label: "Exit".into(),
                    id: 0
                },
                DialogPart::Text(" bye [x]".into()),
            ]
        );
    }
}
