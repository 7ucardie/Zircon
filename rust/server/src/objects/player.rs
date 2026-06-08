//! Player game object.

use super::ObjectId;

/// All server-side state for one logged-in player.
pub struct PlayerObject {
    pub object_id: ObjectId,
    /// TCP connection that owns this player (0 = none / disconnected).
    pub conn_id: u32,
    pub map_index: i32,
    pub x: i32,
    pub y: i32,

    // ── Character data ────────────────────────────────────────────────────
    pub name: String,
    /// MirClass enum value.
    pub class: i32,
    /// MirGender enum value.
    pub gender: i32,
    pub level: i32,
    pub experience: i64,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,

    // ── Tick scheduler ────────────────────────────────────────────────────
    pub next_tick: u64,
    pub move_tick: u64,
    pub action_tick: u64,
}

impl PlayerObject {
    pub fn new(
        object_id: ObjectId,
        conn_id: u32,
        map_index: i32,
        x: i32,
        y: i32,
        name: String,
        class: i32,
        level: i32,
    ) -> Self {
        PlayerObject {
            object_id,
            conn_id,
            map_index,
            x,
            y,
            name,
            class,
            gender: 0,
            level,
            experience: 0,
            hp: 100,
            max_hp: 100,
            mp: 50,
            max_mp: 50,
            next_tick: 0,
            move_tick: 0,
            action_tick: 0,
        }
    }

    pub fn process(&mut self, _tick: u64) {
        // Task 2.6: regen, buff decay, logout timer
    }
}
