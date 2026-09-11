//! Spell visuals (Zircon `MirEffect` / `MirProjectile`): timed sprite
//! animations anchored on an object or a cell, and projectiles that travel
//! between cells with 16 direction blocks.

use mir_proto::{element, magic_type, ObjectId, Point};

pub const MAGIC: u16 = 242;
pub const MAGIC_EX: u16 = 243;
pub const MAGIC_EX3: u16 = 245;
pub const MAGIC_EX5: u16 = 247;

pub const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
pub const FIRE: [f32; 4] = [1.0, 69.0 / 255.0, 0.0, 1.0];
pub const ICE: [f32; 4] = [175.0 / 255.0, 238.0 / 255.0, 238.0 / 255.0, 1.0];
pub const LIGHTNING: [f32; 4] = [135.0 / 255.0, 206.0 / 255.0, 250.0 / 255.0, 1.0];
pub const WIND: [f32; 4] = [32.0 / 255.0, 178.0 / 255.0, 170.0 / 255.0, 1.0];
pub const HOLY: [f32; 4] = [189.0 / 255.0, 183.0 / 255.0, 107.0 / 255.0, 1.0];
pub const DARK: [f32; 4] = [139.0 / 255.0, 69.0 / 255.0, 19.0 / 255.0, 1.0];

#[derive(Debug, Clone, Copy)]
pub enum Anchor {
    Object(ObjectId),
    Cell(Point),
}

#[derive(Debug, Clone)]
pub struct Effect {
    pub library: u16,
    pub start: u32,
    pub count: u32,
    pub delay_ms: u64,
    pub color: [f32; 4],
    pub anchor: Anchor,
    /// 8-way direction block (`start + dir * 10`).
    pub direction: Option<u8>,
    pub started: u64,
}

impl Effect {
    pub fn frame(&self, now: u64) -> Option<u32> {
        let i = (now.saturating_sub(self.started) / self.delay_ms.max(1)) as u32;
        if i >= self.count {
            return None;
        }
        Some(self.start + i + self.direction.unwrap_or(0) as u32 * 10)
    }
}

#[derive(Debug, Clone)]
pub struct Projectile {
    pub library: u16,
    pub start: u32,
    pub count: u32,
    pub delay_ms: u64,
    pub color: [f32; 4],
    pub from: Point,
    pub to: Anchor,
    pub started: u64,
    /// Pixel distance based flight time (Zircon: 1 px per ms).
    pub duration: u64,
    pub dir16: u8,
    /// What to play on arrival.
    pub explode: Option<(u16, u32, u32, u64, [f32; 4])>,
}

impl Projectile {
    pub fn frame(&self, now: u64) -> u32 {
        let i =
            (now.saturating_sub(self.started) / self.delay_ms.max(1)) as u32 % self.count.max(1);
        self.start + i + self.dir16 as u32 * 10
    }
    pub fn progress(&self, now: u64) -> f32 {
        if self.duration == 0 {
            return 1.0;
        }
        (now.saturating_sub(self.started) as f32 / self.duration as f32).min(1.0)
    }
}

/// Zircon `Functions.Direction16` on pixel deltas (y stretched to 48/32).
pub fn direction16(from_px: (f32, f32), to_px: (f32, f32)) -> u8 {
    let dx = to_px.0 - from_px.0;
    let dy = (to_px.1 - from_px.1) * 48.0 / 32.0;
    if dx == 0.0 && dy == 0.0 {
        return 0;
    }
    // Angle clockwise from "up" in screen space.
    let angle = dx.atan2(-dy).to_degrees();
    let a = if angle < 0.0 { angle + 360.0 } else { angle };
    (((a + 11.25) / 22.5).floor() as i32).rem_euclid(16) as u8
}

pub fn element_color(e: u8) -> [f32; 4] {
    match e {
        element::FIRE => FIRE,
        element::ICE => ICE,
        element::LIGHTNING => LIGHTNING,
        element::WIND => WIND,
        element::HOLY => HOLY,
        element::DARK => DARK,
        element::PHANTOM => [128.0 / 255.0, 0.0, 128.0 / 255.0, 1.0],
        _ => WHITE,
    }
}

/// Struck-by-magic flash on the victim (`MagicEx`, 6 frames).
pub fn struck_effect(e: u8, target: ObjectId, now: u64) -> Effect {
    let start = match e {
        element::FIRE => 790,
        element::ICE => 810,
        element::LIGHTNING => 830,
        element::WIND => 850,
        element::HOLY => 870,
        element::DARK => 890,
        element::PHANTOM => 910,
        _ => 930,
    };
    Effect {
        library: MAGIC_EX,
        start,
        count: 6,
        delay_ms: 100,
        color: element_color(e),
        anchor: Anchor::Object(target),
        direction: None,
        started: now,
    }
}

