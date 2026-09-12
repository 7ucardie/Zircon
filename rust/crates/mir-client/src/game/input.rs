use super::*;

impl Game {
    pub(super) fn hit_test(&mut self, width: i32, height: i32) -> Option<ObjectId> {
        let view = View::new(width, height, self.user());
        let (mx, my) = (self.mouse.0 as i32, self.mouse.1 as i32);
        let mut best: Option<(i32, ObjectId)> = None;
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in ids {
            let o = &self.objects[&id];
            if Some(id) == self.user || o.dead || o.is_spell() {
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

    pub(super) fn blocked(&self, p: Point) -> bool {
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

    pub(super) fn handle_input(
        &mut self,
        now: u64,
        width: i32,
        height: i32,
        conn: Option<&Connection>,
    ) {
        // Keyboard shortcuts (Zircon defaults: Tab pick up, W bag, Q character).
        for ch in self.input.text.chars() {
            match ch.to_ascii_lowercase() {
                'w' | 'i' => self.windows.inventory_open = !self.windows.inventory_open,
                'q' | 'c' => self.windows.character_open = !self.windows.character_open,
                'e' | 's' => self.windows.skills_open = !self.windows.skills_open,
                'z' => self.windows.belt_open = !self.windows.belt_open,
                'l' => self.windows.quests_open = !self.windows.quests_open,
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
                if self.attackable(target) && user_loc.distance(target.location) <= 1 {
                    if now >= self.action_time && now >= self.attack_time {
                        let direction = Direction::from_points(user_loc, target.location);
                        self.action_time = now + ATTACK_TIME;
                        // Zircon: the swing delay shortens with attack speed.
                        self.attack_time = now + attack_delay(self.stats.attack_speed as i64);
                        // Zircon `UserObject` priority: lotus arm, Slaying,
                        // stances, Destructive Surge, then charged power attacks.
                        let mut attack_magic = self.armed_lotus.or(self.auto_charged);
                        if self.slaying_ready {
                            attack_magic = Some(magic_type::SLAYING);
                        }
                        if self.toggles.contains(&magic_type::THRUSTING) {
                            attack_magic = Some(magic_type::THRUSTING);
                        }
                        if self.toggles.contains(&magic_type::HALF_MOON) {
                            attack_magic = Some(magic_type::HALF_MOON);
                        }
                        if self.toggles.contains(&magic_type::DESTRUCTIVE_SURGE) {
                            attack_magic = Some(magic_type::DESTRUCTIVE_SURGE);
                        }
                        if self.toggles.contains(&magic_type::FLAME_SPLASH)
                            && attack_magic.is_none()
                        {
                            attack_magic = Some(magic_type::FLAME_SPLASH);
                        }
                        if let Some(c) = self.charged {
                            attack_magic = Some(c);
                        }
                        let action = attack_action(attack_magic);
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
    pub(super) fn belt_key(&mut self, slot: u8, now: u64, conn: Option<&Connection>) {
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

    pub(super) fn apply_belt_link(&mut self, link: BeltLink) {
        if let Some(l) = self.belt.get_mut(link.slot as usize) {
            *l = link;
        }
    }

    /// Zircon `DXItemCell.UseItem` for consumables: the client-side lock is
    /// `max(250, Durability)` ms; the server enforces its own.
    pub(super) fn try_use_item(&mut self, slot: u8, now: u64, conn: Option<&Connection>) {
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
            self.use_item_time = now + use_item_lock(def.durability);
        }
        if let Some(c) = conn {
            c.send(ClientMessage::ItemUse { slot });
        }
    }

    pub(super) fn function_key(
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
        let Some(magic) = self.magics.iter().find(|m| m.key == f).map(|m| m.magic) else {
            return;
        };
        self.use_skill(magic, now, width, height, conn);
    }

    /// Use a learned skill: toggle, charge, arm or cast it.
    pub(super) fn use_skill(
        &mut self,
        magic: u16,
        now: u64,
        width: i32,
        height: i32,
        conn: Option<&Connection>,
    ) {
        let Some(m) = self.magics.iter().find(|m| m.magic == magic).cloned() else {
            return;
        };
        let Some(def) = self.catalog.magic(magic).cloned() else {
            return;
        };
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
        if magic_type::is_charge(magic) {
            if def.cost(m.level) > self.stats.mp {
                self.say("Not enough mana.".into(), now);
                return;
            }
            if let Some(c) = conn {
                c.send(ClientMessage::MagicToggle { magic, on: true });
            }
            return;
        }
        if magic_type::is_armed(magic) {
            if magic == magic_type::KARMA && !self.buffs.iter().any(|(k, _)| *k == buff_type::CLOAK)
            {
                self.say("Karma needs the cloak.".into(), now);
                return;
            }
            if self.armed_lotus != Some(magic) {
                self.armed_lotus = Some(magic);
                self.say(format!("{} is ready.", def.name), now);
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
        if magic == magic_type::MAGIC_SHIELD
            && self
                .buffs
                .iter()
                .any(|(k, _)| *k == buff_type::MAGIC_SHIELD)
        {
            self.say("You are already shielded.".into(), now);
            return;
        }
        let self_cast = magic_type::is_self_cast(magic);
        let target = match magic {
            magic_type::HEAL => hovered
                .filter(|o| o.is_player())
                .map(|o| o.id)
                .or(self.user),
            m if magic_type::needs_target(m) => {
                hovered.filter(|o| self.attackable(o)).map(|o| o.id)
            }
            _ => None,
        };
        // Ground casts and lines use the mouse cell; self casts the own cell.
        let target_loc = if self_cast {
            user_loc
        } else {
            target
                .and_then(|t| self.objects.get(&t))
                .map(|o| o.location)
                .unwrap_or(mouse_cell)
        };
        if target_loc.distance(user_loc) > mir_proto::MAGIC_RANGE {
            self.say("Too far away.".into(), now);
            return;
        }
        let direction = if magic_type::is_stance_cast(magic) {
            Direction::Down
        } else if magic == magic_type::SHOULDER_DASH
            || magic == magic_type::COMBAT_KICK
            || magic == magic_type::RAKE
            || magic_type::is_directional(magic)
            || magic_type::is_line(magic)
        {
            if mouse_cell == user_loc {
                user.direction
            } else {
                Direction::from_points(user_loc, mouse_cell)
            }
        } else if target_loc == user_loc {
            user.direction
        } else {
            Direction::from_points(user_loc, target_loc)
        };
        self.magic_time = now + mir_proto::MAGIC_DELAY;
        self.action_time = now + 600;
        if magic == magic_type::SHOULDER_DASH {
            // No cast animation: the server streams dash steps.
            if let Some(c) = conn {
                c.send(ClientMessage::Magic {
                    magic,
                    direction,
                    target: None,
                    location: user_loc,
                });
            }
            return;
        }
        let action = cast_action(magic);
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

    /// One walk/run step toward `target`, turning if blocked.
    pub(super) fn step_toward(
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
}
