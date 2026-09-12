//! Game state, input and scene rendering.
//!
//! Screen geometry follows `MapControl` in Zircon: 48x32 cells, the player's
//! cell centred horizontally and 34 px above centre, back tiles drawn 2x2,
//! middle/front tiles bottom-anchored, objects y-sorted per row.

use std::collections::HashMap;

use mir_formats::zl::SurfaceKind;
use mir_formats::MapFile;
use mir_proto::{
    Action, Appearance, BeltLink, CharacterSummary, Class, ClientMessage, Direction, Gender,
    ItemInstance, ObjectId, ObjectState, PlayerStats, Point, ServerMessage, Weights,
    EQUIPMENT_SIZE, INVENTORY_SIZE, MAX_BELT,
};

use crate::anim::{ClientObject, Queued};
use crate::assets::{
    armour_library, helmet_library, kr_library, lib, shield_library, weapon_library, Assets,
};
use crate::effects::{self, Anchor, Effect, Projectile};
use crate::gfx::{Blend, Gpu, SpriteKey, SpriteRegion, SpriteRenderer, Surface};
use crate::items::ItemCatalog;
use crate::monster_table::monster_sprite;
use crate::net::Connection;
use crate::text::TextLayer;
use crate::ui::{Ctx, Input};
use crate::windows::{Bag, NpcDialog, WindowState};
use mir_proto::{buff_type, magic_type, MagicSummary};
use std::collections::{HashMap as StdHashMap, HashSet};

pub const CELL_W: i32 = 48;
pub const CELL_H: i32 = 32;
const MANUAL_HEIGHT_OFFSET: i32 = 34;
use mir_proto::rules::{attack_delay, use_item_lock, ATTACK_TIME, MOVE_TIME, TURN_TIME};

/// A spell payload waiting for the cast animation: (time, magic, caster cell, targets, cells).
type PendingPayload = (u64, u16, Point, Vec<ObjectId>, Vec<Point>);

pub struct Game {
    pub assets: Assets,
    /// The character we entered the world with (name, look).
    pub character: Option<CharacterSummary>,
    pub status: String,
    map: Option<MapFile>,
    map_name: String,
    objects: HashMap<ObjectId, ClientObject>,
    user: Option<ObjectId>,
    stats: PlayerStats,
    pub mouse: (f32, f32),
    pub lmb: bool,
    pub rmb: bool,
    action_time: u64,
    move_time: u64,
    attack_time: u64,
    animation: u32,
    animation_time: u64,
    chat: Vec<(String, u64)>,
    hovered: Option<ObjectId>,
    pub debug: bool,
    pub input: Input,
    pub catalog: ItemCatalog,
    inventory: Vec<Option<ItemInstance>>,
    equipment: Vec<Option<ItemInstance>>,
    gold: u64,
    weights: Weights,
    windows: WindowState,
    /// Object we are walking toward to interact with (NPC or ground item).
    goal: Option<ObjectId>,
    windows_open_last_frame: bool,
    pending_messages: Vec<ClientMessage>,
    magics: Vec<MagicSummary>,
    /// Stance skills currently on (Thrusting, Half Moon).
    toggles: HashSet<u16>,
    /// Slaying's power attack is charged.
    slaying_ready: bool,
    cooldowns: StdHashMap<u16, u64>,
    magic_time: u64,
    effects: Vec<Effect>,
    projectiles: Vec<Projectile>,
    /// Spell payloads waiting for the cast animation: (time, magic, caster cell, targets, cells).
    pending_payloads: Vec<PendingPayload>,
    /// Belt links, one per slot (Zircon `BeltDialog.Links`).
    belt: Vec<BeltLink>,
    /// Zircon `GameScene.UseItemTime`: no consumable before this.
    use_item_time: u64,
    /// Own buffs: (kind, client time it ends; MAX = never).
    buffs: Vec<(u16, u64)>,
    /// A charged warrior power attack waiting for the next swing.
    charged: Option<u16>,
    /// Lotus skill armed for the next swing (Zircon `User.AttackMagic`).
    armed_lotus: Option<u16>,
    /// Moon charge the server armed (Calamity Of Full Moon / Waning Moon).
    auto_charged: Option<u16>,
}

