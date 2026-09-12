use super::*;

impl Game {
    /// Inventory, character and NPC windows on top of everything; consumes
    /// clicks over them.
    pub(super) fn draw_windows(
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
                        self.use_item_time = now + use_item_lock(def.durability);
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

    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_hud(
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
        // Own buffs: Zircon `BuffDialog` icons (CBIcon), top-right, 27 px pitch.
        let buffs = self.buffs.clone();
        for (i, (kind, until)) in buffs.iter().enumerate() {
            let icon = buff_icon(*kind);
            let x = width as f32 - 30.0 - (i % 6) as f32 * 27.0;
            let y = 6.0 + (i / 6) as f32 * 27.0;
            if let Some(r) = Self::sprite(&mut self.assets, renderer, gpu, 21, icon, Surface::Image)
            {
                renderer.draw(r, x, y, white, Blend::Alpha);
            }
            if *until != u64::MAX {
                let secs = until.saturating_sub(now).div_ceil(1000);
                text.draw(
                    &secs.to_string(),
                    10,
                    x + 2.0,
                    y + 14.0,
                    [255, 255, 255, 230],
                );
            }
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
