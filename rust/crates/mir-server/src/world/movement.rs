use super::*;

impl World {
    /// Is `p` blocked for movement? `grace` applies Zircon's 300 ms vacated-cell
    /// grace that only players get.
    pub(super) fn cell_blocked(&self, map: i32, p: Point, grace: bool) -> bool {
        let Some(m) = self.maps.get(&map) else {
            return true;
        };
        if !m.file.is_walkable(p.x, p.y) {
            return true;
        }
        m.objects_at(p).iter().any(|id| {
            let o = &self.objects[id];
            o.blocking() && !(grace && o.cell_time > self.now)
        })
    }

    pub(super) fn move_object(&mut self, id: ObjectId, to: Point) {
        let obj = self.objects.get_mut(&id).unwrap();
        let from = obj.location;
        obj.location = to;
        obj.cell_time = self.now + CELL_GRACE;
        let map = obj.map;
        let m = self.maps.get_mut(&map).unwrap();
        m.remove_from_cell(id, from);
        m.add_to_cell(id, to);
    }

    // ---- player commands -------------------------------------------------

    pub fn player_turn(&mut self, id: ObjectId, direction: Direction) {
        let Some(o) = self.objects.get_mut(&id) else {
            return;
        };
        if o.dead || self.now < o.action_time {
            let (loc, dir) = (o.location, o.direction);
            self.send_to(
                id,
                ServerMessage::MoveDenied {
                    location: loc,
                    direction: dir,
                },
            );
            return;
        }
        o.direction = direction;
        o.action_time = self.now + TURN_TIME;
        self.events
            .push((id, ServerMessage::ObjectTurn { id, direction }));
    }

    pub fn player_move(&mut self, id: ObjectId, direction: Direction, run: bool) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let (map, from) = (o.map, o.location);
        let distance = if run { 2 } else { 1 };
        let ok = !o.dead && self.now >= o.action_time && self.now >= o.move_time && {
            (1..=distance).all(|i| !self.cell_blocked(map, from.step(direction, i), true))
        };
        if !ok {
            let dir = o.direction;
            self.send_to(
                id,
                ServerMessage::MoveDenied {
                    location: from,
                    direction: dir,
                },
            );
            return;
        }
        let to = from.step(direction, distance);
        {
            let o = self.objects.get_mut(&id).unwrap();
            o.direction = direction;
            o.action_time = self.now + MOVE_TIME;
            o.move_time = self.now + MOVE_TIME;
        }
        self.move_object(id, to);
        if self.try_travel(id) {
            return;
        }
        self.events.push((
            id,
            ServerMessage::ObjectMove {
                id,
                from,
                to,
                direction,
                run,
            },
        ));
    }

    /// Zircon `ShoulderDash.MagicComplete`: one cell every 300 ms, pushing
    /// weaker monsters out of the way; stops at walls or unpushable objects.
    pub(super) fn process_dashes(&mut self) {
        let now = self.now;
        let dashing: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| {
                o.player()
                    .and_then(|p| p.dash)
                    .map(|d| now >= d.next_step)
                    .unwrap_or(false)
            })
            .map(|o| o.id)
            .collect();
        for id in dashing {
            let (dash, map, loc, level, dead) = {
                let o = &self.objects[&id];
                let p = o.player().unwrap();
                (p.dash.unwrap(), o.map, o.location, p.level, o.dead)
            };
            let stop = |w: &mut World, id: ObjectId, travelled: i32| {
                if let Some(p) = w.objects.get_mut(&id).and_then(|o| o.player_mut()) {
                    p.dash = None;
                }
                if travelled == 0 {
                    w.send_to(
                        id,
                        ServerMessage::Chat {
                            text: "Dash failed.".into(),
                        },
                    );
                }
            };
            if dead || dash.remaining <= 0 {
                stop(self, id, dash.travelled);
                continue;
            }
            let next = loc.step(dash.direction, 1);
            let walkable = self
                .maps
                .get(&map)
                .map(|m| m.file.is_walkable(next.x, next.y))
                .unwrap_or(false);
            if !walkable {
                stop(self, id, dash.travelled);
                continue;
            }
            // Push what stands there (Zircon `CanPushTarget`).
            let occupants: Vec<ObjectId> = self.maps[&map].objects_at(next).to_vec();
            let mut blocked = false;
            let mlevel_of = |w: &World, v: ObjectId| match &w.objects[&v].kind {
                Kind::Monster(m) => Some((
                    w.data.monsters[&m.def].level,
                    w.data.monsters[&m.def].is_boss,
                )),
                _ => None,
            };
            for v in occupants {
                let o = &self.objects[&v];
                if !o.blocking() || v == id {
                    continue;
                }
                let Some((mlevel, boss)) = mlevel_of(self, v) else {
                    blocked = true;
                    break;
                };
                let lvl = self.objects[&id]
                    .player()
                    .and_then(|p| {
                        p.magics
                            .iter()
                            .find(|m| m.magic == magic_type::SHOULDER_DASH)
                    })
                    .map(|m| m.level as i32)
                    .unwrap_or(0);
                if boss || mlevel >= level {
                    blocked = true;
                    break;
                }
                if self.rng.random_range(0..16) >= 6 + lvl * 3 + level - mlevel {
                    blocked = true;
                    break;
                }
                let beyond = next.step(dash.direction, 1);
                if self.cell_blocked(map, beyond, false) {
                    blocked = true;
                    break;
                }
                self.move_object(v, beyond);
                if let Some(o) = self.objects.get_mut(&v) {
                    o.direction = dash.direction.rotate(4);
                }
                self.events.push((
                    v,
                    ServerMessage::ObjectMove {
                        id: v,
                        from: next,
                        to: beyond,
                        direction: dash.direction,
                        run: false,
                    },
                ));
                self.level_magic(id, magic_type::SHOULDER_DASH);
            }
            if blocked {
                stop(self, id, dash.travelled);
                continue;
            }
            self.move_object(id, next);
            {
                let o = self.objects.get_mut(&id).unwrap();
                o.direction = dash.direction;
                o.action_time = now + 300;
                let p = o.player_mut().unwrap();
                p.dash = Some(Dash {
                    remaining: dash.remaining - 1,
                    travelled: dash.travelled + 1,
                    direction: dash.direction,
                    next_step: now + 300,
                });
            }
            self.events.push((
                id,
                ServerMessage::ObjectDash {
                    id,
                    direction: dash.direction,
                    location: next,
                    distance: 1,
                    magic: magic_type::SHOULDER_DASH,
                },
            ));
        }
    }
}