struct View {
    off_x: i32,
    off_y: i32,
    pox: i32,
    poy: i32,
    ux: i32,
    uy: i32,
    umx: i32,
    umy: i32,
}

impl View {
    fn new(w: i32, h: i32, user: Option<&ClientObject>) -> View {
        let off_x = w / 2 / CELL_W;
        let off_y = h / 2 / CELL_H;
        let (ux, uy, umx, umy) = user
            .map(|u| {
                (
                    u.location.x,
                    u.location.y,
                    u.moving_offset.0,
                    u.moving_offset.1,
                )
            })
            .unwrap_or((0, 0, 0, 0));
        View {
            off_x,
            off_y,
            pox: (w - CELL_W) / 2 - off_x * CELL_W,
            poy: (h - CELL_H) / 2 - off_y * CELL_H - MANUAL_HEIGHT_OFFSET,
            ux,
            uy,
            umx,
            umy,
        }
    }
    /// Top-aligned pixel position of a cell.
    fn cell_px(&self, x: i32, y: i32) -> (i32, i32) {
        (
            (x - self.ux + self.off_x) * CELL_W + self.pox - self.umx,
            (y - self.uy + self.off_y) * CELL_H + self.poy - self.umy,
        )
    }
    fn object_px(&self, o: &ClientObject) -> (i32, i32) {
        let (x, y) = self.cell_px(o.location.x, o.location.y);
        (x + o.moving_offset.0, y + o.moving_offset.1)
    }
    fn cell_at(&self, mx: f32, my: f32) -> Point {
        let cx = ((mx as i32 - self.pox + self.umx).div_euclid(CELL_W)) + self.ux - self.off_x;
        let cy = ((my as i32 - self.poy + self.umy).div_euclid(CELL_H)) + self.uy - self.off_y;
        Point::new(cx, cy)
    }
}

#[allow(clippy::too_many_arguments)]
mod fx;
mod hud;
mod input;
mod net;
mod render;

impl Game {
    pub fn new(assets: Assets, catalog: ItemCatalog) -> Game {
        Game {
            assets,
            catalog,
            input: Input::default(),
            inventory: (0..INVENTORY_SIZE).map(|_| None).collect(),
            equipment: (0..EQUIPMENT_SIZE).map(|_| None).collect(),
            gold: 0,
            weights: Weights {
                bag: 0,
                max_bag: 1,
                wear: 0,
                max_wear: 1,
                hand: 0,
                max_hand: 1,
            },
            windows: {
                let mut w = WindowState::default();
                w.inventory_open = std::env::var_os("ZIRCON_OPEN_WINDOWS").is_some();
                w.character_open = w.inventory_open;
                w.skills_open = std::env::var_os("ZIRCON_OPEN_SKILLS").is_some();
                w
            },
            goal: None,
            windows_open_last_frame: false,
            pending_messages: Vec::new(),
            magics: Vec::new(),
            toggles: HashSet::new(),
            slaying_ready: false,
            cooldowns: StdHashMap::new(),
            magic_time: 0,
            effects: Vec::new(),
            projectiles: Vec::new(),
            pending_payloads: Vec::new(),
            belt: (0..MAX_BELT as u8).map(BeltLink::empty).collect(),
            use_item_time: 0,
            buffs: Vec::new(),
            charged: None,
            armed_lotus: None,
            auto_charged: None,
            character: None,
            status: String::new(),
            map: None,
            map_name: String::new(),
            objects: HashMap::new(),
            user: None,
            stats: PlayerStats {
                level: 1,
                hp: 0,
                max_hp: 1,
                mp: 0,
                max_mp: 1,
                experience: 0,
                max_experience: 100,
                min_dc: 0,
                max_dc: 0,
                min_ac: 0,
                max_ac: 0,
                accuracy: 0,
                agility: 0,
                attack_speed: 0,
            },
            mouse: (0.0, 0.0),
            lmb: false,
            rmb: false,
            action_time: 0,
            move_time: 0,
            attack_time: 0,
            animation: 0,
            animation_time: 0,
            chat: Vec::new(),
            hovered: None,
            debug: true,
        }
    }

