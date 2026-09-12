use super::*;

impl World {
    pub fn npc_call(&mut self, id: ObjectId, npc: ObjectId) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let (map, loc) = (o.map, o.location);
        let entry = match self.objects.get(&npc) {
            Some(n) if n.map == map && n.location.distance(loc) <= MAX_VIEW_RANGE => {
                match &n.kind {
                    Kind::Npc(d) => d.entry_page,
                    _ => return,
                }
            }
            _ => return,
        };
        if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
            p.npc = None;
        }
        self.npc_run_page(id, npc, entry);
    }

    pub fn npc_button(&mut self, id: ObjectId, button: i32) {
        let Some((npc, page)) = self
            .objects
            .get(&id)
            .and_then(|o| o.player())
            .and_then(|p| p.npc)
        else {
            return;
        };
        let Some(def) = self.data.npc_pages.get(&page) else {
            return;
        };
        let Some((_, dest)) = def.buttons.iter().find(|(b, d)| *b == button && *d != 0) else {
            return;
        };
        let dest = *dest;
        self.npc_run_page(id, npc, dest);
    }

    pub fn npc_close(&mut self, id: ObjectId) {
        if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
            p.npc = None;
        }
    }

    /// Zircon `NPCObject.NPCCall`: walk pages through checks and actions until
    /// one with text is reached.
    pub(super) fn npc_run_page(&mut self, id: ObjectId, npc: ObjectId, mut page: i32) {
        for _ in 0..20 {
            if page == 0 {
                self.npc_close(id);
                self.send_to(id, ServerMessage::NpcClose);
                return;
            }
            let Some(def) = self.data.npc_pages.get(&page).cloned() else {
                self.npc_close(id);
                self.send_to(id, ServerMessage::NpcClose);
                return;
            };
            let mut failed = None;
            for c in &def.checks {
                if !self.npc_check(id, c) {
                    failed = Some(c.fail_page);
                    break;
                }
            }
            if let Some(fail) = failed {
                page = fail;
                continue;
            }
            for a in &def.actions {
                self.npc_action(id, a);
            }
            if def.say.trim().is_empty() {
                if def.success_page != 0 {
                    page = def.success_page;
                    continue;
                }
                self.npc_close(id);
                self.send_to(id, ServerMessage::NpcClose);
                return;
            }
            let goods = def
                .goods
                .iter()
                .filter_map(|(item, rate)| {
                    let d = self.data.items.get(item)?;
                    Some(Good {
                        info: *item,
                        price: ((d.price as f64 * rate).round() as u64).max(1),
                    })
                })
                .collect();
            if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
                p.npc = Some((npc, page));
            }
            // Strip markup the client does not render.
            let say = def.say.clone();
            let _ = parse_dialog(&say);
            self.send_to(
                id,
                ServerMessage::NpcResponse {
                    npc,
                    page,
                    say,
                    dialog_type: def.dialog_type,
                    goods,
                    sell_types: def.types.clone(),
                },
            );
            return;
        }
    }

    pub(super) fn npc_check(&mut self, id: ObjectId, c: &crate::data::NpcCheckDef) -> bool {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return false;
        };
        let cmp = |op: i32, a: i64, b: i64| match op {
            0 => a == b,
            1 => a != b,
            2 => a < b,
            3 => a <= b,
            4 => a > b,
            5 => a >= b,
            _ => true,
        };
        // Zircon `Config.RedPoint`.
        const RED_POINT: i64 = 200;
        let weapon = p.bag.equipment.get(slot::WEAPON).and_then(|w| w.as_ref());
        let equal = c.operator == 0;
        match c.check_type {
            0 => cmp(c.operator, p.level as i64, c.int1 as i64),
            1 => cmp(c.operator, p.class.mir_class() as i64, c.int1 as i64),
            // Gender has no server case in Zircon: always passes.
            2 => true,
            3 => cmp(c.operator, p.bag.gold as i64, c.int1 as i64),
            4 => c.item1 == 0 || cmp(c.operator, p.bag.count_of(c.item1) as i64, c.int1 as i64),
            // PK points: nobody is red in the prototype.
            5 => {
                let threshold = if c.int1 == 0 {
                    RED_POINT
                } else {
                    c.int1 as i64
                };
                cmp(c.operator, 0, threshold)
            }
            6 => weapon.is_some() == equal,
            // Weapon level / element / added stats: no refining yet, so the
            // weapon counts as level 0 with no element (Zircon would throw
            // without a weapon; treat that as failing).
            7 => weapon.is_some() && cmp(c.operator, 0, c.int1 as i64),
            8 => weapon.is_some() && cmp(c.operator, 0, c.int2 as i64),
            9 => weapon.is_some() && !equal,
            16 => weapon.is_some() && cmp(c.operator, 0, c.int1 as i64),
            // Horse: none owned.
            10 => cmp(c.operator, 0, c.int1 as i64),
            // Marriage and wedding ring: not married.
            11 | 12 => !equal,
            13 => {
                c.item1 == 0
                    || p.bag
                        .can_gain(&self.data, c.item1, c.int1.max(1) as u32, p.max_bag)
            }
            // Weapon reset cooldown: never on cooldown.
            14 => weapon.is_some() && equal,
            15 => {
                let roll = self.rng.random_range(0..c.int1.max(1)) as i64;
                cmp(c.operator, roll, c.int2 as i64)
            }
            // Currency by name: only gold exists.
            17 => {
                if c.string1.eq_ignore_ascii_case("gold") {
                    cmp(c.operator, p.bag.gold as i64, c.int1 as i64)
                } else {
                    true
                }
            }
            // Roll results, data lists and fame do not exist yet: fail like
            // Zircon does when the data is missing.
            18 | 19 | 21 => false,
            20 => cmp(c.operator, 0, c.int2 as i64),
            _ => true,
        }
    }

    pub(super) fn npc_action(&mut self, id: ObjectId, a: &crate::data::NpcActionDef) {
        match a.action_type {
            0 => {
                // Teleport to MapParameter1 at (int1, int2) or a random cell.
                let Some(map) = self.data.maps.get(&a.map1).map(|m| m.index) else {
                    return;
                };
                if self.ensure_map(map).is_err() {
                    return;
                }
                let target = if a.int1 == 0 && a.int2 == 0 {
                    self.random_walkable(map)
                } else {
                    Some(Point::new(a.int1, a.int2))
                };
                if let Some(t) = target {
                    self.change_map(id, map, t);
                }
            }
            1 | 2 => {
                if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
                    if a.action_type == 1 {
                        p.bag.gold += a.int1.max(0) as u64;
                    } else {
                        p.bag.gold = p.bag.gold.saturating_sub(a.int1.max(0) as u64);
                    }
                }
                self.send_gold(id);
            }
            3 => {
                let count = a.int1.max(1) as u32;
                if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
                    let mut next = p.next_item_id;
                    let changes = p.bag.gain(&self.data, a.item1, count, &mut next);
                    p.next_item_id = next;
                    self.send_changes(id, changes);
                }
            }
            4 => {
                let count = a.int1.max(1) as u32;
                if let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) {
                    let changes = p.bag.take_info(a.item1, count);
                    self.send_changes(id, changes);
                }
            }
            _ => {}
        }
    }

    pub fn npc_buy(&mut self, id: ObjectId, info: i32, count: u32) {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let Some((_, page)) = p.npc else {
            return;
        };
        let Some(def) = self.data.npc_pages.get(&page) else {
            return;
        };
        let Some((_, rate)) = def.goods.iter().find(|(i, _)| *i == info) else {
            return;
        };
        let Some(item) = self.data.items.get(&info) else {
            return;
        };
        let count = count.clamp(1, item.stack_size.max(1) as u32);
        let price = ((item.price as f64 * rate).round() as u64).max(1);
        let total = price * count as u64;
        if p.bag.gold < total {
            self.send_to(
                id,
                ServerMessage::Chat {
                    text: "Not enough gold.".into(),
                },
            );
            return;
        }
        if !p.bag.can_gain(&self.data, info, count, p.max_bag) {
            self.send_to(
                id,
                ServerMessage::Chat {
                    text: "You cannot carry that.".into(),
                },
            );
            return;
        }
        let name = item.name.clone();
        let o = self.objects.get_mut(&id).unwrap();
        let p = o.player_mut().unwrap();
        p.bag.gold -= total;
        let mut next = p.next_item_id;
        let changes = p.bag.gain(&self.data, info, count, &mut next);
        p.next_item_id = next;
        self.send_gold(id);
        self.send_changes(id, changes);
        self.send_to(
            id,
            ServerMessage::Chat {
                text: format!("Bought {name} x{count} for {total} gold."),
            },
        );
    }

    pub fn npc_sell(&mut self, id: ObjectId, slots: Vec<u8>) {
        let Some(p) = self.objects.get(&id).and_then(|o| o.player()) else {
            return;
        };
        let Some((_, page)) = p.npc else {
            return;
        };
        let Some(def) = self.data.npc_pages.get(&page).cloned() else {
            return;
        };
        if def.dialog_type != 1 || def.types.is_empty() {
            return;
        }
        let mut earned = 0u64;
        let mut sold = 0u32;
        let mut changes = Changed::new();
        for slot in slots {
            let Some(item) = self.objects[&id]
                .player()
                .unwrap()
                .bag
                .inventory
                .get(slot as usize)
                .cloned()
                .flatten()
            else {
                continue;
            };
            let Some(idef) = self.data.items.get(&item.info) else {
                continue;
            };
            if !idef.can_sell || !def.types.contains(&idef.item_type) {
                continue;
            }
            let price = sell_price(idef, &item);
            let o = self.objects.get_mut(&id).unwrap();
            let p = o.player_mut().unwrap();
            if let Some(c) = p.bag.take(Grid::Inventory, slot, item.count) {
                changes.push(c);
            }
            p.bag.gold += price;
            earned += price;
            sold += item.count;
        }
        self.send_gold(id);
        self.send_changes(id, changes);
        if sold > 0 {
            self.send_to(
                id,
                ServerMessage::Chat {
                    text: format!("Sold {sold} item(s) for {earned} gold."),
                },
            );
        }
    }
}
