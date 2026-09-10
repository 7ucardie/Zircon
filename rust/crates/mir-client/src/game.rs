//! Game state, input and scene rendering.
//!
//! Screen geometry follows `MapControl` in Zircon: 48x32 cells, the player's
//! cell centred horizontally and 34 px above centre, back tiles drawn 2x2,
//! middle/front tiles bottom-anchored, objects y-sorted per row.

use std::collections::HashMap;

use mir_formats::zl::SurfaceKind;
use mir_formats::MapFile;
use mir_proto::{
    Action, Appearance, CharacterSummary, ClientMessage, Direction, Gender, ObjectId, ObjectState,
    PlayerStats, Point, ServerMessage,
};

use crate::anim::{ClientObject, Queued};
use crate::assets::{kr_library, lib, Assets};
use crate::gfx::{Blend, Gpu, SpriteKey, SpriteRegion, SpriteRenderer, Surface};
use crate::monster_table::monster_sprite;
use crate::net::Connection;
use crate::text::TextLayer;

pub const CELL_W: i32 = 48;
pub const CELL_H: i32 = 32;
const MANUAL_HEIGHT_OFFSET: i32 = 34;
const MOVE_TIME: u64 = 600;
const TURN_TIME: u64 = 300;
const ATTACK_TIME: u64 = 600;
const ATTACK_DELAY: u64 = 1500;

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
    pub fn new(assets: Assets) -> Game {
        Game {
            assets,
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
                let path = self.assets.root().join(format!("Map/{}.map", map.file));
                match MapFile::load(&path) {
                    Ok(m) => {
                        tracing::info!(map = map.name, w = m.width, h = m.height, "map loaded");
                        self.map = Some(m);
                    }
                    Err(e) => {
                        self.status = format!("cannot load {}: {e}", path.display());
                        tracing::error!("{}", self.status);
                    }
                }
                self.map_name = map.name.clone();
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
                        weapon: 0,
                        hair,
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
            ServerMessage::ObjectAttack { id, direction } => {
                if let Some(o) = self.objects.get_mut(&id) {
                    let location = o.queue.back().map(|q| q.location).unwrap_or(o.location);
                    o.enqueue(Queued {
                        action: Action::Attack,
                        direction,
                        location,
                        distance: 0,
                    });
                }
            }
            ServerMessage::ObjectStruck { id, damage, .. } => {
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
    }

    pub fn update(&mut self, now: u64, width: i32, height: i32, conn: Option<&Connection>) {
        if now >= self.animation_time + 100 {
            self.animation_time = now;
            self.animation = self.animation.wrapping_add(1);
        }
        for o in self.objects.values_mut() {
            o.process(now);
        }
        self.hovered = self.hit_test(width, height);
        self.handle_input(now, width, height, conn);
        if let Some(u) = self.user() {
            self.status = format!("{} ({}, {})", self.map_name, u.location.x, u.location.y);
        }
    }

    fn body_sprite(&self, o: &ClientObject) -> Option<(u16, u32)> {
        match &o.appearance {
            Appearance::Player { gender, armour, .. } => {
                let library = match gender {
                    Gender::Male => lib::M_HUM,
                    Gender::Female => lib::WM_HUM,
                };
                Some((library, o.sprite_index(*armour)))
            }
            Appearance::Monster { image, .. } => {
                let (library, shape) = monster_sprite(*image)?;
                Some((library, o.sprite_index(shape)))
            }
        }
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
            let x0 = dx + info.offset_x as i32;
            let y0 = dy + info.offset_y as i32;
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
            .any(|o| !o.dead && Some(o.id) != self.user && o.location == p)
    }

    fn handle_input(&mut self, now: u64, width: i32, height: i32, conn: Option<&Connection>) {
        if !(self.lmb || self.rmb) {
            return;
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

        // Attack a hovered monster in melee range.
        if self.lmb {
            if let Some(target) = self.hovered.and_then(|id| self.objects.get(&id)) {
                if !target.is_player() && !target.dead && user_loc.distance(target.location) <= 1 {
                    if now >= self.action_time && now >= self.attack_time {
                        let direction = Direction::from_points(user_loc, target.location);
                        self.action_time = now + ATTACK_TIME;
                        self.attack_time = now + ATTACK_DELAY;
                        if let Some(u) = self.user_mut() {
                            u.queue.clear();
                            u.enqueue(Queued {
                                action: Action::Attack,
                                direction,
                                location: user_loc,
                                distance: 0,
                            });
                        }
                        if let Some(c) = conn {
                            c.send(ClientMessage::Attack { direction });
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
        let wanted = Direction::from_points(user_loc, target);
        let run = self.rmb && user_loc.distance(target) >= 2;
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

            // Overlay: names, health bars, damage numbers.
            let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
            for id in ids {
                let o = &self.objects[&id];
                let (dx, dy) = view.object_px(o);
                if dx < -100 || dx > width + 100 || dy < -150 || dy > height + 100 {
                    continue;
                }
                let name_color = if o.is_player() {
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
    }

    fn draw_object(&mut self, id: ObjectId, view: &View, gpu: &Gpu, renderer: &mut SpriteRenderer) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let Some((library, index)) = self.body_sprite(o) else {
            return;
        };
        let (dx, dy) = view.object_px(o);
        let hair = match &o.appearance {
            Appearance::Player { hair, gender, .. } if *hair > 0 => {
                let hair_lib = match gender {
                    Gender::Male => lib::M_HAIR,
                    Gender::Female => lib::WM_HAIR,
                };
                Some((hair_lib, index + (*hair as u32 - 1) * 5000))
            }
            _ => None,
        };
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
            let tint = if self.hovered == Some(id) {
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
        if let Some((hair_lib, hair_index)) = hair {
            if let Some(hi) = self.assets.info(hair_lib, hair_index) {
                if let Some(r) = Self::sprite(
                    &mut self.assets,
                    renderer,
                    gpu,
                    hair_lib,
                    hair_index,
                    Surface::Image,
                ) {
                    renderer.draw(
                        r,
                        (dx + hi.offset_x as i32) as f32,
                        (dy + hi.offset_y as i32) as f32,
                        white,
                        Blend::Alpha,
                    );
                }
            }
        }
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
                "{} | {:.0} fps | {} objects | {} sprites / {} pages | LMB walk, RMB run, click monster to attack, F1 hide",
                self.status,
                fps,
                self.objects.len(),
                sprites,
                pages
            );
            text.draw(&dbg, 12, 12.0, y + 4.0, [200, 200, 200, 255]);
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
