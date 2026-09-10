//! Wire protocol shared by the Rust Zircon client and server.
//!
//! Every frame on the TCP stream is `u32 little-endian payload length` followed
//! by a [postcard](https://docs.rs/postcard) encoded [`ClientMessage`] or
//! [`ServerMessage`]. Both ends own this crate, so there is no need to match
//! the C# reflection-ordered packet ids.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;
pub const DEFAULT_PORT: u16 = 7000;
pub const MAX_FRAME_LEN: usize = 1 << 20;

/// Eight-way direction, in the same order Zircon's sprite sheets use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Direction {
    Up = 0,
    UpRight = 1,
    Right = 2,
    DownRight = 3,
    Down = 4,
    DownLeft = 5,
    Left = 6,
    UpLeft = 7,
}

impl Direction {
    pub const ALL: [Direction; 8] = [
        Direction::Up,
        Direction::UpRight,
        Direction::Right,
        Direction::DownRight,
        Direction::Down,
        Direction::DownLeft,
        Direction::Left,
        Direction::UpLeft,
    ];

    pub fn from_index(i: u8) -> Direction {
        Direction::ALL[(i & 7) as usize]
    }

    pub fn index(self) -> u8 {
        self as u8
    }

    /// Cell delta for one step in this direction (x right, y down).
    pub fn delta(self) -> (i32, i32) {
        match self {
            Direction::Up => (0, -1),
            Direction::UpRight => (1, -1),
            Direction::Right => (1, 0),
            Direction::DownRight => (1, 1),
            Direction::Down => (0, 1),
            Direction::DownLeft => (-1, 1),
            Direction::Left => (-1, 0),
            Direction::UpLeft => (-1, -1),
        }
    }

    pub fn opposite(self) -> Direction {
        Direction::from_index(self.index().wrapping_add(4))
    }

    pub fn rotate(self, steps: i8) -> Direction {
        Direction::from_index((self.index() as i8).wrapping_add(steps) as u8 & 7)
    }