/// Slash effect for a melee skill on the attacker, with 8-way direction.
pub fn attack_effect(magic: u16, attacker: ObjectId, dir: u8, now: u64) -> Option<Effect> {
    let (library, start) = match magic {
        magic_type::SLAYING => (MAGIC, 1350),
        magic_type::THRUSTING => (MAGIC_EX3, 0),
        magic_type::HALF_MOON => (MAGIC, 230),
        _ => return None,
    };
    Some(Effect {
        library,
        start,
        count: 6,
        delay_ms: 100,
        color: WHITE,
        anchor: Anchor::Object(attacker),
        direction: Some(dir),
        started: now,
    })
}

/// The charge-up effect on the caster when a spell starts.
pub fn cast_effect(magic: u16, caster: ObjectId, dir: u8, now: u64) -> Option<Effect> {
    let (library, start, count, delay, color, directed) = match magic {
        magic_type::FIRE_BALL => (MAGIC, 1820, 8, 70, FIRE, true),
        magic_type::ICE_BOLT => (MAGIC, 2620, 6, 80, ICE, true),
        magic_type::THUNDER_BOLT => (MAGIC, 1430, 12, 50, LIGHTNING, false),
        magic_type::REPULSION => (MAGIC, 90, 10, 100, WIND, false),
        magic_type::HEAL => (MAGIC, 660, 10, 60, HOLY, false),
        magic_type::POISON_DUST => (MAGIC, 60, 10, 60, DARK, false),
        _ => return None,
    };
    Some(Effect {
        library,
        start,
        count,
        delay_ms: delay,
        color,
        anchor: Anchor::Object(caster),
        direction: if directed { Some(dir) } else { None },
        started: now,
    })
}

/// Effects to spawn on targets/cells once the cast animation is done.
pub struct Payload {
    pub effects: Vec<Effect>,
    pub projectiles: Vec<(Point, Anchor, Projectile)>,
}

#[allow(clippy::too_many_arguments)]
pub fn payload(
    magic: u16,
    caster_cell: Point,
    targets: &[ObjectId],
    locations: &[Point],
    now: u64,
) -> Payload {
    let mut out = Payload {
        effects: Vec::new(),
        projectiles: Vec::new(),
    };
    let anchors: Vec<Anchor> = targets
        .iter()
        .map(|t| Anchor::Object(*t))
        .chain(locations.iter().map(|p| Anchor::Cell(*p)))
        .collect();
    match magic {
        magic_type::FIRE_BALL
        | magic_type::ICE_BOLT
        | magic_type::FLAMING_DAGGERS
        | magic_type::SHREDDING => {
            let (library, start, count, color, explode) = match magic {
                magic_type::FIRE_BALL => (MAGIC, 420, 5, FIRE, (MAGIC, 580, 10, 100, FIRE)),
                magic_type::ICE_BOLT => (MAGIC, 2700, 3, ICE, (MAGIC, 2860, 10, 100, ICE)),
                magic_type::FLAMING_DAGGERS => {
                    (MAGIC_EX5, 3900, 7, FIRE, (MAGIC_EX5, 4100, 8, 100, FIRE))
                }
                _ => (MAGIC_EX5, 4300, 5, FIRE, (MAGIC_EX5, 4500, 10, 100, FIRE)),
            };
            for a in anchors {
                out.projectiles.push((
                    caster_cell,
                    a,
                    Projectile {
                        library,
                        start,
                        count,
                        delay_ms: 100,
                        color,
                        from: caster_cell,
                        to: a,
                        started: now,
                        duration: 0,
                        dir16: 0,
                        explode: Some(explode),
                    },
                ));
            }
        }
        magic_type::THUNDER_BOLT => {
            for a in anchors {
                out.effects.push(Effect {
                    library: MAGIC,
                    start: 1450,
                    count: 3,
                    delay_ms: 150,
                    color: LIGHTNING,
                    anchor: a,
                    direction: None,
                    started: now,
                });
            }
        }
        magic_type::HEAL => {
            for a in anchors {
                out.effects.push(Effect {
                    library: MAGIC,
                    start: 610,
                    count: 10,
                    delay_ms: 100,
                    color: HOLY,
                    anchor: a,
                    direction: None,
                    started: now,
                });
            }
        }
        magic_type::POISON_DUST => {
            for a in anchors {
                out.effects.push(Effect {
                    library: MAGIC,
                    start: 70,
                    count: 10,
                    delay_ms: 100,
                    color: DARK,
                    anchor: a,
                    direction: None,
                    started: now,
                });
            }
        }
        _ => {}
    }
    out
}
