//! Monster game object.

use super::ObjectId;

/// All server-side state for one live monster.
pub struct MonsterObject {
    pub object_id: ObjectId,
    /// FK into GameData.monsters (MonsterInfo.index).
    pub monster_info_index: i32,
    pub map_index: i32,
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    pub max_hp: i32,
    /// Current attack target.
    pub target_id: Option<ObjectId>,

    // ── Tick scheduler ────────────────────────────────────────────────────
    pub next_tick: u64,
    pub move_tick: u64,
    pub attack_tick: u64,
}

impl MonsterObject {
    pub fn new(
        object_id: ObjectId,
        monster_info_index: i32,
        map_index: i32,
        x: i32,
        y: i32,
        hp: i32,
    ) -> Self {
        MonsterObject {
            object_id,
            monster_info_index,
            map_index,
            x,
            y,
            hp,
            max_hp: hp,
            target_id: None,
            next_tick: 0,
            move_tick: 0,
            attack_tick: 0,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.hp > 0
    }

    pub fn process(&mut self, _tick: u64) {
        // Task 2.6: AI search → roam → attack decision tree
    }
}