    /// Direction from `from` toward `to` (Zircon's `Functions.DirectionFromPoint`).
    pub fn from_points(from: Point, to: Point) -> Direction {
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        if dx == 0 && dy == 0 {
            return Direction::Down;
        }
        // Angle in screen space; classify into 8 sectors.
        let angle = (dy as f64).atan2(dx as f64); // 0 = right, pi/2 = down
        let sector = ((angle / (std::f64::consts::PI / 4.0)).round() as i32).rem_euclid(8);
        match sector {
            0 => Direction::Right,
            1 => Direction::DownRight,
            2 => Direction::Down,
            3 => Direction::DownLeft,
            4 => Direction::Left,
            5 => Direction::UpLeft,
            6 => Direction::Up,
            _ => Direction::UpRight,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Point {
        Point { x, y }
    }
    pub fn step(self, dir: Direction, distance: i32) -> Point {
        let (dx, dy) = dir.delta();
        Point::new(self.x + dx * distance, self.y + dy * distance)
    }
    /// Chebyshev distance: the number of 8-way steps between two cells.
    pub fn distance(self, other: Point) -> i32 {
        (self.x - other.x).abs().max((self.y - other.y).abs())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObjectId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Gender {
    Male,
    Female,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Class {
    Warrior,
    Wizard,
    Taoist,
    Assassin,
}

impl Class {
    pub const ALL: [Class; 4] = [
        Class::Warrior,
        Class::Wizard,
        Class::Taoist,
        Class::Assassin,
    ];
    /// Zircon `MirClass` value (BaseStat.Class).
    pub fn mir_class(self) -> u8 {
        match self {
            Class::Warrior => 0,
            Class::Wizard => 1,
            Class::Taoist => 2,
            Class::Assassin => 3,
        }
    }
    /// Zircon `RequiredClass` flag (SafeZoneInfo.StartClass).
    pub fn flag(self) -> u8 {
        1 << self.mir_class()
    }
    pub fn name(self) -> &'static str {
        match self {
            Class::Warrior => "Warrior",
            Class::Wizard => "Wizard",
            Class::Taoist => "Taoist",
            Class::Assassin => "Assassin",
        }
    }
}

/// What an object looks like; enough for the client to pick sprites.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Appearance {
    Player {
        name: String,
        gender: Gender,
        class: Class,
        /// Armour sprite set index (0 = naked/basic clothes).
        armour: u16,
        /// Weapon sprite set index (0 = none).
        weapon: u16,
        hair: u8,
    },
    Monster {
        name: String,
        /// Zircon `MonsterImage` value; the client maps it to a library + base index.
        image: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Standing,
    Walking,
    Running,
    Attack,
    Struck,
    Die,
    Dead,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectState {
    pub id: ObjectId,
    pub appearance: Appearance,
    pub location: Point,
    pub direction: Direction,
    pub hp: i32,
    pub max_hp: i32,
    pub dead: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapDescriptor {
    /// File stem under `Map/`, e.g. "0" for `Map/0.map`.
    pub file: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerStats {
    pub level: u8,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub experience: u64,
    pub max_experience: u64,
}

/// Summary of a character on the select screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub id: u32,
    pub name: String,
    pub class: Class,
    pub gender: Gender,
    pub hair: u8,
    pub level: u8,
    /// Unix seconds of the last login, 0 if never played.
    pub last_login: u64,
    /// Name of the map the character is on ("" for a new character).
    pub location: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LoginResult {
    Success { characters: Vec<CharacterSummary> },
    Failed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NewAccountResult {
    Success,
    Failed { reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NewCharacterResult {
    Success { character: CharacterSummary },
    Failed { reason: String },
}

/// Account rules shared by client-side validation and the server.
pub mod rules {
    pub const EMAIL_MIN: usize = 3;
    pub const EMAIL_MAX: usize = 50;
    pub const PASSWORD_MIN: usize = 6;
    pub const PASSWORD_MAX: usize = 30;
    pub const NAME_MIN: usize = 3;
    pub const NAME_MAX: usize = 15;
    pub const MAX_CHARACTERS: usize = 4;
    pub const HAIR_TYPES: u8 = 10;

    pub fn valid_email(s: &str) -> bool {
        let n = s.chars().count();
        (EMAIL_MIN..=EMAIL_MAX).contains(&n)
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "@._-+".contains(c))
    }
    pub fn valid_password(s: &str) -> bool {
        let n = s.chars().count();
        (PASSWORD_MIN..=PASSWORD_MAX).contains(&n) && !s.chars().any(char::is_control)
    }
    /// Character names: letters and digits, must start with a letter.
    pub fn valid_name(s: &str) -> bool {
        let n = s.chars().count();
        (NAME_MIN..=NAME_MAX).contains(&n)
            && s.chars()
                .next()
                .map(|c| c.is_ascii_alphabetic())
                .unwrap_or(false)
            && s.chars().all(|c| c.is_ascii_alphanumeric())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientMessage {
    Hello {
        version: u16,
    },
    NewAccount {
        email: String,
        password: String,
    },
    Login {
        email: String,
        password: String,
    },
    NewCharacter {
        name: String,
        class: Class,
        gender: Gender,
        hair: u8,
    },
    DeleteCharacter {
        id: u32,
    },
    StartGame {
        id: u32,
    },
    /// Leave the map and return to the character list.
    Logout,
    Turn {
        direction: Direction,
    },
    Move {
        direction: Direction,
        run: bool,
    },
    Attack {
        direction: Direction,
    },
    Ping {
        nonce: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Sent once after `Hello` is accepted.
    Connected,
    NewAccountResult(NewAccountResult),
    LoginResult(LoginResult),
    NewCharacterResult(NewCharacterResult),
    DeleteCharacterResult {
        id: u32,
        ok: bool,
        reason: String,
    },
    /// The player left the map; the client should show the character list.
    LoggedOut {
        characters: Vec<CharacterSummary>,
    },
    Welcome {
        id: ObjectId,
        map: MapDescriptor,
        location: Point,
        direction: Direction,
        stats: PlayerStats,
    },
    Rejected {
        reason: String,
    },
    ObjectShow(ObjectState),
    ObjectRemove {
        id: ObjectId,
    },
    ObjectTurn {
        id: ObjectId,
        direction: Direction,
    },
    ObjectMove {
        id: ObjectId,
        from: Point,
        to: Point,
        direction: Direction,
        run: bool,
    },
    /// The server refused the requester's own move; resync to `location`.
    MoveDenied {
        location: Point,
        direction: Direction,
    },
    ObjectAttack {
        id: ObjectId,
        direction: Direction,
    },
    ObjectStruck {
        id: ObjectId,
        attacker: ObjectId,
        damage: i32,
    },
    HealthChanged {
        id: ObjectId,
        hp: i32,
        max_hp: i32,
    },
    ObjectDie {
        id: ObjectId,
    },
    ObjectRevive {
        id: ObjectId,
        location: Point,
        direction: Direction,
        hp: i32,
    },
    StatsChanged(PlayerStats),
    Chat {
        text: String,
    },
    Pong {
        nonce: u32,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ProtoError {
    #[error("frame too large: {0} bytes")]
    FrameTooLarge(usize),
    #[error("encode error: {0}")]
    Encode(postcard::Error),
    #[error("decode error: {0}")]
    Decode(postcard::Error),
}

/// Encode a message into a length-prefixed frame.
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, ProtoError> {
    let body = postcard::to_stdvec(msg).map_err(ProtoError::Encode)?;
    if body.len() > MAX_FRAME_LEN {
        return Err(ProtoError::FrameTooLarge(body.len()));
    }
    let mut frame = Vec::with_capacity(body.len() + 4);
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// Try to take one complete frame from the front of `buf`. Returns the decoded
/// message and drains the consumed bytes. `Ok(None)` means more data is needed.
pub fn decode_frame<T: for<'de> Deserialize<'de>>(
    buf: &mut Vec<u8>,
) -> Result<Option<T>, ProtoError> {
    if buf.len() < 4 {
        return Ok(None);
    }
    let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if len > MAX_FRAME_LEN {
        return Err(ProtoError::FrameTooLarge(len));
    }
    if buf.len() < 4 + len {
        return Ok(None);
    }
    let msg = postcard::from_bytes(&buf[4..4 + len]).map_err(ProtoError::Decode)?;
    buf.drain(..4 + len);
    Ok(Some(msg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let msg = ServerMessage::ObjectMove {
            id: ObjectId(7),
            from: Point::new(1, 2),
            to: Point::new(2, 3),
            direction: Direction::DownRight,
            run: false,
        };
        let mut buf = encode(&msg).unwrap();
        buf.extend_from_slice(&encode(&ServerMessage::Pong { nonce: 9 }).unwrap());
        let a: ServerMessage = decode_frame(&mut buf).unwrap().unwrap();
        assert_eq!(a, msg);
        let b: ServerMessage = decode_frame(&mut buf).unwrap().unwrap();
        assert_eq!(b, ServerMessage::Pong { nonce: 9 });
        assert!(decode_frame::<ServerMessage>(&mut buf).unwrap().is_none());
    }

    #[test]
    fn direction_from_points() {
        let o = Point::new(5, 5);
        assert_eq!(Direction::from_points(o, Point::new(5, 4)), Direction::Up);
        assert_eq!(
            Direction::from_points(o, Point::new(6, 4)),
            Direction::UpRight
        );
        assert_eq!(
            Direction::from_points(o, Point::new(6, 5)),
            Direction::Right
        );
        assert_eq!(
            Direction::from_points(o, Point::new(6, 6)),
            Direction::DownRight
        );
        assert_eq!(Direction::from_points(o, Point::new(5, 6)), Direction::Down);
        assert_eq!(
            Direction::from_points(o, Point::new(4, 6)),
            Direction::DownLeft
        );
        assert_eq!(Direction::from_points(o, Point::new(4, 5)), Direction::Left);
        assert_eq!(
            Direction::from_points(o, Point::new(4, 4)),
            Direction::UpLeft
        );
        assert_eq!(
            Direction::from_points(o, Point::new(15, 6)),
            Direction::Right
        );
    }
}
