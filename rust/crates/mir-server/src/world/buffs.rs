use super::*;

impl World {
    // ---- buffs, charges, dashes, spell objects -----------------------------

    /// Zircon `BuffAdd`: replaces a buff of the same kind.
    pub(super) fn buff_add(&mut self, id: ObjectId, kind: u16, duration_ms: u64, stats: BuffStats) {
        let now = self.now;
        let Some(o) = self.objects.get_mut(&id) else {
            return;
        };
        let Some(p) = o.player_mut() else {
            return;
        };
        p.buffs.retain(|b| b.kind != kind);
        let expires = if duration_ms == u64::MAX {
            u64::MAX
        } else {
            now + duration_ms
        };
        p.buffs.push(Buff {
            kind,
            expires,
            stats,
            tick_at: now + 2000,
        });
        self.refresh_stats(id, false);
        self.send_player_stats(id);
        self.send_to(
            id,
            ServerMessage::BuffAdd(BuffSummary {
                kind,
                remaining_ms: duration_ms,
            }),
        );
        if buff_type::is_visible(kind) {
            self.events
                .push((id, ServerMessage::ObjectBuff { id, kind, on: true }));
        }
        if kind == buff_type::STRENGTH_OF_FAITH {
            self.refresh_pets(id);
        }
    }

    pub(super) fn buff_remove(&mut self, id: ObjectId, kind: u16) {
        let Some(p) = self.objects.get_mut(&id).and_then(|o| o.player_mut()) else {
            return;
        };
        let before = p.buffs.len();
        p.buffs.retain(|b| b.kind != kind);
        if p.buffs.len() == before {
            return;
        }
        if kind == buff_type::CLOAK {
            // Ghost Walk cannot outlive the cloak.
            p.buffs.retain(|b| b.kind != buff_type::GHOST_WALK);
            self.send_to(
                id,
                ServerMessage::BuffRemove {
                    kind: buff_type::GHOST_WALK,
                },
            );
        }
        self.refresh_stats(id, false);
        self.send_player_stats(id);
        self.send_to(id, ServerMessage::BuffRemove { kind });
        if buff_type::is_visible(kind) {
            self.events.push((
                id,
                ServerMessage::ObjectBuff {
                    id,
                    kind,
                    on: false,
                },
            ));
        }
        if kind == buff_type::STRENGTH_OF_FAITH {
            self.refresh_pets(id);
        }
    }

    pub(super) fn process_buffs(&mut self) {
        let now = self.now;
        let mut expired = Vec::new();
        let mut drains = Vec::new();
        for o in self.players() {
            if let Some(p) = o.player() {
                for b in &p.buffs {
                    if b.expires <= now {
                        expired.push((o.id, b.kind));
                    } else if b.kind == buff_type::CLOAK && now >= b.tick_at {
                        // Cloak drains HP every 2 s and drops instead of killing.
                        if b.stats.cloak_damage >= o.hp {
                            expired.push((o.id, b.kind));
                        } else {
                            drains.push((o.id, b.stats.cloak_damage));
                        }
                    }
                }
            }
        }
        for (id, amount) in drains {
            let (hp, max_hp) = {
                let o = self.objects.get_mut(&id).unwrap();
                o.hp -= amount;
                if let Some(b) = o
                    .player_mut()
                    .and_then(|p| p.buffs.iter_mut().find(|b| b.kind == buff_type::CLOAK))
                {
                    b.tick_at = now + 2000;
                }
                (o.hp, o.max_hp)
            };
            self.events
                .push((id, ServerMessage::HealthChanged { id, hp, max_hp }));
            let stats = self.player_stats(&self.objects[&id]);
            self.send_to(id, ServerMessage::StatsChanged(stats));
        }
        for (id, kind) in expired {
            self.buff_remove(id, kind);
        }
    }

    /// Zircon `ApplyPoison`: a stronger instance of the same type wins.
    pub(super) fn apply_poison(&mut self, target: ObjectId, poison: Poison) {
        let Some(o) = self.objects.get_mut(&target) else {
            return;
        };
        // Zircon: Endurance makes players immune to every poison.
        if o.has_buff(buff_type::ENDURANCE) {
            return;
        }
        if let Some(existing) = o.poisons.iter().position(|p| p.kind == poison.kind) {
            if o.poisons[existing].value > poison.value {
                return;
            }
            o.poisons.remove(existing);
        }
        let was = !o.poisons.is_empty();
        o.poisons.push(poison);
        if !was {
            self.events.push((
                target,
                ServerMessage::ObjectPoisoned {
                    id: target,
                    poisoned: true,
                },
            ));
        }
    }

    pub(super) fn process_poisons(&mut self) {
        let now = self.now;
        let ids: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| !o.poisons.is_empty())
            .map(|o| o.id)
            .collect();
        for id in ids {
            let (damage, owner, cleared) = {
                let o = self.objects.get_mut(&id).unwrap();
                let mut damage = 0;
                let mut owner = None;
                for p in o.poisons.iter_mut() {
                    if now < p.next_tick {
                        continue;
                    }
                    p.next_tick = now + 2000;
                    p.ticks_left -= 1;
                    if p.kind == poison_kind::GREEN || p.kind == poison_kind::HELL_FIRE {
                        damage += p.value;
                        owner = p.owner;
                    }
                }
                o.poisons.retain(|p| p.ticks_left >= 0);
                let cleared = o.poisons.is_empty();
                if o.dead {
                    damage = 0;
                }
                // Poison never kills (Zircon `CanKill = false`).
                damage = damage.min(o.hp - 1).max(0);
                (damage, owner, cleared)
            };
            if damage > 0 {
                let attacker = owner.unwrap_or(id);
                let o = self.objects.get_mut(&id).unwrap();
                o.hp -= damage;
                let (hp, max_hp) = (o.hp, o.max_hp);
                self.events
                    .push((id, ServerMessage::HealthChanged { id, hp, max_hp }));
                if let Some(m) = self.objects.get_mut(&id).and_then(|o| o.monster_mut()) {
                    if m.target.is_none() && attacker != id {
                        m.target = Some(attacker);
                    }
                }
            }
            if cleared {
                self.events.push((
                    id,
                    ServerMessage::ObjectPoisoned {
                        id,
                        poisoned: false,
                    },
                ));
            }
        }
    }

    pub(super) fn process_heals(&mut self) {
        let now = self.now;
        let ids: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| o.heal.is_some())
            .map(|o| o.id)
            .collect();
        for id in ids {
            let o = self.objects.get_mut(&id).unwrap();
            let Some(h) = o.heal.as_mut() else { continue };
            if now < h.next_tick {
                continue;
            }
            h.next_tick = now + 1000;
            let amount = h.pool.min(h.cap);
            h.pool -= amount;
            o.hp = (o.hp + amount).min(o.max_hp);
            if o.hp >= o.max_hp || h.pool <= 0 || o.dead {
                o.heal = None;
            }
            let (hp, max_hp) = (o.hp, o.max_hp);
            self.events
                .push((id, ServerMessage::HealthChanged { id, hp, max_hp }));
            if self.objects[&id].is_player() {
                self.send_player_stats(id);
            }
        }
    }
}
