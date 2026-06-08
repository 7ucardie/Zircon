//! Game world state — maps and live objects.

use std::{collections::HashMap, path::Path};

use crate::{
    map,
    objects::{MapObject, ObjectId},
};

/// One loaded map at runtime: cell data + the objects currently on it.
pub struct GameMap {
    pub data: map::GameMap,
    pub objects: Vec<ObjectId>,
}

impl GameMap {
    pub fn info(&self) -> &zircon_db::MapInfo { &self.data.info }
    pub fn is_walkable(&self, x: i32, y: i32) -> bool { self.data.is_walkable(x, y) }
    pub fn walkable_count(&self) -> usize { self.data.walkable_count }
}

/// The entire mutable game world.
#[derive(Default)]
pub struct World {
    /// Maps indexed by MapInfo.index.
    pub maps: HashMap<i32, GameMap>,
    /// All live game objects indexed by ObjectId.
    pub objects: HashMap<ObjectId, MapObject>,
    /// Objects that need a process() call this tick.
    pub active_queue: Vec<ObjectId>,
    next_object_id: u32,
}

impl World {
    pub fn new() -> Self {
        World::default()
    }

    /// Load map cell grids from `maps_dir`.  Missing files produce empty maps
    /// (dev mode — server still starts without map assets).
    pub fn load_maps(&mut self, maps: Vec<zircon_db::MapInfo>, maps_dir: &Path) {
        for info in maps {
            let idx = info.index;
            let data = map::GameMap::load(info, maps_dir);
            self.maps.insert(idx, GameMap { data, objects: Vec::new() });
        }
    }

    /// Allocate a new unique ObjectId.
    pub fn alloc_id(&mut self) -> ObjectId {
        self.next_object_id += 1;
        ObjectId(self.next_object_id)
    }

    /// Insert a new object into the world and register it on its map.
    pub fn insert_object(&mut self, obj: MapObject) {
        let map_idx = obj.map_index();
        let id = obj.object_id();
        self.objects.insert(id, obj);
        if let Some(map) = self.maps.get_mut(&map_idx) {
            map.objects.push(id);
        }
    }

    /// Remove an object from the world and its map's object list.
    pub fn remove_object(&mut self, id: ObjectId) {
        if let Some(obj) = self.objects.remove(&id) {
            if let Some(map) = self.maps.get_mut(&obj.map_index()) {
                map.objects.retain(|&o| o != id);
            }
        }
        self.active_queue.retain(|&o| o != id);
    }

    /// Mark an object as needing process() this tick.
    pub fn activate(&mut self, id: ObjectId) {
        if !self.active_queue.contains(&id) {
            self.active_queue.push(id);
        }
    }

    /// Remove an object from the active queue.
    pub fn deactivate(&mut self, id: ObjectId) {
        self.active_queue.retain(|&o| o != id);
    }
}
