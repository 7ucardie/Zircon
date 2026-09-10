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
        Action::Struck => Frame::new(1840, 3, 100),
        Action::Die => Frame::new(1920, 10, 100),
        Action::Dead => Frame::new(1929, 1, 1000),
    }
}

/// `FrameSet.DefaultMonster`.
pub fn monster_frame(action: Action) -> Frame {
    match action {
        Action::Standing => Frame::new(0, 4, 500),
        Action::Walking | Action::Running => Frame::new(80, 6, 100),
        Action::Attack => Frame::new(160, 6, 100),
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
}

impl ClientObject {
    pub fn new(state: &mir_proto::ObjectState, now: u64) -> ClientObject {
        let is_player = matches!(state.appearance, Appearance::Player { .. });
        let action = if state.dead {
            Action::Dead
        } else {
            Action::Standing
        };
        let frame = if is_player {
            player_frame(action)
        } else {
            monster_frame(action)
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
        }
    }

    pub fn is_player(&self) -> bool {
        matches!(self.appearance, Appearance::Player { .. })
    }

    pub fn name(&self) -> &str {
        match &self.appearance {
            Appearance::Player { name, .. } => name,
            Appearance::Monster { name, .. } => name,
        }
    }

    fn frame_for(&self, action: Action) -> Frame {
        if self.is_player() {
            player_frame(action)
        } else {
            monster_frame(action)
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
        if matches!(self.action, Action::Walking | Action::Running) && self.move_distance > 0 {
            let sum = self.frame.sum() as f32;
            let t = (now.saturating_sub(self.frame_start) as f32).min(sum);
            let r = (sum - t) / sum;
            let dx = (48.0 * self.move_distance as f32 * r) as i32;
            let dy = (32.0 * self.move_distance as f32 * r) as i32;
            let (mut x, mut y) = match self.direction {
                Direction::Up => (0, dy),
                Direction::UpRight => (-dx, dy),
                Direction::Right => (-dx, 0),
                Direction::DownRight => (-dx, -dy),
                Direction::Down => (0, -dy),
                Direction::DownLeft => (dx, -dy),
                Direction::Left => (dx, 0),
                Direction::UpLeft => (dx, dy),
            };
            x -= x % 2;
            y -= y % 2;
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
        if self.is_player() {
            base + (shape as u32 % 11) * 5000
        } else {
            base + (shape as u32 % 10) * 1000
        }
    }
}
