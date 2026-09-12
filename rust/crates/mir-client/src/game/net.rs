use super::*;

impl Game {
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
            ServerMessage::ObjectTeleport {
                id,
                location,
                direction,
            } => {
                if Some(id) == self.user {
                    self.goal = None;
                    self.move_time = 0;
                }
                if let Some(o) = self.objects.get_mut(&id) {
                    o.snap(location, direction, now);
                }
            }
            ServerMessage::ObjectAttack {
                id,
                direction,
                attack_magic,
            } => {
                if let Some(o) = self.objects.get_mut(&id) {
                    let location = o.queue.back().map(|q| q.location).unwrap_or(o.location);
                    o.enqueue(Queued {
                        action: attack_action(attack_magic),
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
                            action: cast_action(magic),
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
                } else if magic_type::is_charge(magic) {
                    if on {
                        self.charged = Some(magic);
                    } else if self.charged == Some(magic) {
                        self.charged = None;
                    }
                } else if magic_type::is_armed(magic) {
                    if !on && self.armed_lotus == Some(magic) {
                        self.armed_lotus = None;
                    }
                } else if magic_type::is_auto_charge(magic) {
                    if on {
                        self.auto_charged = Some(magic);
                    } else if self.auto_charged == Some(magic) {
                        self.auto_charged = None;
                    }
                } else if on {
                    self.toggles.insert(magic);
                } else {
                    self.toggles.remove(&magic);
                }
            }
            ServerMessage::BuffAdd(b) => {
                self.buffs.retain(|(k, _)| *k != b.kind);
                let until = if b.remaining_ms == u64::MAX {
                    u64::MAX
                } else {
                    now + b.remaining_ms
                };
                self.buffs.push((b.kind, until));
            }
            ServerMessage::BuffRemove { kind } => self.buffs.retain(|(k, _)| *k != kind),
            ServerMessage::BuffTime { kind, remaining_ms } => {
                if let Some(b) = self.buffs.iter_mut().find(|(k, _)| *k == kind) {
                    b.1 = now + remaining_ms;
                }
            }
            ServerMessage::ObjectBuff { id, kind, on } => {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.visible_buffs.retain(|k| *k != kind);
                    if on {
                        o.visible_buffs.push(kind);
                    }
                }
            }
            ServerMessage::ObjectDash {
                id,
                direction,
                location,
                distance,
                ..
            } => {
                let from = location.step(direction.rotate(4), distance as i32);
                if Some(id) == self.user {
                    self.goal = None;
                    self.action_time = now + 300;
                    self.move_time = now + 300;
                }
                if let Some(o) = self.objects.get_mut(&id) {
                    let pending_end = o.queue.back().map(|q| q.location);
                    if pending_end.is_none() && o.moving_offset == (0, 0) && o.location != from {
                        o.location = from;
                    }
                    o.enqueue(Queued {
                        action: Action::Dash,
                        direction,
                        location,
                        distance: distance.max(1) as i32,
                    });
                }
            }
            ServerMessage::ObjectEffect {
                id,
                effect,
                location,
            } => {
                if let Some(e) = effects::object_effect(effect, id, location, now) {
                    self.effects.push(e);
                }
            }
            ServerMessage::MapEffect { location, effect } => {
                self.effects
                    .extend(effects::map_effect(effect, location, now));
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

    pub(super) fn load_map(&mut self, file: &str, name: &str) {
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
}
