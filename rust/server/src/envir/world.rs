//! Game world state — maps, players, and active objects.
//!
//! This is the data layer that Task 2.5 will populate with full game logic.

use std::collections::HashMap;

use zircon_db::MapInfo;

/// Unique identifier for a game object (player, monster, NPC, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(pub u32);

/// Coarse type tag for a game object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Player,
    Monster,
    Npc,
    Item,
    Spell,
}

/// A minimal game object — sufficient for the loop scheduler.
/// Task 2.5 will extend this with full combat/AI fields.
#[derive(Debug)]
pub struct GameObject {
    pub id: ObjectId,
    pub kind: ObjectKind,
    pub map_index: i32,
    pub x: i32,
    pub y: i32,
    /// When the object next needs processing (in ticks).
    pub next_tick: u64,
}

/// One loaded map.
#[derive(Debug)]
pub struct GameMap {
    pub info: MapInfo,
    /// All object IDs currently on this map.
    pub objects: Vec<ObjectId>,
}

/// The entire mutable game world.
#[derive(Debug, Default)]
pub struct World {
    /// Maps indexed by MapInfo.Index.
    pub maps: HashMap<i32, GameMap>,
    /// All live game objects indexed by ObjectId.
    pub objects: HashMap<ObjectId, GameObject>,
    /// Objects that need a process call this tick (mirrors C# ActiveObjects).
    pub active_queue: Vec<ObjectId>,
    /// Monotonically increasing object ID counter.
    next_object_id: u32,
}

impl World {
    pub fn new() -> Self {
        World::default()
    }

    /// Populate maps from loaded game data.
    pub fn load_maps(&mut self, maps: Vec<MapInfo>) {
        for info in maps {
            let idx = info.index;
            self.maps.insert(idx, GameMap { info, objects: Vec::new() });
        }
    }

    pub fn alloc_id(&mut self) -> ObjectId {
        self.next_object_id += 1;
        ObjectId(self.next_object_id)
    }

    pub fn insert_object(&mut self, obj: GameObject) {
        let map_idx = obj.map_index;
        let id = obj.id;
        self.objects.insert(id, obj);
        if let Some(map) = self.maps.get_mut(&map_idx) {
            map.objects.push(id);
        }
    }

    pub fn remove_object(&mut self, id: ObjectId) {
        if let Some(obj) = self.objects.remove(&id) {
            if let Some(map) = self.maps.get_mut(&obj.map_index) {
                map.objects.retain(|&o| o != id);
            }
        }
        self.active_queue.retain(|&o| o != id);
    }

    pub fn activate(&mut self, id: ObjectId) {
        if !self.active_queue.contains(&id) {
            self.active_queue.push(id);
        }
    }

    pub fn deactivate(&mut self, id: ObjectId) {
        self.active_queue.retain(|&o| o != id);
    }
}
