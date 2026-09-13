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
                self.map_light = map.light;
                self.audio.stop_all();
                self.audio.play_music(music_index(map.music));
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
                    light: 0,
                    appearance: Appearance::Player {
                        name,
                        gender,
                        class,
                        armour: 0,
                        weapon: None,
                        hair,
                        helmet: 0,
                        shield: None,
                        name_color: 0,
                        guild: String::new(),
                        guild_rank: String::new(),
                        horse: 0,
                        horse_shape: 0,
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
                        self.sfx_appear(&state);
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
            ServerMessage::ObjectMining {
                id,
                direction,
                effect,
            } => {
                if let Some(o) = self.objects.get_mut(&id) {
                    let location = o.queue.back().map(|q| q.location).unwrap_or(o.location);
                    o.enqueue(Queued {
                        action: Action::Mining,
                        direction,
                        location,
                        distance: 0,
                    });
                }
                self.audio.play(if effect {
                    sound_table::idx::MINING_HIT
                } else {
                    sound_table::idx::MINING_STRUCK
                });
            }
            ServerMessage::ObjectFishing {
                id,
                state,
                direction,
                float,
                found,
            } => {
                use mir_proto::fishing_state as fs;
                let was_fishing = self
                    .objects
                    .get(&id)
                    .map(|o| {
                        matches!(o.action, Action::FishingCast | Action::FishingWait)
                            || o.queue.iter().any(|q| {
                                matches!(q.action, Action::FishingCast | Action::FishingWait)
                            })
                    })
                    .unwrap_or(false);
                let action = match state {
                    fs::CAST if was_fishing => Action::FishingWait,
                    fs::CAST => Action::FishingCast,
                    _ if was_fishing => Action::FishingReel,
                    _ => Action::Standing,
                };
                if let Some(o) = self.objects.get_mut(&id) {
                    let location = o.queue.back().map(|q| q.location).unwrap_or(o.location);
                    if action != Action::Standing {
                        o.enqueue(Queued {
                            action,
                            direction,
                            location,
                            distance: 0,
                        });
                    }
                }
                match action {
                    Action::FishingCast => self.audio.play(sound_table::idx::FISHING_CAST),
                    Action::FishingWait => {
                        if found {
                            self.audio.play(sound_table::idx::FISHING_BOB);
                        }
                        self.effects
                            .extend(effects::fishing_float(float, found, now));
                    }
                    Action::FishingReel => self.audio.play(sound_table::idx::FISHING_REEL),
                    _ => {}
                }
                if Some(id) == self.user {
                    if state == fs::CAST {
                        if let Some(f) = &mut self.fishing {
                            f.found = found;
                        }
                    } else {
                        self.fishing = None;
                    }
                }
            }
            ServerMessage::FishingStats {
                points, required, ..
            } => {
                if let Some(f) = &mut self.fishing {
                    f.points = points;
                    f.required = required;
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
                self.sfx_attack(id, attack_magic);
                if let Some(m) = attack_magic {
                    if let Some(e) = effects::attack_effect(m, id, direction.index(), now) {
                        self.effects.push(e);
                    }
                }
            }
            ServerMessage::ObjectRangeAttack {
                id,
                direction,
                target,
                location,
                magic,
            } => {
                let from = self.objects.get(&id).map(|o| o.location);
                self.sfx_attack(id, None);
                if let Some(o) = self.objects.get_mut(&id) {
                    let loc = o.queue.back().map(|q| q.location).unwrap_or(o.location);
                    o.enqueue(Queued {
                        action: Action::Attack,
                        direction,
                        location: loc,
                        distance: 0,
                    });
                }
                if let Some(from) = from {
                    let to = match target {
                        Some(t) => effects::Anchor::Object(t),
                        None => effects::Anchor::Cell(location),
                    };
                    self.projectiles
                        .push(effects::monster_projectile(magic, from, to, now));
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
                for &s in sound_table::magic_cast(magic) {
                    self.audio.play(s);
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
            ServerMessage::QuestList(list) => self.quests = list,
            ServerMessage::QuestChanged(q) => {
                let was_done = self
                    .quests
                    .iter()
                    .find(|x| x.quest == q.quest)
                    .map(|x| x.completed)
                    .unwrap_or(false);
                let name = self
                    .catalog
                    .quest(q.quest)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| format!("Quest {}", q.quest));
                let (text, sound) = if q.completed && !was_done {
                    (
                        format!("Quest completed: {name}"),
                        sound_table::idx::QUEST_COMPLETE,
                    )
                } else if !self.quests.iter().any(|x| x.quest == q.quest) {
                    (
                        format!("Quest accepted: {name}"),
                        sound_table::idx::QUEST_TAKE,
                    )
                } else {
                    (format!("Quest updated: {name}"), 0)
                };
                self.audio.play(sound);
                match self.quests.iter_mut().find(|x| x.quest == q.quest) {
                    Some(x) => *x = q,
                    None => self.quests.push(q),
                }
                self.say(text, now);
            }
            ServerMessage::QuestCancelled { quest } => self.quests.retain(|q| q.quest != quest),
            ServerMessage::DayChanged { day_time } => self.day_time = day_time,
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
                self.audio.play(object_effect_sound(effect));
                if let Some(e) = effects::object_effect(effect, id, location, now) {
                    self.effects.push(e);
                }
            }
            ServerMessage::MapEffect { location, effect } => {
                self.audio.play(map_effect_sound(effect));
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
                self.sfx_struck(id);
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
                self.sfx_die(id);
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
                    mir_proto::Grid::Storage => &mut self.storage,
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
            ServerMessage::GoldChanged { gold } => {
                if gold > self.gold {
                    self.audio.play(sound_table::idx::GOLD_GAINED);
                }
                self.gold = gold;
            }
            ServerMessage::WeightsChanged(w) => self.weights = w,
            ServerMessage::ObjectAppearance { id, appearance } => {
                if let Some(o) = self.objects.get_mut(&id) {
                    let was_mounted = o.mounted();
                    o.appearance = appearance;
                    if o.mounted() != was_mounted {
                        o.refresh_frame();
                    }
                }
            }
            ServerMessage::MarriageInvite { from } => {
                self.say_colored(
                    format!("{from} proposes to you (see the prompt)."),
                    now,
                    [255, 150, 200, 255],
                );
                self.marriage_invite = Some(from);
            }
            ServerMessage::MarriageInfo {
                partner,
                wedding_ring,
            } => {
                self.partner = partner;
                self.wedding_ring = wedding_ring;
            }
            ServerMessage::MapChanged {
                map,
                location,
                direction,
            } => {
                self.load_map(&map.file, &map.name);
                self.map_light = map.light;
                self.audio.play_music(music_index(map.music));
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
                quests,
            } => {
                self.windows.npc = Some(NpcDialog::new(
                    npc,
                    page,
                    &say,
                    dialog_type,
                    goods,
                    sell_types,
                    quests,
                ));
            }
            ServerMessage::NpcClose => {
                self.windows.npc = None;
                self.companion_shop.clear();
            }
            ServerMessage::Chat { text } => self.say(text, now),
            ServerMessage::Storage { size, items } => {
                self.storage = (0..size).map(|_| None).collect();
                for (slot, item) in items {
                    if let Some(c) = self.storage.get_mut(slot as usize) {
                        *c = Some(item);
                    }
                }
            }
            ServerMessage::TradeRequest { from } => {
                self.say_colored(
                    format!("{from} wants to trade with you."),
                    now,
                    [0, 255, 255, 255],
                );
                self.trade_request = Some(from);
            }
            ServerMessage::TradeOpen { name } => {
                self.say_colored(format!("Trading with {name}."), now, [0, 255, 255, 255]);
                self.trade = Some(crate::windows::TradeState {
                    partner: name,
                    my_items: Vec::new(),
                    my_gold: 0,
                    their_items: Vec::new(),
                    their_gold: 0,
                    confirmed: false,
                });
            }
            ServerMessage::TradeClose => {
                if self.trade.take().is_some() {
                    self.say_colored("Trade closed.".into(), now, [0, 255, 255, 255]);
                }
            }
            ServerMessage::TradeAddItem { grid, slot, count } => {
                if let Some(t) = &mut self.trade {
                    t.my_items.push((grid, slot, count));
                }
            }
            ServerMessage::TradeAddGold { gold } => {
                if let Some(t) = &mut self.trade {
                    t.my_gold = gold;
                }
            }
            ServerMessage::TradeItemAdded { item } => {
                if let Some(t) = &mut self.trade {
                    t.their_items.push(item);
                }
            }
            ServerMessage::TradeGoldAdded { gold } => {
                if let Some(t) = &mut self.trade {
                    t.their_gold = gold;
                }
            }
            ServerMessage::TradeUnlock => {
                if let Some(t) = &mut self.trade {
                    t.confirmed = false;
                }
            }
            ServerMessage::AttackMode { mode } => {
                self.attack_mode = mode;
                self.say_colored(
                    format!("Attack mode: {}", attack_mode_name(mode)),
                    now,
                    [255, 200, 120, 255],
                );
            }
            ServerMessage::RefineList(list) => {
                for r in list {
                    self.refines.retain(|x| x.index != r.index);
                    self.refines.push(r);
                }
            }
            ServerMessage::RefineRetrieved { index } => self.refines.retain(|r| r.index != index),
            ServerMessage::CompanionShop(offers) => self.companion_shop = offers,
            ServerMessage::Companions(list) => self.companions = list,
            ServerMessage::MailList(list) => {
                let unread = list.iter().filter(|m| !m.opened).count();
                if unread > 0 {
                    self.say_colored(
                        format!("You have {unread} unread mail (M)."),
                        now,
                        [255, 200, 120, 255],
                    );
                }
                self.mail = list;
            }
            ServerMessage::MailNew(m) => {
                self.say_colored(
                    format!("New mail from {}: {} (M)", m.sender, m.subject),
                    now,
                    [255, 200, 120, 255],
                );
                self.mail.push(m);
            }
            ServerMessage::MailDelete { index } => {
                self.mail.retain(|m| m.index != index);
                if self.windows.mail_selected == Some(index) {
                    self.windows.mail_selected = None;
                }
            }
            ServerMessage::MailItemDelete { index, slot } => {
                if let Some(m) = self.mail.iter_mut().find(|m| m.index == index) {
                    if slot == 255 {
                        m.gold = 0;
                    } else if (slot as usize) < m.items.len() {
                        m.items.remove(slot as usize);
                    }
                }
            }
            ServerMessage::GuildInfo(info) => {
                if self.guild.is_some() && info.is_none() {
                    self.windows.guild_open = false;
                }
                self.guild = info;
            }
            ServerMessage::GuildNoticeChanged { notice } => {
                if let Some(g) = &mut self.guild {
                    g.notice = notice;
                }
                self.say_colored(
                    "The guild notice changed.".into(),
                    now,
                    [255, 200, 255, 255],
                );
            }
            ServerMessage::GuildUpdate {
                member_limit,
                funds,
                tax,
            } => {
                if let Some(g) = &mut self.guild {
                    g.member_limit = member_limit;
                    g.funds = funds;
                    g.tax = tax;
                }
            }
            ServerMessage::GuildKick { index } => {
                if let Some(g) = &mut self.guild {
                    g.members.retain(|m| m.index != index);
                }
            }
            ServerMessage::GuildMemberOffline { index } => {
                if let Some(m) = self
                    .guild
                    .as_mut()
                    .and_then(|g| g.members.iter_mut().find(|m| m.index == index))
                {
                    m.online = false;
                }
            }
            ServerMessage::GuildInvite { from, guild } => {
                self.say_colored(
                    format!("{from} invites you to the guild {guild} (G to answer)."),
                    now,
                    [255, 200, 255, 255],
                );
                self.guild_invite = Some((from, guild));
            }
            ServerMessage::GroupSwitch { allow } => self.allow_group = allow,
            ServerMessage::GroupInvite { from } => {
                self.say_colored(
                    format!("{from} invites you to a group (P to answer)."),
                    now,
                    [0, 255, 255, 255],
                );
                self.group_invite = Some(from);
            }
            ServerMessage::GroupMember { id, name } => {
                if !self.group.iter().any(|(i, _)| *i == id) {
                    if Some(id) != self.user {
                        self.say_colored(
                            format!("{name} has joined the group."),
                            now,
                            [0, 255, 255, 255],
                        );
                    }
                    self.group.push((id, name));
                }
            }
            ServerMessage::GroupRemove { id } => {
                if Some(id) == self.user {
                    self.group.clear();
                    self.say_colored("You have left the group.".into(), now, [0, 255, 255, 255]);
                } else if let Some(pos) = self.group.iter().position(|(i, _)| *i == id) {
                    let (_, name) = self.group.remove(pos);
                    self.say_colored(
                        format!("{name} has left the group."),
                        now,
                        [0, 255, 255, 255],
                    );
                }
            }
            ServerMessage::Say { id, kind, text } => {
                use mir_proto::ChatKind;
                // Zircon chat colours (ChatPanel): white talk, yellow shout,
                // green whispers, cyan group, orange global, red system.
                let color = match kind {
                    ChatKind::Normal => [255, 255, 255, 255],
                    ChatKind::Shout => [255, 255, 0, 255],
                    ChatKind::WhisperIn | ChatKind::WhisperOut => [0, 255, 0, 255],
                    ChatKind::Group => [0, 255, 255, 255],
                    ChatKind::Global => [255, 165, 0, 255],
                    ChatKind::Guild => [255, 200, 255, 255],
                    ChatKind::System => [255, 80, 80, 255],
                };
                if let Some(o) = id.and_then(|i| self.objects.get_mut(&i)) {
                    let spoken = text.split_once(": ").map(|(_, t)| t).unwrap_or(&text);
                    o.bubble = Some((spoken.to_string(), now));
                }
                self.say_colored(text, now, color);
            }
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
        self.audio.stop_all();
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
        self.group.clear();
        self.group_invite = None;
        self.auto_group_done = false;
        self.trade = None;
        self.trade_request = None;
        self.guild = None;
        self.guild_invite = None;
        self.mail.clear();
        self.partner = None;
        self.wedding_ring = None;
        self.marriage_invite = None;
        self.fishing = None;
        self.refines.clear();
        self.companions.clear();
        self.companion_shop.clear();
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
