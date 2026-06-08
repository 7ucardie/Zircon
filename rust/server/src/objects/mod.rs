//! Typed game objects — players, monsters, and NPCs.

pub mod monster;
pub mod npc;
pub mod player;

pub use monster::MonsterObject;
pub use npc::NpcObject;
pub use player::PlayerObject;

/// Unique identifier for any live game object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(pub u32);

/// Enum-dispatched game entity; covers every live object type.
pub enum MapObject {
    Player(PlayerObject),
    Monster(MonsterObject),
    Npc(NpcObject),
}

impl MapObject {
    pub fn object_id(&self) -> ObjectId {
        match self {
            MapObject::Player(p)  => p.object_id,
            MapObject::Monster(m) => m.object_id,
            MapObject::Npc(n)     => n.object_id,
        }
    }

    pub fn map_index(&self) -> i32 {
        match self {
            MapObject::Player(p)  => p.map_index,
            MapObject::Monster(m) => m.map_index,
            MapObject::Npc(n)     => n.map_index,
        }
    }

    pub fn x(&self) -> i32 {
        match self {
            MapObject::Player(p)  => p.x,
            MapObject::Monster(m) => m.x,
            MapObject::Npc(n)     => n.x,
        }
    }

    pub fn y(&self) -> i32 {
        match self {
            MapObject::Player(p)  => p.y,
            MapObject::Monster(m) => m.y,
            MapObject::Npc(n)     => n.y,
        }
    }

    pub fn next_tick(&self) -> u64 {
        match self {
            MapObject::Player(p)  => p.next_tick,
            MapObject::Monster(m) => m.next_tick,
            MapObject::Npc(n)     => n.next_tick,
        }
    }

    pub fn set_next_tick(&mut self, tick: u64) {
        match self {
            MapObject::Player(p)  => p.next_tick = tick,
            MapObject::Monster(m) => m.next_tick = tick,
            MapObject::Npc(n)     => n.next_tick = tick,
        }
    }

    /// Advance this object one server tick.
    pub fn process(&mut self, tick: u64) {
        match self {
            MapObject::Player(p)  => p.process(tick),
            MapObject::Monster(m) => m.process(tick),
            MapObject::Npc(n)     => n.process(tick),
        }
    }
}
