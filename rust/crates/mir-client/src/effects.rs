//! Spell visuals (Zircon `MirEffect` / `MirProjectile`): timed sprite
//! animations anchored on an object or a cell, and projectiles that travel
//! between cells with 16 direction blocks.

use mir_proto::{element, magic_type, ObjectId, Point};

pub const PROG_USE: u16 = 13;
pub const MAGIC: u16 = 242;
pub const MAGIC_EX: u16 = 243;
pub const MAGIC_EX2: u16 = 244;
pub const MAGIC_EX3: u16 = 245;
pub const MAGIC_EX4: u16 = 246;
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
    /// `None` before it starts and once it is over.
    pub fn frame(&self, now: u64) -> Option<u32> {
        if now < self.started {
            return None;
        }
        let i = (now.saturating_sub(self.started) / self.delay_ms.max(1)) as u32;
        if i >= self.count {
            return None;
        }
        Some(self.start + i + self.direction.unwrap_or(0) as u32 * 10)
    }
    pub fn finished(&self, now: u64) -> bool {
        now >= self.started + self.delay_ms.max(1) * self.count as u64
    }
    fn at(
        library: u16,
        start: u32,
        count: u32,
        delay_ms: u64,
        color: [f32; 4],
        anchor: Anchor,
        started: u64,
    ) -> Effect {
        Effect {
            library,
            start,
            count,
            delay_ms,
            color,
            anchor,
            direction: None,
            started,
        }
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
    /// Frames between direction blocks (Zircon `Skip`; 0 = one sprite for all).
    pub dir_stride: u32,
    /// What to play on arrival.
    pub explode: Option<(u16, u32, u32, u64, [f32; 4])>,
}

