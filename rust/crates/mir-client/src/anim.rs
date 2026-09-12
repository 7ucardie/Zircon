//! Client-side objects and their animation state, following Zircon's
//! `MapObject.UpdateFrame` / `FrameSet` tables.

use std::collections::VecDeque;

use mir_proto::{Action, Appearance, Direction, ObjectId, Point};

#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub start: u32,
    pub count: u32,
    pub interval: u64,
}

impl Frame {
    pub const fn new(start: u32, count: u32, interval: u64) -> Frame {
        Frame {
            start,
            count,
            interval,
        }
    }
    pub fn sum(&self) -> u64 {
        self.count as u64 * self.interval
    }
}

/// `FrameSet.Players` (all have direction stride 10).
pub fn player_frame(action: Action) -> Frame {
    match action {
        Action::Standing => Frame::new(0, 4, 500),
        Action::Walking => Frame::new(80, 6, 100),
        Action::Running => Frame::new(160, 6, 100),
        Action::Attack => Frame::new(720, 6, 100),
        Action::Attack2 => Frame::new(800, 6, 100),
        Action::Cast1 => Frame::new(560, 5, 120),
        Action::Cast2 => Frame::new(640, 5, 120),
        Action::Attack5 => Frame::new(880, 10, 60),
        Action::Attack6 => Frame::new(960, 10, 60),
        Action::Dash => Frame::new(1120, 6, 50),
        Action::Stance => Frame::new(400, 3, 200),
        Action::Struck => Frame::new(1840, 3, 100),
        Action::Die => Frame::new(1920, 10, 100),
        Action::Dead => Frame::new(1929, 1, 1000),
    }
}

/// Spell objects (`Client/Models/SpellObject.cs`): looping floor animations.
pub fn spell_frame(effect: u8) -> Frame {
    match effect {
        mir_proto::spell_effect::FIRE_WALL => Frame::new(920, 5, 150),
        mir_proto::spell_effect::TEMPEST => Frame::new(920, 10, 150),
        mir_proto::spell_effect::POISONOUS_CLOUD => Frame::new(400, 15, 100),
        _ => Frame::new(0, 1, 3_600_000),
    }
}

/// NPC standing loop (`Client/Models/NPCObject.cs`): a few images have
/// longer or static animations, the rest use 4 frames at 1 s.
pub fn npc_frame(image: u16) -> Frame {
    match image {
        64 | 65 | 91 | 92 | 93 | 157 | 158 | 160 | 165 | 166 | 168 | 208 | 209 | 210 | 211
        | 212 | 213 | 214 | 231 | 234 => Frame::new(0, 1, 3_600_000),
        56 | 57 => Frame::new(0, 12, 200),
        156 => Frame::new(0, 16, 200),
        _ => Frame::new(0, 4, 1000),
    }
}

/// `FrameSet.DefaultMonster`.
pub fn monster_frame(action: Action) -> Frame {
    match action {
        Action::Standing => Frame::new(0, 4, 500),
        Action::Walking | Action::Running => Frame::new(80, 6, 100),
        Action::Attack
        | Action::Attack2
        | Action::Attack5
        | Action::Attack6
        | Action::Cast1
        | Action::Cast2
        | Action::Stance => Frame::new(160, 6, 100),
        Action::Dash => Frame::new(80, 6, 100),
        Action::Struck => Frame::new(240, 2, 100),
        Action::Die => Frame::new(320, 10, 100),
        Action::Dead => Frame::new(329, 1, 1000),
    }
}

#[derive(Debug, Clone)]
pub struct Queued {
    pub action: Action,
    pub direction: Direction,
    /// Destination cell (for moves) or current cell.
    pub location: Point,
    pub distance: i32,
}

#[derive(Debug)]
pub struct ClientObject {
    pub id: ObjectId,
    pub appearance: Appearance,
    pub location: Point,
    pub direction: Direction,
    pub hp: i32,
    pub max_hp: i32,
    pub dead: bool,
    pub action: Action,
    pub frame: Frame,
    pub frame_index: u32,
    pub frame_start: u64,
    pub move_distance: i32,
    pub moving_offset: (i32, i32),
    pub queue: VecDeque<Queued>,
    /// Show the health bar until this time.
    pub health_time: u64,
    /// Floating damage numbers: (value, time shown).
    pub damage: Vec<(i32, u64)>,
    pub poisoned: bool,
    /// Visible buffs (Zircon `VisibleBuffs`): Magic Shield etc.
    pub visible_buffs: Vec<u16>,
}

impl ClientObject {
    pub fn new(state: &mir_proto::ObjectState, now: u64) -> ClientObject {
        let action = if state.dead {
            Action::Dead
        } else {
            Action::Standing
        };
        let frame = match &state.appearance {
            Appearance::Player { .. } => player_frame(action),
            Appearance::Monster { .. } => monster_frame(action),
            Appearance::Npc { image, .. } => npc_frame(*image),
            Appearance::Item { .. } => Frame::new(0, 1, 3_600_000),
            Appearance::Spell { effect } => spell_frame(*effect),
        };
        ClientObject {
            id: state.id,
            appearance: state.appearance.clone(),
            location: state.location,
            direction: state.direction,
            hp: state.hp,
            max_hp: state.max_hp,
            dead: state.dead,
            action,
            frame,
            frame_index: 0,
            frame_start: now.saturating_sub((now % 5) * 100),
            move_distance: 0,
            moving_offset: (0, 0),
            queue: VecDeque::new(),
            health_time: 0,
            damage: Vec::new(),
            poisoned: false,
            visible_buffs: Vec::new(),
        }
    }

