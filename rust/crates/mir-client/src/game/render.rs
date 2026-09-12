use super::*;

impl Game {
    pub(super) fn body_sprite(&self, o: &ClientObject) -> Option<(u16, u32)> {
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
            Appearance::Spell { effect } => {
                let library = match *effect {
                    mir_proto::spell_effect::FIRE_WALL => effects::MAGIC,
                    mir_proto::spell_effect::TEMPEST => effects::MAGIC_EX2,
                    mir_proto::spell_effect::POISONOUS_CLOUD => effects::MAGIC_EX4,
                    _ => return None,
                };
                Some((library, o.sprite_index(0)))
            }
            Appearance::Npc { .. } => Some((lib::NPC, o.sprite_index(0))),
            Appearance::Item { info, .. } => {
                let image = self.catalog.get(*info).map(|d| d.image).unwrap_or(0);
                Some((lib::GROUND, image.max(0) as u32))
            }
        }
    }

    /// Weapon library and index for a player, if one is equipped.
    pub(super) fn weapon_sprite(&self, o: &ClientObject) -> Option<(u16, u32)> {
        let Appearance::Player { gender, weapon, .. } = &o.appearance else {
            return None;
        };
        let shape = (*weapon)?;
        let library = weapon_library(shape, *gender == Gender::Female)?;
        let draw_shape = if shape >= 1000 { shape - 1000 } else { shape };
        Some((library, o.draw_frame() + (draw_shape as u32 % 10) * 5000))
    }

    // ---- rendering ----------------------------------------------------------------

    pub(super) fn sprite(
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

    #[allow(clippy::too_many_arguments)]
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

            // Floor pass (upstream MapControl fix): cell-sized static middle and
            // front tiles belong to the floor and never occlude objects.
            for y in y_range.clone() {
                let draw_y = (y - uy + view.off_y + 1) * CELL_H + view.poy - view.umy;
                for x in x_range.clone() {
                    let Some(cell) = map.cell(x, y) else { continue };
                    let draw_x = (x - ux + view.off_x) * CELL_W + view.pox - view.umx;
                    for (file, index, animated) in [
                        (cell.middle_file, cell.middle_image, cell.middle_animated()),
                        (cell.front_file, cell.front_image, cell.front_animated()),
                    ] {
                        if file == 0 || animated {
                            continue;
                        }
                        let Some(library) = kr_library(file) else {
                            continue;
                        };
                        let Some(info) = self.assets.info(library, index as u32) else {
                            continue;
                        };
                        let (w, h) = (info.width as i32, info.height as i32);
                        if !((w == 48 && h == 32) || (w == 96 && h == 64)) {
                            continue;
                        }
                        if let Some(r) = Self::sprite(
                            &mut self.assets,
                            renderer,
                            gpu,
                            library,
                            index as u32,
                            Surface::Image,
                        ) {
                            renderer.draw(
                                r,
                                draw_x as f32,
                                (draw_y - CELL_H) as f32,
                                white,
                                Blend::Alpha,
                            );
                        }
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
                        // Static cell-sized tiles were drawn by the floor pass.
                        if cell_sized && !animated {
                            continue;
                        }
                        let draw_x = (x - ux + view.off_x) * CELL_W + view.pox - view.umx;
                        let py = if cell_sized {
                            draw_y - CELL_H
                        } else {
                            draw_y - h
                        };
                        let use_blend = blend;
                        let _ = layer;
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
                if o.is_spell() {
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

    pub(super) fn draw_object(
        &mut self,
        id: ObjectId,
        view: &View,
        gpu: &Gpu,
        renderer: &mut SpriteRenderer,
    ) {
        let Some(o) = self.objects.get(&id) else {
            return;
        };
        let Some((library, index)) = self.body_sprite(o) else {
            return;
        };
        let (dx, dy) = view.object_px(o);
        if o.is_spell() {
            let color = match &o.appearance {
                Appearance::Spell { effect }
                    if matches!(
                        *effect,
                        mir_proto::spell_effect::FIRE_WALL | mir_proto::spell_effect::TEMPEST
                    ) =>
                {
                    [1.0, 1.0, 1.0, 0.55]
                }
                _ => [139.0 / 255.0, 69.0 / 255.0, 19.0 / 255.0, 1.0],
            };
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
                        color,
                        Blend::Screen,
                    );
                }
            }
            return;
        }
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
    pub(super) fn draw_layer(
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
}