    /// Name of the character in play.
    pub fn user_name(&self) -> Option<&str> {
        self.character.as_ref().map(|c| c.name.as_str())
    }

    /// Something the player may attack: a living monster that is not a pet
    /// of theirs.
    fn attackable(&self, o: &ClientObject) -> bool {
        o.is_monster() && !o.dead && !(o.pet_owner().is_some() && o.pet_owner() == self.user_name())
    }

    fn user(&self) -> Option<&ClientObject> {
        self.user.and_then(|id| self.objects.get(&id))
    }

    fn user_mut(&mut self) -> Option<&mut ClientObject> {
        let id = self.user?;
        self.objects.get_mut(&id)
    }

    fn say(&mut self, text: String, now: u64) {
        tracing::info!("{text}");
        self.chat.push((text, now));
        if self.chat.len() > 8 {
            self.chat.remove(0);
        }
    }

    /// True if a window consumed Escape this frame.
    pub fn windows_were_open(&self) -> bool {
        self.windows_open_last_frame
    }

    pub fn update(&mut self, now: u64, width: i32, height: i32, conn: Option<&Connection>) {
        if now >= self.animation_time + 100 {
            self.animation_time = now;
            self.animation = self.animation.wrapping_add(1);
        }
        for o in self.objects.values_mut() {
            o.process(now);
        }
        self.advance_effects(now, width, height);
        self.hovered = self.hit_test(width, height);
        // Developer automation: ZIRCON_AUTO_CAST=<F key> (or m<magic id>) casts
        // at the nearest monster, with the mouse over it for cell casts.
        if let Ok(v) = std::env::var("ZIRCON_AUTO_CAST") {
            if now >= self.magic_time {
                let nearest = self.user().and_then(|u| {
                    self.objects
                        .values()
                        .filter(|o| {
                            o.is_monster() && !o.dead && o.location.distance(u.location) <= 6
                        })
                        .min_by_key(|o| o.location.distance(u.location))
                        .map(|o| o.id)
                });
                let by_id = v.strip_prefix('m').and_then(|s| s.parse::<u16>().ok());
                // Self casts do not need a monster in reach.
                let self_cast = by_id.map(magic_type::is_self_cast).unwrap_or(false);
                if nearest.is_some() || self_cast {
                    if let Some(t) = nearest {
                        let view = View::new(width, height, self.user());
                        if let Some(o) = self.objects.get(&t) {
                            let (px, py) = view.object_px(o);
                            self.mouse = (px as f32 + 24.0, py as f32 + 16.0);
                        }
                        self.hovered = Some(t);
                    }
                    if let Some(id) = by_id {
                        self.use_skill(id, now, width, height, conn);
                    } else if let Ok(f) = v.parse::<u8>() {
                        self.function_key(f, now, width, height, conn);
                    }
                }
            }
        }
        // Developer automation: ZIRCON_AUTO_BELT=1 links every consumable type
        // in the bag to the belt; ZIRCON_AUTO_USE=<slot> presses that belt key.
        if std::env::var_os("ZIRCON_AUTO_BELT").is_some() && !self.inventory.is_empty() {
            let mut free: Vec<u8> = self
                .belt
                .iter()
                .filter(|l| l.info.is_none() && l.item.is_none())
                .map(|l| l.slot)
                .collect();
            free.reverse();
            let mut linked: Vec<i32> = self.belt.iter().filter_map(|l| l.info).collect();
            for (i, it) in self.inventory.clone().iter().enumerate() {
                let Some(it) = it else { continue };
                let consumable = self
                    .catalog
                    .get(it.info)
                    .map(|d| d.item_type == mir_proto::item_type::CONSUMABLE)
                    .unwrap_or(false);
                if !consumable || linked.contains(&it.info) {
                    continue;
                }
                let Some(slot) = free.pop() else { break };
                let link = crate::windows::link_for(&self.catalog, &self.inventory, i as u8, slot);
                linked.push(it.info);
                self.apply_belt_link(link);
                if let Some(c) = conn {
                    c.send(ClientMessage::BeltLink {
                        slot: link.slot,
                        info: link.info,
                        item: link.item,
                    });
                }
            }
        }
        if let Some(slot) = std::env::var("ZIRCON_AUTO_USE")
            .ok()
            .and_then(|v| v.parse::<u8>().ok())
        {
            if now > 3000 && self.use_item_time == 0 {
                self.belt_key(slot, now, conn);
            }
        }
        self.handle_input(now, width, height, conn);
        if let Some(u) = self.user() {
            self.status = format!("{} ({}, {})", self.map_name, u.location.x, u.location.y);
        }
    }