impl Projectile {
    pub fn frame(&self, now: u64) -> u32 {
        let i =
            (now.saturating_sub(self.started) / self.delay_ms.max(1)) as u32 % self.count.max(1);
        self.start + i + self.dir16 as u32 * self.dir_stride
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
    let (library, start, count, delay, color, directed, wait) = match magic {
        magic_type::SLAYING => (MAGIC, 1350, 6, 100, WHITE, true, 0),
        magic_type::THRUSTING => (MAGIC_EX3, 0, 6, 100, WHITE, true, 0),
        magic_type::HALF_MOON => (MAGIC, 230, 6, 100, WHITE, true, 0),
        magic_type::DESTRUCTIVE_SURGE => (MAGIC_EX2, 1420, 6, 100, WHITE, false, 0),
        magic_type::FLAMING_SWORD => (MAGIC, 1470, 6, 100, FIRE, true, 0),
        magic_type::DRAGON_RISE => (MAGIC, 2185, 10, 100, WHITE, true, 200),
        magic_type::BLADE_STORM => (MAGIC_EX, 1780, 10, 60, WHITE, true, 0),
        _ => return None,
    };
    Some(Effect {
        library,
        start,
        count,
        delay_ms: delay,
        color,
        anchor: Anchor::Object(attacker),
        direction: if directed { Some(dir) } else { None },
        started: now + wait,
    })
}

/// Effects triggered by `ObjectEffect` (teleport, lotus hits).
pub fn object_effect(kind: u8, id: ObjectId, cell: Point, now: u64) -> Option<Effect> {
    use mir_proto::effect as e;
    Some(match kind {
        e::TELEPORT_OUT => Effect::at(MAGIC, 110, 10, 100, WHITE, Anchor::Cell(cell), now),
        e::TELEPORT_IN => Effect::at(MAGIC, 110, 10, 100, WHITE, Anchor::Object(id), now),
        e::FULL_BLOOM => Effect::at(MAGIC_EX4, 1700, 4, 100, WHITE, Anchor::Object(id), now),
        e::WHITE_LOTUS => Effect::at(MAGIC_EX4, 1600, 12, 100, WHITE, Anchor::Object(id), now),
        e::RED_LOTUS => Effect::at(MAGIC_EX4, 1700, 12, 100, WHITE, Anchor::Object(id), now),
        _ => return None,
    })
}

/// Effects triggered by `MapEffect` on a cell.
pub fn map_effect(kind: u8, cell: Point, now: u64) -> Vec<Effect> {
    match kind {
        mir_proto::effect::FIRE_WALL_SMOKE => vec![
            Effect::at(
                PROG_USE,
                220,
                1,
                3500,
                [1.0, 1.0, 1.0, 0.8],
                Anchor::Cell(cell),
                now,
            ),
            Effect::at(MAGIC, 2450, 10, 250, WHITE, Anchor::Cell(cell), now),
        ],
        _ => Vec::new(),
    }
}

/// Looping Magic Shield ring on a shielded player (`Magic` 850..852).
pub fn shield_frame(now: u64) -> u32 {
    850 + ((now / 200) % 3) as u32
}

/// The charge-up effect on the caster when a spell starts.
pub fn cast_effect(magic: u16, caster: ObjectId, dir: u8, now: u64) -> Option<Effect> {
    let (library, start, count, delay, color, directed) = match magic {
        magic_type::FIRE_BALL => (MAGIC, 1820, 8, 70, FIRE, true),
        magic_type::ICE_BOLT => (MAGIC, 2620, 6, 80, ICE, true),
        magic_type::THUNDER_BOLT => (MAGIC, 1430, 12, 50, LIGHTNING, false),
        magic_type::REPULSION => (MAGIC, 90, 10, 100, WIND, false),
        magic_type::HEAL => (MAGIC, 660, 10, 60, HOLY, false),
        magic_type::MASS_HEAL => (MAGIC, 660, 10, 60, HOLY, false),
        magic_type::POISON_DUST => (MAGIC, 60, 10, 60, DARK, false),
        magic_type::LIGHTNING_BALL => (MAGIC, 2990, 6, 80, LIGHTNING, true),
        magic_type::GUST_BLAST => (MAGIC_EX, 350, 7, 70, WIND, true),
        magic_type::ADAMANTINE_FIRE_BALL => (MAGIC, 1560, 9, 65, FIRE, true),
        magic_type::ICE_BLADES => (MAGIC, 2880, 6, 115, ICE, true),
        magic_type::CYCLONE => (MAGIC_EX, 1970, 10, 60, WIND, false),
        magic_type::TELEPORTATION => (MAGIC, 110, 10, 60, WHITE, false),
        magic_type::MAGIC_SHIELD => (MAGIC, 830, 19, 60, WHITE, false),
        magic_type::SCORCHED_EARTH => (MAGIC, 1820, 8, 60, FIRE, true),
        magic_type::LIGHTNING_BEAM => (MAGIC, 1970, 10, 30, LIGHTNING, true),
        magic_type::FROZEN_EARTH => (MAGIC_EX, 0, 10, 50, ICE, true),
        magic_type::BLOW_EARTH => (MAGIC_EX, 1970, 10, 60, WIND, false),
        magic_type::FIRE_WALL => (MAGIC, 910, 10, 60, FIRE, false),
        magic_type::DEFIANCE => (MAGIC_EX2, 40, 10, 100, WHITE, false),
        magic_type::MIGHT => (MAGIC_EX2, 60, 10, 100, WHITE, false),
        magic_type::EXPLOSIVE_TALISMAN => (MAGIC, 2080, 6, 80, DARK, true),
        magic_type::EVIL_SLAYER => (MAGIC, 3250, 6, 80, HOLY, true),
        magic_type::GREATER_EVIL_SLAYER => (MAGIC, 3360, 6, 80, HOLY, true),
        magic_type::MAGIC_RESISTANCE | magic_type::RESILIENCE => (MAGIC, 2080, 6, 80, WHITE, true),
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
        | magic_type::SHREDDING
        | magic_type::LIGHTNING_BALL
        | magic_type::GUST_BLAST
        | magic_type::ADAMANTINE_FIRE_BALL
        | magic_type::ICE_BLADES
        | magic_type::EXPLOSIVE_TALISMAN
        | magic_type::EVIL_SLAYER
        | magic_type::GREATER_EVIL_SLAYER
        | magic_type::MAGIC_RESISTANCE
        | magic_type::RESILIENCE => {
            // (library, start, count, frame ms, colour, direction stride, explode)
            let (library, start, count, delay, color, stride, explode) = match magic {
                magic_type::FIRE_BALL => {
                    (MAGIC, 420, 5, 100, FIRE, 10, (MAGIC, 580, 10, 100, FIRE))
                }
                magic_type::ICE_BOLT => (MAGIC, 2700, 3, 100, ICE, 10, (MAGIC, 2860, 10, 100, ICE)),
                magic_type::FLAMING_DAGGERS => (
                    MAGIC_EX5,
                    3900,
                    7,
                    100,
                    FIRE,
                    10,
                    (MAGIC_EX5, 4100, 8, 100, FIRE),
                ),
                magic_type::SHREDDING => (
                    MAGIC_EX5,
                    4300,
                    5,
                    100,
                    FIRE,
                    10,
                    (MAGIC_EX5, 4500, 10, 100, FIRE),
                ),
                magic_type::LIGHTNING_BALL => (
                    MAGIC,
                    3070,
                    6,
                    100,
                    LIGHTNING,
                    10,
                    (MAGIC, 3230, 10, 100, LIGHTNING),
                ),
                magic_type::GUST_BLAST => (
                    MAGIC_EX,
                    430,
                    5,
                    100,
                    WIND,
                    10,
                    (MAGIC_EX, 590, 10, 100, WIND),
                ),
                magic_type::ADAMANTINE_FIRE_BALL => {
                    (MAGIC, 1640, 6, 100, FIRE, 10, (MAGIC, 1800, 10, 100, FIRE))
                }
                magic_type::ICE_BLADES => (MAGIC, 2960, 6, 50, ICE, 0, (MAGIC, 2970, 10, 100, ICE)),
                magic_type::EXPLOSIVE_TALISMAN => {
                    (MAGIC, 980, 3, 100, DARK, 10, (MAGIC, 1140, 10, 100, DARK))
                }
                magic_type::EVIL_SLAYER => {
                    (MAGIC, 3330, 6, 100, HOLY, 0, (MAGIC, 3340, 10, 100, HOLY))
                }
                magic_type::GREATER_EVIL_SLAYER => {
                    (MAGIC, 3440, 6, 50, HOLY, 0, (MAGIC, 3450, 10, 100, HOLY))
                }
                magic_type::MAGIC_RESISTANCE => {
                    (MAGIC, 980, 3, 100, WHITE, 10, (MAGIC, 200, 8, 100, WHITE))
                }
                _ => (MAGIC, 980, 3, 100, WHITE, 10, (MAGIC, 170, 8, 100, WHITE)),
            };
            for a in anchors {
                out.projectiles.push((
                    caster_cell,
                    a,
                    Projectile {
                        library,
                        start,
                        count,
                        delay_ms: delay,
                        color,
                        from: caster_cell,
                        to: a,
                        started: now,
                        duration: 0,
                        dir16: 0,
                        dir_stride: stride,
                        explode: Some(explode),
                    },
                ));
            }
        }
        magic_type::CYCLONE => {
            for a in anchors {
                out.effects
                    .push(Effect::at(MAGIC_EX, 1990, 5, 100, WIND, a, now));
                out.effects
                    .push(Effect::at(MAGIC_EX, 2000, 8, 100, WIND, a, now + 500));
            }
        }
        magic_type::MASS_HEAL => {
            for a in anchors {
                out.effects
                    .push(Effect::at(MAGIC, 670, 7, 100, HOLY, a, now));
            }
        }
        magic_type::SCORCHED_EARTH | magic_type::FROZEN_EARTH => {
            for p in locations {
                let wait = caster_cell.distance(*p) as u64 * 50;
                let a = Anchor::Cell(*p);
                if magic == magic_type::SCORCHED_EARTH {
                    out.effects
                        .push(Effect::at(MAGIC, 1900, 30, 50, FIRE, a, now + wait));
                    out.effects.push(Effect::at(
                        PROG_USE,
                        220,
                        1,
                        3500,
                        [1.0, 1.0, 1.0, 0.8],
                        a,
                        now + 500 + wait,
                    ));
                    out.effects
                        .push(Effect::at(MAGIC, 2450, 10, 250, WHITE, a, now + 500 + wait));
                } else {
                    out.effects
                        .push(Effect::at(MAGIC_EX, 90, 20, 50, ICE, a, now + wait));
                    out.effects.push(Effect::at(
                        PROG_USE,
                        260,
                        1,
                        2500,
                        [1.0, 1.0, 1.0, 0.8],
                        a,
                        now + wait + 1000,
                    ));
                }
            }
        }
        magic_type::LIGHTNING_BEAM => {
            // One directional beam on the caster toward the reported cell.
            if let Some(p) = locations.first() {
                let dir = mir_proto::Direction::from_points(caster_cell, *p);
                out.effects.push(Effect {
                    library: MAGIC_EX,
                    start: 1180,
                    count: 4,
                    delay_ms: 100,
                    color: LIGHTNING,
                    anchor: Anchor::Cell(caster_cell),
                    direction: Some(dir.index()),
                    started: now,
                });
            }
        }
        magic_type::BLOW_EARTH => {
            if let Some(last) = locations.last() {
                let a = Anchor::Cell(*last);
                out.projectiles.push((
                    caster_cell,
                    a,
                    Projectile {
                        library: MAGIC_EX,
                        start: 1990,
                        count: 5,
                        delay_ms: 100,
                        color: WIND,
                        from: caster_cell,
                        to: a,
                        started: now,
                        duration: 0,
                        dir16: 0,
                        dir_stride: 0,
                        explode: Some((MAGIC_EX, 2000, 8, 100, WIND)),
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
