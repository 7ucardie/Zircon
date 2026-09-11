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
use mir_proto::{magic_type, MagicSummary};
use std::collections::{HashMap as StdHashMap, HashSet};

pub const CELL_W: i32 = 48;
pub const CELL_H: i32 = 32;
const MANUAL_HEIGHT_OFFSET: i32 = 34;
const MOVE_TIME: u64 = 600;
const TURN_TIME: u64 = 300;
const ATTACK_TIME: u64 = 600;
const ATTACK_DELAY: u64 = 1500;

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

    // ---- network ---------------------------------------------------------------

    pub fn handle(&mut self, msg: ServerMessage, now: u64) {
        match msg {
            ServerMessage::Welcome {
                id,
                map,
                location,
                direction,
                stats,
            } => {
                self.load_map(&map.file, &map.name);
                self.objects.clear();
                let (name, gender, class, hair) = self
                    .character
                    .as_ref()
                    .map(|c| (c.name.clone(), c.gender, c.class, c.hair))
                    .unwrap_or_else(|| {
                        ("Player".into(), Gender::Male, mir_proto::Class::Warrior, 1)
                    });
                let state = ObjectState {
                    id,
                    appearance: Appearance::Player {
                        name,
                        gender,
                        class,
                        armour: 0,
                        weapon: None,
                        hair,
                        helmet: 0,
                        shield: None,
                    },
                    location,
                    direction,
                    hp: stats.hp,
                    max_hp: stats.max_hp,
                    dead: false,
                };
                self.objects.insert(id, ClientObject::new(&state, now));
                self.user = Some(id);
                self.stats = stats;
                self.status = format!("{} ({}, {})", map.name, location.x, location.y);
                self.say(format!("Welcome to {}.", map.name), now);
            }
            ServerMessage::Rejected { reason } => {
                self.status = format!("rejected: {reason}");
            }
            ServerMessage::ObjectShow(state) => {
                if Some(state.id) == self.user {
                    return;
                }
                // Developer automation: walk to and talk to a named NPC.
                if let (Ok(wanted), Appearance::Npc { name, .. }) =
                    (std::env::var("ZIRCON_AUTO_NPC"), &state.appearance)
                {
                    if name.eq_ignore_ascii_case(&wanted)
                        && self.goal.is_none()
                        && self.windows.npc.is_none()
                    {
                        self.goal = Some(state.id);
                    }
                }
                match self.objects.get_mut(&state.id) {
                    Some(o) => {
                        o.hp = state.hp;
                        o.max_hp = state.max_hp;
                        o.dead = state.dead;
                        o.snap(state.location, state.direction, now);
                    }
                    None => {
                        self.objects
                            .insert(state.id, ClientObject::new(&state, now));
                    }
                }
            }
            ServerMessage::ObjectRemove { id } => {
                if Some(id) != self.user {
                    self.objects.remove(&id);
                }
            }
            ServerMessage::ObjectTurn { id, direction } => {
                if let Some(o) = self.objects.get_mut(&id) {
                    let location = o.queue.back().map(|q| q.location).unwrap_or(o.location);
                    o.enqueue(Queued {
                        action: Action::Standing,
                        direction,
                        location,
                        distance: 0,
                    });
                }
            }
            ServerMessage::ObjectMove {
                id,
                from,
                to,
                direction,
                run,
            } => {
                if let Some(o) = self.objects.get_mut(&id) {
                    let pending_end = o.queue.back().map(|q| q.location);
                    let moving = matches!(o.action, Action::Walking | Action::Running);
                    if pending_end.is_none() && !moving && o.location != from {
                        o.location = from;
                    }
                    let distance = from.distance(to).max(1);
                    o.enqueue(Queued {
                        action: if run {
                            Action::Running
                        } else {
                            Action::Walking
                        },
                        direction,
                        location: to,
                        distance,
                    });
                }
            }
            ServerMessage::MoveDenied {
                location,
                direction,
            } => {
                if let Some(u) = self.user_mut() {
                    u.snap(location, direction, now);
                }
                self.move_time = 0;
                self.action_time = 0;
            }
            ServerMessage::ObjectAttack {
                id,
                direction,
                attack_magic,
            } => {
                if let Some(o) = self.objects.get_mut(&id) {
                    let location = o.queue.back().map(|q| q.location).unwrap_or(o.location);
                    o.enqueue(Queued {
                        action: if attack_magic == Some(magic_type::HALF_MOON) {
                            Action::Attack2
                        } else {
                            Action::Attack
                        },
                        direction,
                        location,
                        distance: 0,
                    });
                }
                if let Some(m) = attack_magic {
                    if let Some(e) = effects::attack_effect(m, id, direction.index(), now) {
                        self.effects.push(e);
                    }
                }
            }
            ServerMessage::ObjectMagic {
                id,
                direction,
                location,
                magic,
                targets,
                locations,
                cast,
            } => {
                if Some(id) != self.user {
                    if let Some(o) = self.objects.get_mut(&id) {
                        o.enqueue(Queued {
                            action: if magic_type::is_projectile_cast(magic) {
                                Action::Cast1
                            } else {
                                Action::Cast2
                            },
                            direction,
                            location,
                            distance: 0,
                        });
                    }
                }
                if let Some(e) = effects::cast_effect(magic, id, direction.index(), now) {
                    self.effects.push(e);
                }
                if cast {
                    self.pending_payloads
                        .push((now + 600, magic, location, targets, locations));
                }
            }
            ServerMessage::Magics(list) => self.magics = list,
            ServerMessage::BeltLinks(links) => {
                for l in links {
                    if let Some(slot) = self.belt.get_mut(l.slot as usize) {
                        *slot = l;
                    }
                }
            }
            ServerMessage::NewMagic(m) => {
                self.magics.retain(|x| x.magic != m.magic);
                self.magics.push(m);
            }
            ServerMessage::MagicLeveled {
                magic,
                level,
                experience,
            } => {
                if let Some(m) = self.magics.iter_mut().find(|m| m.magic == magic) {
                    m.level = level;
                    m.experience = experience;
                }
            }
            ServerMessage::MagicCooldown { magic, delay_ms } => {
                self.cooldowns.insert(magic, now + delay_ms as u64);
            }
            ServerMessage::MagicToggle { magic, on } => {
                if magic == magic_type::SLAYING {
                    self.slaying_ready = on;
                } else if on {
                    self.toggles.insert(magic);
                } else {
                    self.toggles.remove(&magic);
                }
            }
            ServerMessage::ObjectPoisoned { id, poisoned } => {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.poisoned = poisoned;
                }
            }
            ServerMessage::ObjectStruck {
                id,
                damage,
                element,
                magic,
                ..
            } => {
                if magic {
                    self.effects.push(effects::struck_effect(element, id, now));
                }
                if let Some(o) = self.objects.get_mut(&id) {
                    o.health_time = now + 5000;
                    o.damage.push((damage, now));
                    if !o.dead && !matches!(o.action, Action::Attack) {
                        let (direction, location) = o
                            .queue
                            .back()
                            .map(|q| (q.direction, q.location))
                            .unwrap_or((o.direction, o.location));
                        o.enqueue(Queued {
                            action: Action::Struck,
                            direction,
                            location,
                            distance: 0,
                        });
                    }
                }
            }
            ServerMessage::HealthChanged { id, hp, max_hp } => {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.hp = hp;
                    o.max_hp = max_hp;
                }
                if Some(id) == self.user {
                    self.stats.hp = hp;
                    self.stats.max_hp = max_hp;
                }
            }
            ServerMessage::ObjectDie { id } => {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.dead = true;
                    o.hp = 0;
                    let (direction, location) = o
                        .queue
                        .back()
                        .map(|q| (q.direction, q.location))
                        .unwrap_or((o.direction, o.location));
                    o.queue.clear();
                    o.enqueue(Queued {
                        action: Action::Die,
                        direction,
                        location,
                        distance: 0,
                    });
                }
                if Some(id) == self.user {
                    self.stats.hp = 0;
                }
            }
            ServerMessage::ObjectRevive {
                id,
                location,
                direction,
                hp,
            } => {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.dead = false;
                    o.hp = hp;
                    o.snap(location, direction, now);
                }
                if Some(id) == self.user {
                    self.stats.hp = hp;
                    self.say("You have been revived.".into(), now);
                }
            }
            ServerMessage::StatsChanged(stats) => self.stats = stats,
            ServerMessage::Inventory {
                inventory,
                equipment,
                gold,
                weights,
            } => {
                self.inventory = (0..INVENTORY_SIZE).map(|_| None).collect();
                self.equipment = (0..EQUIPMENT_SIZE).map(|_| None).collect();
                for (slot, item) in inventory {
                    if let Some(c) = self.inventory.get_mut(slot as usize) {
                        *c = Some(item);
                    }
                }
                for (slot, item) in equipment {
                    if let Some(c) = self.equipment.get_mut(slot as usize) {
                        *c = Some(item);
                    }
                }
                self.gold = gold;
                self.weights = weights;
            }
            ServerMessage::ItemChanged { grid, slot, item } => {
                let grid = match grid {
                    mir_proto::Grid::Inventory => &mut self.inventory,
                    mir_proto::Grid::Equipment => &mut self.equipment,
                };
                if let Some(c) = grid.get_mut(slot as usize) {
                    *c = item;
                }
                // Zircon clears item links whose item left the bag.
                for l in self.belt.iter_mut() {
                    let Some(item_id) = l.item else {
                        continue;
                    };
                    let present = self.inventory.iter().flatten().any(|it| it.id == item_id);
                    if !present {
                        l.item = None;
                        self.pending_messages.push(ClientMessage::BeltLink {
                            slot: l.slot,
                            info: None,
                            item: None,
                        });
                    }
                }
            }
            ServerMessage::GoldChanged { gold } => self.gold = gold,
            ServerMessage::WeightsChanged(w) => self.weights = w,
            ServerMessage::ObjectAppearance { id, appearance } => {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.appearance = appearance;
                }
            }
            ServerMessage::MapChanged {
                map,
                location,
                direction,
            } => {
                self.load_map(&map.file, &map.name);
                let user = self.user;
                self.objects.retain(|id, _| Some(*id) == user);
                self.goal = None;
                self.windows.npc = None;
                if let Some(u) = self.user_mut() {
                    u.snap(location, direction, now);
                }
                self.move_time = 0;
                self.action_time = 0;
                self.say(format!("Entered {}.", map.name), now);
            }
            ServerMessage::NpcResponse {
                npc,
                page,
                say,
                dialog_type,
                goods,
                sell_types,
            } => {
                self.windows.npc = Some(NpcDialog::new(
                    npc,
                    page,
                    &say,
                    dialog_type,
                    goods,
                    sell_types,
                ));
            }
            ServerMessage::NpcClose => self.windows.npc = None,
            ServerMessage::Chat { text } => self.say(text, now),
            ServerMessage::Pong { .. } => {}
            // Pre-game messages are handled by the client shell.
            ServerMessage::Connected
            | ServerMessage::NewAccountResult(_)
            | ServerMessage::LoginResult(_)
            | ServerMessage::NewCharacterResult(_)
            | ServerMessage::DeleteCharacterResult { .. }
            | ServerMessage::LoggedOut { .. } => {}
        }
    }

    // ---- update ------------------------------------------------------------------

    /// Reset all world state (when leaving the map).
    pub fn leave_world(&mut self) {
        self.map = None;
        self.objects.clear();
        self.user = None;
        self.chat.clear();
        self.hovered = None;
        self.windows = WindowState::default();
        self.goal = None;
        self.effects.clear();
        self.projectiles.clear();
        self.pending_payloads.clear();
        self.magics.clear();
        self.toggles.clear();
        self.slaying_ready = false;
    }

    /// True if a window consumed Escape this frame.
    pub fn windows_were_open(&self) -> bool {
        self.windows_open_last_frame
    }

    fn load_map(&mut self, file: &str, name: &str) {
        let path = self.assets.root().join(format!("Map/{file}.map"));
        match MapFile::load(&path) {
            Ok(m) => {
                tracing::info!(map = name, w = m.width, h = m.height, "map loaded");
                self.map = Some(m);
            }
            Err(e) => {
                self.status = format!("cannot load {}: {e}", path.display());
                tracing::error!("{}", self.status);
                self.map = None;
            }
        }
        self.map_name = name.to_string();
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
        // Developer automation: ZIRCON_AUTO_CAST=<F key> casts at the nearest monster.
        if let Some(f) = std::env::var("ZIRCON_AUTO_CAST")
            .ok()
            .and_then(|v| v.parse::<u8>().ok())
        {
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
                if let Some(t) = nearest {
                    self.hovered = Some(t);
                    self.function_key(f, now, width, height, conn);
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

    fn body_sprite(&self, o: &ClientObject) -> Option<(u16, u32)> {
        match &o.appearance {
            Appearance::Player {
                gender,
                armour,
                class,
                ..
            } => {
                let female = *gender == Gender::Female;
                let assassin = *class == Class::Assassin;
                let library =
                    armour_library(*armour, female, assassin).unwrap_or(match (assassin, female) {
                        (false, false) => lib::M_HUM,
                        (false, true) => lib::WM_HUM,
                        (true, false) => lib::M_HUM_A,
                        (true, true) => lib::WM_HUM_A,
                    });
                Some((library, o.sprite_index(*armour)))
            }
            Appearance::Monster { image, .. } => {
                let (library, shape) = monster_sprite(*image)?;
                Some((library, o.sprite_index(shape)))
            }
            Appearance::Npc { .. } => Some((lib::NPC, o.sprite_index(0))),
            Appearance::Item { info, .. } => {
                let image = self.catalog.get(*info).map(|d| d.image).unwrap_or(0);
                Some((lib::GROUND, image.max(0) as u32))
            }
        }
    }

    /// Weapon library and index for a player, if one is equipped.
    fn weapon_sprite(&self, o: &ClientObject) -> Option<(u16, u32)> {
        let Appearance::Player { gender, weapon, .. } = &o.appearance else {
            return None;
        };
        let shape = (*weapon)?;
        let library = weapon_library(shape, *gender == Gender::Female)?;
        let draw_shape = if shape >= 1000 { shape - 1000 } else { shape };
        Some((library, o.draw_frame() + (draw_shape as u32 % 10) * 5000))
    }

    fn hit_test(&mut self, width: i32, height: i32) -> Option<ObjectId> {
        let view = View::new(width, height, self.user());
        let (mx, my) = (self.mouse.0 as i32, self.mouse.1 as i32);
        let mut best: Option<(i32, ObjectId)> = None;
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in ids {
            let o = &self.objects[&id];
            if Some(id) == self.user || o.dead {
                continue;
            }
            let Some((library, index)) = self.body_sprite(o) else {
                continue;
            };
            let (dx, dy) = view.object_px(o);
            let ry = o.render_y();
            let Some(info) = self.assets.info(library, index) else {
                continue;
            };
            let (x0, y0) = if o.is_item() {
                (
                    dx + (CELL_W - info.width as i32) / 2,
                    dy + (CELL_H - info.height as i32) / 2,
                )
            } else {
                (dx + info.offset_x as i32, dy + info.offset_y as i32)
            };
            let inside =
                mx >= x0 && mx < x0 + info.width as i32 && my >= y0 && my < y0 + info.height as i32;
            if inside && best.map(|(y, _)| ry >= y).unwrap_or(true) {
                best = Some((ry, id));
            }
        }
        best.map(|(_, id)| id)
    }

    fn blocked(&self, p: Point) -> bool {
        let Some(map) = &self.map else {
            return true;
        };
        if !map.is_walkable(p.x, p.y) {
            return true;
        }
        self.objects
            .values()
            .any(|o| !o.dead && !o.is_item() && Some(o.id) != self.user && o.location == p)
    }

    fn handle_input(&mut self, now: u64, width: i32, height: i32, conn: Option<&Connection>) {
        // Keyboard shortcuts (Zircon defaults: Tab pick up, W bag, Q character).
        for ch in self.input.text.chars() {
            match ch.to_ascii_lowercase() {
                'w' | 'i' => self.windows.inventory_open = !self.windows.inventory_open,
                'q' | 'c' => self.windows.character_open = !self.windows.character_open,
                'e' | 's' => self.windows.skills_open = !self.windows.skills_open,
                'z' => self.windows.belt_open = !self.windows.belt_open,
                _ => {}
            }
        }
        if let Some(slot) = self.input.digit {
            self.belt_key(slot, now, conn);
        }
        if self.input.tab {
            if let Some(c) = conn {
                c.send(ClientMessage::PickUp);
            }
        }
        if let Some(f) = self.input.fkey {
            self.function_key(f, now, width, height, conn);
        }
        if self.input.escape && self.windows_open_last_frame {
            self.windows.inventory_open = false;
            self.windows.character_open = false;
            self.windows.skills_open = false;
            if self.windows.npc.take().is_some() {
                if let Some(c) = conn {
                    c.send(ClientMessage::NpcClose);
                }
            }
        }
        if self.input.lmb_pressed || self.input.rmb_pressed {
            self.goal = None;
        }
        let Some(user) = self.user() else {
            return;
        };
        if user.dead {
            return;
        }
        let user_loc = user.location;
        let user_dir = user.direction;
        let view = View::new(width, height, Some(user));

        // Interacting with a goal object once close enough.
        if let Some(gid) = self.goal {
            match self.objects.get(&gid) {
                Some(g) if user_loc.distance(g.location) <= 1 => {
                    if let Some(c) = conn {
                        if g.is_item() {
                            c.send(ClientMessage::PickUp);
                        } else if g.is_npc() {
                            c.send(ClientMessage::NpcCall { id: gid });
                        }
                    }
                    self.goal = None;
                    return;
                }
                Some(g) if now >= self.action_time && now >= self.move_time => {
                    let target = g.location;
                    self.step_toward(now, user_loc, target, false, conn);
                    return;
                }
                Some(_) => return,
                None => self.goal = None,
            }
        }

        if self.input.lmb_pressed {
            if let Some(target) = self.hovered.and_then(|id| self.objects.get(&id)) {
                if target.is_npc() || target.is_item() {
                    self.goal = Some(target.id);
                    return;
                }
            }
        }
        if !(self.lmb || self.rmb) {
            return;
        }

        // Attack a hovered monster in melee range.
        if self.lmb {
            if let Some(target) = self.hovered.and_then(|id| self.objects.get(&id)) {
                if target.is_monster() && !target.dead && user_loc.distance(target.location) <= 1 {
                    if now >= self.action_time && now >= self.attack_time {
                        let direction = Direction::from_points(user_loc, target.location);
                        self.action_time = now + ATTACK_TIME;
                        self.attack_time = now + ATTACK_DELAY;
                        // Zircon priority: Slaying, then Thrusting, then Half Moon (last wins).
                        let mut attack_magic = None;
                        if self.slaying_ready {
                            attack_magic = Some(magic_type::SLAYING);
                        }
                        if self.toggles.contains(&magic_type::THRUSTING) {
                            attack_magic = Some(magic_type::THRUSTING);
                        }
                        if self.toggles.contains(&magic_type::HALF_MOON) {
                            attack_magic = Some(magic_type::HALF_MOON);
                        }
                        let action = if attack_magic == Some(magic_type::HALF_MOON) {
                            Action::Attack2
                        } else {
                            Action::Attack
                        };
                        if let Some(u) = self.user_mut() {
                            u.queue.clear();
                            u.enqueue(Queued {
                                action,
                                direction,
                                location: user_loc,
                                distance: 0,
                            });
                        }
                        if let Some(m) = attack_magic {
                            if let Some(uid) = self.user {
                                if let Some(e) =
                                    effects::attack_effect(m, uid, direction.index(), now)
                                {
                                    self.effects.push(e);
                                }
                            }
                        }
                        if let Some(c) = conn {
                            c.send(ClientMessage::Attack {
                                direction,
                                attack_magic,
                            });
                        }
                    }
                    return;
                }
            }
        }

        let target = view.cell_at(self.mouse.0, self.mouse.1);
        if target == user_loc || now < self.action_time || now < self.move_time {
            return;
        }
        let run = self.rmb && user_loc.distance(target) >= 2;
        let _ = user_dir;
        self.step_toward(now, user_loc, target, run, conn);
    }

    /// F1..F11: bind in the skill window, toggle a stance, or cast.
    /// Digit key: link the carried/hovered bag item to the belt slot, else
    /// use what the slot links to (Zircon `UseBelt01..10`).
    fn belt_key(&mut self, slot: u8, now: u64, conn: Option<&Connection>) {
        let source = match self.windows.carrying {
            Some((mir_proto::Grid::Inventory, s)) => Some(s),
            _ => self.windows.hover_inventory,
        };
        if let Some(s) = source {
            if self.inventory.get(s as usize).map(|i| i.is_some()) == Some(true) {
                let link = crate::windows::link_for(&self.catalog, &self.inventory, s, slot);
                self.windows.carrying = None;
                self.apply_belt_link(link);
                if let Some(c) = conn {
                    c.send(ClientMessage::BeltLink {
                        slot: link.slot,
                        info: link.info,
                        item: link.item,
                    });
                }
                return;
            }
        }
        let Some(link) = self.belt.get(slot as usize).copied() else {
            return;
        };
        if let Some(inv) = crate::windows::belt_inventory_slot(&self.inventory, &link) {
            self.try_use_item(inv, now, conn);
        }
    }

    fn apply_belt_link(&mut self, link: BeltLink) {
        if let Some(l) = self.belt.get_mut(link.slot as usize) {
            *l = link;
        }
    }

    /// Zircon `DXItemCell.UseItem` for consumables: the client-side lock is
    /// `max(250, Durability)` ms; the server enforces its own.
    fn try_use_item(&mut self, slot: u8, now: u64, conn: Option<&Connection>) {
        let Some(item) = self.inventory.get(slot as usize).cloned().flatten() else {
            return;
        };
        let Some(def) = self.catalog.get(item.info) else {
            return;
        };
        if def.item_type == mir_proto::item_type::CONSUMABLE {
            if now < self.use_item_time {
                return;
            }
            self.use_item_time = now + (def.durability.max(250)) as u64;
        }
        if let Some(c) = conn {
            c.send(ClientMessage::ItemUse { slot });
        }
    }

    fn function_key(
        &mut self,
        f: u8,
        now: u64,
        width: i32,
        height: i32,
        conn: Option<&Connection>,
    ) {
        // Binding: hovering a learned skill's icon in the skill window.
        if let Some(magic) = self.windows.hover_magic {
            if self.magics.iter().any(|m| m.magic == magic) {
                if let Some(m) = self.magics.iter_mut().find(|m| m.key == f) {
                    m.key = 0;
                }
                if let Some(m) = self.magics.iter_mut().find(|m| m.magic == magic) {
                    m.key = f;
                }
                if let Some(c) = conn {
                    c.send(ClientMessage::MagicKey { magic, key: f });
                }
            }
            return;
        }
        let Some(m) = self.magics.iter().find(|m| m.key == f).cloned() else {
            return;
        };
        let Some(def) = self.catalog.magic(m.magic).cloned() else {
            return;
        };
        let magic = m.magic;
        if magic_type::is_passive(magic) {
            self.say(format!("{} works on its own.", def.name), now);
            return;
        }
        if magic_type::is_toggle(magic) {
            let on = !self.toggles.contains(&magic);
            if let Some(c) = conn {
                c.send(ClientMessage::MagicToggle { magic, on });
            }
            return;
        }
        if !magic_type::is_castable(magic) {
            self.say(format!("{} is not implemented yet.", def.name), now);
            return;
        }
        let Some(user) = self.user() else {
            return;
        };
        if user.dead {
            return;
        }
        let user_loc = user.location;
        if (self.stats.level as i32) < def.need_level[0] {
            self.say(
                format!("{} needs level {}.", def.name, def.need_level[0]),
                now,
            );
            return;
        }
        if now < self.magic_time || now < self.action_time {
            return;
        }
        if self
            .cooldowns
            .get(&magic)
            .map(|t| now < *t)
            .unwrap_or(false)
        {
            self.say(format!("{} is cooling down.", def.name), now);
            return;
        }
        if def.cost(m.level) > self.stats.mp {
            self.say("Not enough mana.".into(), now);
            return;
        }
        let view = View::new(width, height, Some(user));
        let mouse_cell = view.cell_at(self.mouse.0, self.mouse.1);
        let hovered = self.hovered.and_then(|id| self.objects.get(&id));
        let target = match magic {
            magic_type::HEAL => hovered
                .filter(|o| o.is_player())
                .map(|o| o.id)
                .or(self.user),
            magic_type::REPULSION => None,
            _ => hovered.filter(|o| o.is_monster() && !o.dead).map(|o| o.id),
        };
        let target_loc = target
            .and_then(|t| self.objects.get(&t))
            .map(|o| o.location)
            .unwrap_or(mouse_cell);
        if target_loc.distance(user_loc) > mir_proto::MAGIC_RANGE {
            self.say("Too far away.".into(), now);
            return;
        }
        let direction = if target_loc == user_loc {
            user.direction
        } else {
            Direction::from_points(user_loc, target_loc)
        };
        self.magic_time = now + mir_proto::MAGIC_DELAY;
        self.action_time = now + 600;
        let action = if magic_type::is_projectile_cast(magic) {
            Action::Cast1
        } else {
            Action::Cast2
        };
        if let Some(u) = self.user_mut() {
            u.queue.clear();
            u.enqueue(Queued {
                action,
                direction,
                location: user_loc,
                distance: 0,
            });
        }
        if let Some(c) = conn {
            c.send(ClientMessage::Magic {
                magic,
                direction,
                target,
                location: target_loc,
            });
        }
    }

    /// Expire finished effects, fly projectiles, release scheduled payloads.
    fn advance_effects(&mut self, now: u64, width: i32, height: i32) {
        let view = View::new(width, height, self.user());
        // Scheduled payloads (after the cast animation).
        let due: Vec<_> = self
            .pending_payloads
            .iter()
            .filter(|p| p.0 <= now)
            .cloned()
            .collect();
        self.pending_payloads.retain(|p| p.0 > now);
        for (_, magic, caster_cell, targets, locations) in due {
            let payload = effects::payload(magic, caster_cell, &targets, &locations, now);
            self.effects.extend(payload.effects);
            for (from, to, mut p) in payload.projectiles {
                let (fx, fy) = view.cell_px(from.x, from.y);
                let (tx, ty) = match to {
                    Anchor::Object(id) => match self.objects.get(&id) {
                        Some(o) => view.object_px(o),
                        None => continue,
                    },
                    Anchor::Cell(c) => view.cell_px(c.x, c.y),
                };
                let dist = (((tx - fx) as f32).powi(2) + ((ty - fy) as f32).powi(2)).sqrt();
                p.duration = dist.max(1.0) as u64;
                p.dir16 = effects::direction16((fx as f32, fy as f32), (tx as f32, ty as f32));
                self.projectiles.push(p);
            }
        }
        self.effects.retain(|e| e.frame(now).is_some());
        let mut arrived = Vec::new();
        self.projectiles.retain(|p| {
            if p.progress(now) >= 1.0 {
                arrived.push(p.clone());
                false
            } else {
                true
            }
        });
        for p in arrived {
            if let Some((library, start, count, delay, color)) = p.explode {
                self.effects.push(Effect {
                    library,
                    start,
                    count,
                    delay_ms: delay,
                    color,
                    anchor: p.to,
                    direction: None,
                    started: now,
                });
            }
        }
    }

    fn anchor_px(&self, view: &View, a: Anchor) -> Option<(i32, i32)> {
        match a {
            Anchor::Object(id) => self.objects.get(&id).map(|o| view.object_px(o)),
            Anchor::Cell(c) => Some(view.cell_px(c.x, c.y)),
        }
    }

    fn draw_effects(&mut self, view: &View, gpu: &Gpu, renderer: &mut SpriteRenderer, now: u64) {
        let effects = self.effects.clone();
        for e in effects {
            let Some(frame) = e.frame(now) else { continue };
            let Some((dx, dy)) = self.anchor_px(view, e.anchor) else {
                continue;
            };
            if let Some(info) = self.assets.info(e.library, frame) {
                if let Some(r) = Self::sprite(
                    &mut self.assets,
                    renderer,
                    gpu,
                    e.library,
                    frame,
                    Surface::Image,
                ) {
                    renderer.draw(
                        r,
                        (dx + info.offset_x as i32) as f32,
                        (dy + info.offset_y as i32) as f32,
                        e.color,
                        Blend::Screen,
                    );
                }
            }
        }
        let projectiles = self.projectiles.clone();
        for p in projectiles {
            let (fx, fy) = view.cell_px(p.from.x, p.from.y);
            let Some((tx, ty)) = self.anchor_px(view, p.to) else {
                continue;
            };
            let t = p.progress(now);
            let x = fx as f32 + (tx - fx) as f32 * t;
            let y = fy as f32 + (ty - fy) as f32 * t;
            let frame = p.frame(now);
            if let Some(info) = self.assets.info(p.library, frame) {
                if let Some(r) = Self::sprite(
                    &mut self.assets,
                    renderer,
                    gpu,
                    p.library,
                    frame,
                    Surface::Image,
                ) {
                    renderer.draw(
                        r,
                        x + info.offset_x as f32,
                        y + info.offset_y as f32,
                        p.color,
                        Blend::Screen,
                    );
                }
            }
        }
    }

    /// One walk/run step toward `target`, turning if blocked.
    fn step_toward(
        &mut self,
        now: u64,
        user_loc: Point,
        target: Point,
        run: bool,
        conn: Option<&Connection>,
    ) {
        let user_dir = self.user().map(|u| u.direction).unwrap_or(Direction::Down);
        let wanted = Direction::from_points(user_loc, target);
        let steps = if run { 2 } else { 1 };
        let candidates = [
            wanted,
            wanted.rotate(-1),
            wanted.rotate(1),
            wanted.rotate(-2),
            wanted.rotate(2),
        ];
        let chosen = candidates
            .iter()
            .copied()
            .find(|d| (1..=steps).all(|i| !self.blocked(user_loc.step(*d, i))));
        match chosen {
            Some(direction) => {
                let to = user_loc.step(direction, steps);
                self.action_time = now + MOVE_TIME;
                self.move_time = now + MOVE_TIME;
                if let Some(u) = self.user_mut() {
                    u.queue.clear();
                    u.enqueue(Queued {
                        action: if run {
                            Action::Running
                        } else {
                            Action::Walking
                        },
                        direction,
                        location: to,
                        distance: steps,
                    });
                }
                if let Some(c) = conn {
                    c.send(ClientMessage::Move { direction, run });
                }
            }
            None => {
                if user_dir != wanted {
                    self.action_time = now + TURN_TIME;
                    if let Some(u) = self.user_mut() {
                        u.queue.clear();
                        u.enqueue(Queued {
                            action: Action::Standing,
                            direction: wanted,
                            location: user_loc,
                            distance: 0,
                        });
                    }
                    if let Some(c) = conn {
                        c.send(ClientMessage::Turn { direction: wanted });
                    }
                }
            }
        }
    }

    // ---- rendering ----------------------------------------------------------------

    fn sprite(
        assets: &mut Assets,
        renderer: &mut SpriteRenderer,
        gpu: &Gpu,
        library: u16,
        index: u32,
        surface: Surface,
    ) -> Option<SpriteRegion> {
        let key = SpriteKey {
            library,
            index,
            surface,
        };
        let kind = match surface {
            Surface::Image => SurfaceKind::Image,
            Surface::Shadow => SurfaceKind::Shadow,
            Surface::Overlay => SurfaceKind::Overlay,
        };
        renderer.sprite(gpu, key, || assets.decode(library, index, kind))
    }

    pub fn render(
        &mut self,
        gpu: &Gpu,
        renderer: &mut SpriteRenderer,
        text: &mut TextLayer,
        width: i32,
        height: i32,
        now: u64,
        fps: f32,
    ) {
        let white = [1.0, 1.0, 1.0, 1.0];
        let view = View::new(width, height, self.user());
        let map_taken = self.map.take();
        if let (Some(map), Some(_)) = (map_taken.as_ref(), self.user) {
            let (ux, uy) = (view.ux, view.uy);
            let x_range =
                (ux - view.off_x - 4).max(0)..=(ux + view.off_x + 4).min(map.width as i32 - 1);
            let y_range =
                (uy - view.off_y - 4).max(0)..=(uy + view.off_y + 4).min(map.height as i32 - 1);

            // Back layer: 96x64 tiles on even cells.
            for y in y_range.clone() {
                if y % 2 != 0 {
                    continue;
                }
                for x in x_range.clone() {
                    if x % 2 != 0 {
                        continue;
                    }
                    let Some(cell) = map.cell(x, y) else { continue };
                    let Some(library) = kr_library(cell.back_file) else {
                        continue;
                    };
                    let (dx, dy) = view.cell_px(x, y);
                    if let Some(r) = Self::sprite(
                        &mut self.assets,
                        renderer,
                        gpu,
                        library,
                        cell.back_image as u32,
                        Surface::Image,
                    ) {
                        renderer.draw(r, dx as f32, dy as f32, white, Blend::Alpha);
                    }
                }
            }

            // Rows: middle tiles, front tiles, then objects.
            let mut rows: HashMap<i32, Vec<ObjectId>> = HashMap::new();
            for o in self.objects.values() {
                rows.entry(o.render_y()).or_default().push(o.id);
            }
            let row_range =
                (uy - view.off_y - 4).max(0)..=(uy + view.off_y + 25).min(map.height as i32 - 1);
            for y in row_range {
                let draw_y = (y - uy + view.off_y + 1) * CELL_H + view.poy - view.umy;
                for layer in 0..2 {
                    for x in x_range.clone() {
                        let Some(cell) = map.cell(x, y) else { continue };
                        let (file, raw_index, animated, blend_flag, count) = if layer == 0 {
                            (
                                cell.middle_file,
                                cell.middle_image,
                                cell.middle_animated(),
                                cell.middle_anim_blend(),
                                cell.middle_anim_count(),
                            )
                        } else {
                            (
                                cell.front_file,
                                cell.front_image,
                                cell.front_animated(),
                                cell.front_anim_blend(),
                                cell.front_anim_count(),
                            )
                        };
                        if file == 0 {
                            continue;
                        }
                        let Some(library) = kr_library(file) else {
                            continue;
                        };
                        let mut index = raw_index as u32;
                        let mut blend = false;
                        if animated {
                            blend = blend_flag;
                            if count > 0 {
                                index += self.animation % count as u32;
                            }
                        }
                        let Some(info) = self.assets.info(library, index) else {
                            continue;
                        };
                        let (w, h) = (info.width as i32, info.height as i32);
                        let cell_sized = (w == 48 && h == 32) || (w == 96 && h == 64);
                        let draw_x = (x - ux + view.off_x) * CELL_W + view.pox - view.umx;
                        let (py, use_blend) = if layer == 0 {
                            (draw_y - h, blend && !cell_sized)
                        } else {
                            (
                                if cell_sized {
                                    draw_y - CELL_H
                                } else {
                                    draw_y - h
                                },
                                blend,
                            )
                        };
                        if let Some(r) = Self::sprite(
                            &mut self.assets,
                            renderer,
                            gpu,
                            library,
                            index,
                            Surface::Image,
                        ) {
                            renderer.draw(
                                r,
                                draw_x as f32,
                                py as f32,
                                white,
                                if use_blend {
                                    Blend::Screen
                                } else {
                                    Blend::Alpha
                                },
                            );
                        }
                    }
                }
                if let Some(ids) = rows.get(&y) {
                    for id in ids {
                        self.draw_object(*id, &view, gpu, renderer);
                    }
                }
            }
            self.draw_effects(&view, gpu, renderer, now);

            // Overlay: names, health bars, damage numbers.
            let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
            for id in ids {
                let o = &self.objects[&id];
                let (dx, dy) = view.object_px(o);
                if dx < -100 || dx > width + 100 || dy < -150 || dy > height + 100 {
                    continue;
                }
                if o.is_item() {
                    if let Appearance::Item { info, count } = &o.appearance {
                        let mut label = self.catalog.name(*info);
                        if *count > 1 {
                            label = format!("{label} ({count})");
                        }
                        let w = text.width(&label, 11) + 6.0;
                        let (lx, ly) = (dx as f32 + 24.0 - w / 2.0, dy as f32 - 4.0);
                        renderer.fill_rect(
                            lx,
                            ly,
                            w,
                            15.0,
                            [0.0, 24.0 / 255.0, 48.0 / 255.0, 0.75],
                        );
                        text.draw_centered(
                            &label,
                            11,
                            dx as f32 + 24.0,
                            ly - 1.0,
                            [255, 255, 255, 255],
                        );
                    }
                    continue;
                }
                let name_color = if o.is_npc() {
                    [0, 255, 0, 255]
                } else if o.is_player() {
                    [255, 255, 255, 255]
                } else {
                    [255, 255, 255, 220]
                };
                let name_y = if o.dead { dy + 21 } else { dy - 6 };
                text.draw_centered(o.name(), 12, dx as f32 + 24.0, name_y as f32, name_color);
                let show_bar =
                    !o.is_player() && !o.dead && (now < o.health_time || self.hovered == Some(id));
                if show_bar {
                    if let Some(bg) = Self::sprite(
                        &mut self.assets,
                        renderer,
                        gpu,
                        lib::INTERFACE,
                        80,
                        Surface::Image,
                    ) {
                        renderer.draw(bg, dx as f32, (dy - 55) as f32, white, Blend::Alpha);
                    }
                    if let Some(fill) = Self::sprite(
                        &mut self.assets,
                        renderer,
                        gpu,
                        lib::INTERFACE,
                        79,
                        Surface::Image,
                    ) {
                        let pct = o.hp.max(0) as f32 / o.max_hp.max(1) as f32;
                        renderer.draw_cropped(
                            fill,
                            (dx + 1) as f32,
                            (dy - 54) as f32,
                            pct,
                            [0.0, 200.0 / 255.0, 74.0 / 255.0, 1.0],
                        );
                    }
                }
                for (dmg, t) in &o.damage {
                    let age = now.saturating_sub(*t) as f32 / 1500.0;
                    let rise = age * 40.0;
                    let alpha = ((1.0 - age) * 255.0) as u8;
                    let color = if Some(id) == self.user {
                        [255, 80, 80, alpha]
                    } else {
                        [255, 220, 60, alpha]
                    };
                    text.draw_centered(
                        &format!("-{dmg}"),
                        16,
                        dx as f32 + 24.0,
                        (dy - 70) as f32 - rise,
                        color,
                    );
                }
            }
        } else {
            text.draw_centered(
                &self.status,
                18,
                width as f32 / 2.0,
                height as f32 / 2.0,
                [255, 255, 255, 255],
            );
        }
        self.map = map_taken;

        self.draw_hud(gpu, renderer, text, width, height, now, fps);
        self.draw_windows(gpu, renderer, text, width, height, now);
    }

    fn draw_object(&mut self, id: ObjectId, view: &View, gpu: &Gpu, renderer: &mut SpriteRenderer) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let Some((library, index)) = self.body_sprite(o) else {
            return;
        };
        let (dx, dy) = view.object_px(o);
        if o.is_item() {
            if let Some(info) = self.assets.info(library, index) {
                let x = dx + (CELL_W - info.width as i32) / 2;
                let y = dy + (CELL_H - info.height as i32) / 2;
                if let Some(r) = Self::sprite(
                    &mut self.assets,
                    renderer,
                    gpu,
                    library,
                    index,
                    Surface::Image,
                ) {
                    renderer.draw(r, x as f32, y as f32, [1.0, 1.0, 1.0, 1.0], Blend::Alpha);
                }
            }
            return;
        }
        let weapon = self.weapon_sprite(o);
        let direction = o.direction;
        // Zircon: WeaponLibrary1 is behind the body for Up/DownLeft/Left/UpLeft.
        let weapon_behind = matches!(
            direction,
            Direction::Up | Direction::DownLeft | Direction::Left | Direction::UpLeft
        );
        // Helmet replaces hair; shields sit behind the body when facing
        // right (Zircon `DrawBody`).
        let (helmet, shield) = match &o.appearance {
            Appearance::Player {
                helmet,
                shield,
                gender,
                class,
                ..
            } => {
                let female = *gender == Gender::Female;
                let assassin = *class == Class::Assassin;
                let stride = if assassin { 3000 } else { 5000 };
                let h = if *helmet > 0 {
                    helmet_library(*helmet, female, assassin)
                        .map(|l| (l, o.draw_frame() + ((*helmet as u32 - 1) % 10) * stride))
                } else {
                    None
                };
                let s = shield.and_then(|sh| {
                    shield_library(sh, female)
                        .map(|l| (l, o.draw_frame() + (sh as u32 % 10) * stride))
                });
                (h, s)
            }
            _ => (None, None),
        };
        let has_helmet = matches!(&o.appearance, Appearance::Player { helmet, .. } if *helmet > 0);
        let shield_behind = matches!(
            direction,
            Direction::UpRight | Direction::Right | Direction::DownRight
        );
        let hair = match &o.appearance {
            Appearance::Player {
                hair,
                gender,
                class,
                ..
            } if *hair > 0 && !has_helmet => {
                let hair_lib = match (*class == Class::Assassin, *gender == Gender::Female) {
                    (false, false) => lib::M_HAIR,
                    (false, true) => lib::WM_HAIR,
                    (true, false) => lib::M_HAIR_A,
                    (true, true) => lib::WM_HAIR_A,
                };
                Some((hair_lib, o.draw_frame() + (*hair as u32 - 1) * 5000))
            }
            _ => None,
        };
        if let (Some((wl, wi)), true) = (weapon, weapon_behind) {
            self.draw_layer(wl, wi, dx, dy, gpu, renderer);
        }
        if let (Some((sl, si)), true) = (shield, shield_behind) {
            self.draw_layer(sl, si, dx, dy, gpu, renderer);
        }
        let Some(info) = self.assets.info(library, index) else {
            return;
        };
        let white = [1.0, 1.0, 1.0, 1.0];

        // Shadow: baked surface if present, else Zircon's sheared fallback.
        if info.has_surface(SurfaceKind::Shadow) {
            if let Some(r) = Self::sprite(
                &mut self.assets,
                renderer,
                gpu,
                library,
                index,
                Surface::Shadow,
            ) {
                renderer.draw(
                    r,
                    (dx + info.shadow_offset_x as i32) as f32,
                    (dy + info.shadow_offset_y as i32) as f32,
                    [1.0, 1.0, 1.0, 0.5],
                    Blend::Alpha,
                );
            }
        } else if matches!(info.shadow_type, 177 | 176 | 49) {
            if let Some(r) = Self::sprite(
                &mut self.assets,
                renderer,
                gpu,
                library,
                index,
                Surface::Image,
            ) {
                let (w, h) = (r.width as f32, r.height as f32);
                let tx = (dx + info.shadow_offset_x as i32) as f32 + h / 2.0;
                let ty = (dy + info.shadow_offset_y as i32) as f32;
                // (px, py) -> (px - 0.5*py + tx, 0.5*py + ty)
                let corners = [
                    [tx, ty],
                    [tx + w, ty],
                    [tx + w - 0.5 * h, ty + 0.5 * h],
                    [tx - 0.5 * h, ty + 0.5 * h],
                ];
                renderer.draw_quad(r, corners, [0.0, 0.0, 0.0, 0.5], Blend::Alpha);
            }
        }

        if let Some(r) = Self::sprite(
            &mut self.assets,
            renderer,
            gpu,
            library,
            index,
            Surface::Image,
        ) {
            let tint = if self.objects.get(&id).map(|o| o.poisoned).unwrap_or(false) {
                [0.4, 1.0, 0.4, 1.0]
            } else if self.hovered == Some(id) {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                white
            };
            renderer.draw(
                r,
                (dx + info.offset_x as i32) as f32,
                (dy + info.offset_y as i32) as f32,
                tint,
                Blend::Alpha,
            );
        }
        if let Some((hl, hi)) = helmet {
            self.draw_layer(hl, hi, dx, dy, gpu, renderer);
        }
        if let Some((hair_lib, hair_index)) = hair {
            self.draw_layer(hair_lib, hair_index, dx, dy, gpu, renderer);
        }
        if let (Some((wl, wi)), false) = (weapon, weapon_behind) {
            self.draw_layer(wl, wi, dx, dy, gpu, renderer);
        }
        if let (Some((sl, si)), false) = (shield, shield_behind) {
            self.draw_layer(sl, si, dx, dy, gpu, renderer);
        }
    }

    /// Draw one equipment/hair layer with the image's own offsets.
    fn draw_layer(
        &mut self,
        library: u16,
        index: u32,
        dx: i32,
        dy: i32,
        gpu: &Gpu,
        renderer: &mut SpriteRenderer,
    ) {
        if let Some(info) = self.assets.info(library, index) {
            if let Some(r) = Self::sprite(
                &mut self.assets,
                renderer,
                gpu,
                library,
                index,
                Surface::Image,
            ) {
                renderer.draw(
                    r,
                    (dx + info.offset_x as i32) as f32,
                    (dy + info.offset_y as i32) as f32,
                    [1.0, 1.0, 1.0, 1.0],
                    Blend::Alpha,
                );
            }
        }
    }

    /// Inventory, character and NPC windows on top of everything; consumes
    /// clicks over them.
    fn draw_windows(
        &mut self,
        gpu: &Gpu,
        renderer: &mut SpriteRenderer,
        text: &mut TextLayer,
        width: i32,
        height: i32,
        now: u64,
    ) {
        let mut out = Vec::new();
        let over = {
            let Game {
                assets,
                windows,
                inventory,
                equipment,
                gold,
                weights,
                stats,
                catalog,
                input,
                character,
                magics,
                toggles,
                cooldowns,
                belt,
                use_item_time,
                objects,
                user,
                ..
            } = self;
            let dead = user
                .and_then(|u| objects.get(&u))
                .map(|o| o.dead)
                .unwrap_or(false);
            let mut c = Ctx {
                input,
                assets,
                renderer,
                gpu,
                text,
                now,
            };
            let view = crate::windows::PlayerView {
                level: stats.level,
                class: character.as_ref().map(|c| c.class.mir_class()).unwrap_or(0),
                hp: stats.hp,
                max_hp: stats.max_hp,
                mp: stats.mp,
                max_mp: stats.max_mp,
                min_dc: stats.min_dc,
                max_dc: stats.max_dc,
                min_ac: stats.min_ac,
                max_ac: stats.max_ac,
                accuracy: stats.accuracy,
                agility: stats.agility,
            };
            let bag = Bag {
                inventory,
                equipment,
                gold: *gold,
                weights,
                stats: &view,
                catalog,
                magics,
                toggles,
                cooldowns,
                belt,
                use_item_time: *use_item_time,
                dead,
            };
            windows.draw(&mut c, &bag, width, height, &mut out)
        };
        // Belt links and item uses decided inside the windows go through the
        // same local bookkeeping as the keyboard paths.
        let mut kept = Vec::with_capacity(out.len());
        for m in out {
            match m {
                ClientMessage::BeltLink { slot, info, item } => {
                    self.apply_belt_link(BeltLink { slot, info, item });
                    kept.push(ClientMessage::BeltLink { slot, info, item });
                }
                ClientMessage::ItemUse { slot } => {
                    let consumable = self
                        .inventory
                        .get(slot as usize)
                        .cloned()
                        .flatten()
                        .and_then(|i| self.catalog.get(i.info).cloned())
                        .filter(|d| d.item_type == mir_proto::item_type::CONSUMABLE);
                    if let Some(def) = consumable {
                        if now < self.use_item_time {
                            continue;
                        }
                        self.use_item_time = now + def.durability.max(250) as u64;
                    }
                    kept.push(ClientMessage::ItemUse { slot });
                }
                other => kept.push(other),
            }
        }
        let out = kept;
        self.windows_open_last_frame = self.windows.inventory_open
            || self.windows.character_open
            || self.windows.skills_open
            || self.windows.npc.is_some();
        self.pending_messages.extend(out);
        if over {
            // Swallow world input while the mouse is over a window.
            self.lmb = false;
            self.rmb = false;
            self.input.lmb_pressed = false;
            self.input.rmb_pressed = false;
            self.hovered = None;
        }
    }

    /// Messages produced by the windows, sent by the client shell.
    pub fn take_messages(&mut self) -> Vec<ClientMessage> {
        std::mem::take(&mut self.pending_messages)
    }

    fn draw_hud(
        &mut self,
        gpu: &Gpu,
        renderer: &mut SpriteRenderer,
        text: &mut TextLayer,
        width: i32,
        height: i32,
        now: u64,
        fps: f32,
    ) {
        let white = [1.0, 1.0, 1.0, 1.0];
        if let Some(panel) = Self::sprite(
            &mut self.assets,
            renderer,
            gpu,
            lib::GAME_INTER,
            50,
            Surface::Image,
        ) {
            let px = (width - panel.width as i32) / 2;
            let py = height - panel.height as i32;
            renderer.draw(panel, px as f32, py as f32, white, Blend::Alpha);
            let hp_pct = self.stats.hp.max(0) as f32 / self.stats.max_hp.max(1) as f32;
            let mp_pct = self.stats.mp.max(0) as f32 / self.stats.max_mp.max(1) as f32;
            if let Some(hp) = Self::sprite(
                &mut self.assets,
                renderer,
                gpu,
                lib::GAME_INTER,
                52,
                Surface::Image,
            ) {
                renderer.draw_cropped(hp, (px + 35) as f32, (py + 22) as f32, hp_pct, white);
            }
            if let Some(mp) = Self::sprite(
                &mut self.assets,
                renderer,
                gpu,
                lib::GAME_INTER,
                54,
                Surface::Image,
            ) {
                renderer.draw_cropped(mp, (px + 35) as f32, (py + 36) as f32, mp_pct, white);
            }
            if let Some(frame) = Self::sprite(
                &mut self.assets,
                renderer,
                gpu,
                lib::GAME_INTER,
                51,
                Surface::Image,
            ) {
                let fx = px + (panel.width as i32 - frame.width as i32) / 2 + 1;
                renderer.draw(frame, fx as f32, (py + 3) as f32, white, Blend::Alpha);
                if let Some(fill) = Self::sprite(
                    &mut self.assets,
                    renderer,
                    gpu,
                    lib::GAME_INTER,
                    56,
                    Surface::Image,
                ) {
                    let pct =
                        self.stats.experience as f32 / self.stats.max_experience.max(1) as f32;
                    let ix = fx + (frame.width as i32 - fill.width as i32) / 2;
                    renderer.draw_cropped(fill, ix as f32, (py + 2) as f32, pct, white);
                }
            }
            text.draw(
                &format!("{}/{}", self.stats.hp, self.stats.max_hp),
                11,
                (px + 40) as f32,
                (py + 22) as f32,
                [255, 255, 255, 255],
            );
            text.draw(
                &format!("{}/{}", self.stats.mp, self.stats.max_mp),
                11,
                (px + 40) as f32,
                (py + 36) as f32,
                [255, 255, 255, 255],
            );
            text.draw(
                &format!(
                    "Lv {}  EXP {}/{}",
                    self.stats.level, self.stats.experience, self.stats.max_experience
                ),
                11,
                (px + 40) as f32,
                (py + 50) as f32,
                [255, 230, 160, 255],
            );
        }
        let mut y = 8.0;
        for (line, t) in &self.chat {
            let age = now.saturating_sub(*t);
            if age > 15_000 {
                continue;
            }
            text.draw(line, 13, 12.0, y, [255, 255, 200, 255]);
            y += 17.0;
        }
        if self.debug {
            let (pages, sprites) = renderer.stats();
            let dbg = format!(
                "{} | {:.0} fps | {} objects | {} sprites / {} pages | LMB walk, RMB run, click monster/NPC/item, Tab pick up, W bag, Q character, Z belt, ` hide",
                self.status,
                fps,
                self.objects.len(),
                sprites,
                pages
            );
            let _ = y;
            text.draw(&dbg, 12, 12.0, height as f32 - 152.0, [200, 200, 200, 255]);
            if let Some(h) = self.hovered.and_then(|id| self.objects.get(&id)) {
                text.draw(
                    &format!("{} {}/{}", h.name(), h.hp, h.max_hp),
                    12,
                    self.mouse.0 + 14.0,
                    self.mouse.1 + 14.0,
                    [255, 255, 255, 255],
                );
            }
        }
    }
}