    /// Messages produced by the windows, sent by the client shell.
    pub fn take_messages(&mut self) -> Vec<ClientMessage> {
        std::mem::take(&mut self.pending_messages)
    }
}

/// Zircon `Functions.GetAttackAnimation` for the swings we know.
fn attack_action(magic: Option<u16>) -> Action {
    match magic {
        Some(magic_type::HALF_MOON) | Some(magic_type::DESTRUCTIVE_SURGE) => Action::Attack2,
        Some(magic_type::DRAGON_RISE)
        | Some(magic_type::FULL_BLOOM)
        | Some(magic_type::WHITE_LOTUS)
        | Some(magic_type::RED_LOTUS) => Action::Attack5,
        Some(magic_type::BLADE_STORM) => Action::Attack6,
        _ => Action::Attack,
    }
}

/// Zircon `Functions.GetMagicAnimation`.
fn cast_action(magic: u16) -> Action {
    if magic_type::is_stance_anim(magic) || magic == magic_type::MASS_BECKON {
        Action::Stance
    } else if magic == magic_type::SWIFT_BLADE {
        Action::Attack
    } else if magic == magic_type::RAKE {
        Action::Attack5
    } else if magic_type::is_projectile_cast(magic) {
        Action::Cast1
    } else {
        Action::Cast2
    }
}

/// Zircon `BuffDialog` icon index in `CBIcons.Zl`.
fn buff_icon(kind: u16) -> u32 {
    match kind {
        buff_type::DEFIANCE => 97,
        buff_type::MIGHT => 96,
        buff_type::ENDURANCE => 95,
        buff_type::REFLECT_DAMAGE => 98,
        buff_type::RENOUNCE => 94,
        buff_type::STRENGTH_OF_FAITH => 141,
        buff_type::INVISIBILITY => 74,
        buff_type::ELEMENTAL_SUPERIORITY => 93,
        buff_type::BLOOD_LUST => 90,
        buff_type::CELESTIAL_LIGHT => 142,
        buff_type::TRANSPARENCY => 160,
        buff_type::CLOAK | buff_type::GHOST_WALK => 160,
        buff_type::MAGIC_SHIELD => 100,
        buff_type::HEAL => 78,
        buff_type::MAGIC_RESISTANCE => 92,
        buff_type::RESILIENCE => 91,
        buff_type::POISONOUS_CLOUD => 98,
        buff_type::FULL_BLOOM => 162,
        buff_type::WHITE_LOTUS => 163,
        buff_type::RED_LOTUS => 164,
        _ => 73,
    }
}