    pub fn is_spell(&self) -> bool {
        matches!(self.appearance, Appearance::Spell { .. })
    }

    pub fn is_player(&self) -> bool {
        matches!(self.appearance, Appearance::Player { .. })
    }

    pub fn is_monster(&self) -> bool {
        matches!(self.appearance, Appearance::Monster { .. })
    }

    pub fn is_npc(&self) -> bool {
        matches!(self.appearance, Appearance::Npc { .. })
    }

    pub fn is_item(&self) -> bool {
        matches!(self.appearance, Appearance::Item { .. })
    }

    pub fn name(&self) -> &str {
        match &self.appearance {
            Appearance::Player { name, .. } => name,
            Appearance::Monster { name, .. } => name,
            Appearance::Npc { name, .. } => name,
            Appearance::Item { .. } | Appearance::Spell { .. } => "",
        }
    }

    fn frame_for(&self, action: Action) -> Frame {
        match &self.appearance {
            Appearance::Player { .. } => player_frame(action),
            Appearance::Monster { .. } => monster_frame(action),
            Appearance::Npc { image, .. } => npc_frame(*image),
            Appearance::Item { .. } => Frame::new(0, 1, 3_600_000),
            Appearance::Spell { effect } => spell_frame(*effect),
        }
    }

    pub fn enqueue(&mut self, q: Queued) {
        self.queue.push_back(q);
    }

    /// Snap to a server-reported state, dropping any pending animation.
    pub fn snap(&mut self, location: Point, direction: Direction, now: u64) {
        self.queue.clear();
        self.location = location;
        self.direction = direction;
        self.moving_offset = (0, 0);
        self.move_distance = 0;
        self.set_action(
            Queued {
                action: if self.dead {
                    Action::Dead
                } else {
                    Action::Standing
                },
                direction,
                location,
                distance: 0,
            },
            now,
        );
    }

    fn set_action(&mut self, q: Queued, now: u64) {
        self.action = q.action;
        self.direction = q.direction;
        self.location = q.location;
        self.move_distance = q.distance;
        self.frame = self.frame_for(q.action);
        self.frame_index = 0;
        self.frame_start = now;
    }

    fn next_action(&mut self, now: u64) {
        let q = match self.queue.pop_front() {
            Some(q) => q,
            None => Queued {
                action: if matches!(self.action, Action::Die | Action::Dead) || self.dead {
                    Action::Dead
                } else {
                    Action::Standing
                },
                direction: self.direction,
                location: self.location,
                distance: 0,
            },
        };
        self.set_action(q, now);
    }

    /// Advance animation; call every frame.
    pub fn process(&mut self, now: u64) {
        let interruptible = matches!(self.action, Action::Standing | Action::Dead);
        let elapsed = now.saturating_sub(self.frame_start);
        let mut frame = (elapsed / self.frame.interval.max(1)) as u32;
        if frame >= self.frame.count || (interruptible && !self.queue.is_empty()) {
            self.next_action(now);
            let elapsed = now.saturating_sub(self.frame_start);
            frame = ((elapsed / self.frame.interval.max(1)) as u32).min(self.frame.count - 1);
        }
        self.frame_index = frame;

        // Smooth movement (Config.SmoothMove = true): remaining offset back toward origin.
        self.moving_offset = (0, 0);
        if matches!(
            self.action,
            Action::Walking | Action::Running | Action::Dash
        ) && self.move_distance > 0
        {
            let sum = self.frame.sum() as f32;
            let t = (now.saturating_sub(self.frame_start) as f32).min(sum);
            let r = (sum - t) / sum;
            let dx = (48.0 * self.move_distance as f32 * r) as i32;
            let dy = (32.0 * self.move_distance as f32 * r) as i32;
            let (x, y) = match self.direction {
                Direction::Up => (0, dy),
                Direction::UpRight => (-dx, dy),
                Direction::Right => (-dx, 0),
                Direction::DownRight => (-dx, -dy),
                Direction::Down => (0, -dy),
                Direction::DownLeft => (dx, -dy),
                Direction::Left => (dx, 0),
                Direction::UpLeft => (dx, dy),
            };
            // Upstream dropped the even-pixel snap with the render/sim decoupling.
            self.moving_offset = (x, y);
        }
        self.damage.retain(|(_, t)| now < t + 1500);
    }

    /// Row used for y-sorting (`MapObject.RenderY`).
    pub fn render_y(&self) -> i32 {
        if self.moving_offset != (0, 0)
            && matches!(
                self.direction,
                Direction::Up | Direction::UpRight | Direction::UpLeft
            )
        {
            self.location.y + self.move_distance
        } else {
            self.location.y
        }
    }

    /// Sprite index within the body library for the current frame.
    pub fn sprite_index(&self, shape: u16) -> u32 {
        let base = self.frame_index + self.frame.start + 10 * self.direction.index() as u32;
        match &self.appearance {
            Appearance::Player { class, .. } => {
                let stride = if *class == mir_proto::Class::Assassin {
                    3000
                } else {
                    5000
                };
                base + (shape as u32 % 11) * stride
            }
            Appearance::Monster { .. } => base + (shape as u32 % 10) * 1000,
            // NPCs never face a direction: index = image * 100 + frame.
            Appearance::Npc { image, .. } => self.frame_index + *image as u32 * 100,
            Appearance::Item { .. } => 0,
            Appearance::Spell { .. } => self.frame.start + self.frame_index,
        }
    }

    /// Frame index within the current animation, without shape or direction.
    pub fn draw_frame(&self) -> u32 {
        self.frame_index + self.frame.start + 10 * self.direction.index() as u32
    }
}
