//! NPC game object.

use super::ObjectId;

/// Server-side state for one NPC.
pub struct NpcObject {
    pub object_id: ObjectId,
    pub map_index: i32,
    pub x: i32,
    pub y: i32,
    pub next_tick: u64,
}

impl NpcObject {
    pub fn new(object_id: ObjectId, map_index: i32, x: i32, y: i32) -> Self {
        NpcObject { object_id, map_index, x, y, next_tick: 0 }
    }

    pub fn process(&mut self, _tick: u64) {
        // Task 2.6: dialog scripts, spawn triggers
    }
}
