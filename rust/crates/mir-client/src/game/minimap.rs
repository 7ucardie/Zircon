//! Minimap and big map (Zircon `MiniMapDialog` / `BigMapDialog`).
//!
//! Both draw the same image: `MapInfo.MiniMap` in the `MiniMap` library,
//! which covers the whole map, so a cell is `image_size / map_size` pixels.
//! The image is drawn at 1:1 scrolled so the player sits in the middle of
//! the panel and clipped to its edges (Zircon `ClipMap`), with coloured
//! dots for everything the player can see.

use super::*;
use crate::assets::lib;
use crate::gfx::Blend;
use crate::ui::{Ctx, Rect};

/// Panel size of the minimap (Zircon's default window is 200x200).
const MINI_SIZE: f32 = 200.0;
/// Margin from the screen edges.
const MARGIN: f32 = 10.0;
/// Room for the buff icons in the top-right corner.
const MINI_TOP: f32 = 70.0;
/// Room above the big map, and below it for the bottom HUD: the window sits
/// in the band between them so the health bars, spell bar and belt stay
/// readable while it is open.
const BIG_TOP: f32 = 40.0;
const BIG_BOTTOM: f32 = 190.0;

/// Minimap visibility, cycled by its key: Zircon goes shown -> dimmed ->
/// hidden (`GameScene.Input` `MapMiniWindow`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiniMode {
    Shown,
    Dimmed,
    Hidden,
}

#[derive(Debug, Clone)]
pub struct MiniMap {
    pub mode: MiniMode,
    pub big_open: bool,
}

impl Default for MiniMap {
    fn default() -> MiniMap {
        // ZIRCON_OPEN=bigmap / minimap for screenshots.
        let open: Vec<String> = std::env::var("ZIRCON_OPEN")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        MiniMap {
            mode: if open.iter().any(|s| s == "nominimap") {
                MiniMode::Hidden
            } else {
                MiniMode::Shown
            },
            big_open: open.iter().any(|s| s == "bigmap"),
        }
    }
}

impl MiniMap {
    /// Zircon cycles the minimap with one key: shown, dimmed, hidden.
    pub fn cycle(&mut self) {
        self.mode = match self.mode {
            MiniMode::Shown => MiniMode::Dimmed,
            MiniMode::Dimmed => MiniMode::Hidden,
            MiniMode::Hidden => MiniMode::Shown,
        };
    }

    fn opacity(&self) -> f32 {
        match self.mode {
            MiniMode::Shown => 1.0,
            MiniMode::Dimmed => 0.5,
            MiniMode::Hidden => 0.0,
        }
    }
}

/// One marker on the map, in map cells.
struct Dot {
    cell: Point,
    size: f32,
    color: [f32; 4],
}

const LIME: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const ORANGE: [f32; 4] = [1.0, 0.65, 0.0, 1.0];
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const DEEP_PINK: [f32; 4] = [1.0, 0.08, 0.58, 1.0];
const DEEP_SKY_BLUE: [f32; 4] = [0.0, 0.75, 1.0, 1.0];
const NPC_YELLOW: [f32; 4] = [1.0, 0.9, 0.2, 1.0];

impl Game {
    /// Everything worth a dot: the player (lime), group mates (lime),
    /// the partner (pink), guild mates (sky blue), other players (white),
    /// monsters (red, own pets orange) and NPCs (yellow).
    fn map_dots(&self) -> Vec<Dot> {
        let me = self.user;
        let my_name = self.user_name();
        let guild_members: Vec<&str> = self
            .guild
            .as_ref()
            .map(|g| g.members.iter().map(|m| m.name.as_str()).collect())
            .unwrap_or_default();
        let mut dots = Vec::new();
        for (id, o) in &self.objects {
            if o.is_spell() || o.is_item() {
                continue;
            }
            let (size, color) = if o.is_monster() {
                if o.dead {
                    continue;
                }
                match o.pet_owner() {
                    Some(owner) if Some(owner) == my_name => (4.0, ORANGE),
                    _ => (3.0, RED),
                }
            } else if o.is_npc() {
                (3.0, NPC_YELLOW)
            } else if Some(*id) == me {
                (5.0, LIME)
            } else if self.group.iter().any(|(g, _)| g == id) {
                (4.0, LIME)
            } else if self.partner.as_deref() == Some(o.name()) {
                (3.0, DEEP_PINK)
            } else if guild_members.contains(&o.name()) {
                (3.0, DEEP_SKY_BLUE)
            } else {
                (3.0, WHITE)
            };
            dots.push(Dot {
                cell: o.location,
                size,
                color,
            });
            if Some(*id) == me {
                // A white pip inside the lime square, so the player stands
                // out from the NPC and group dots.
                dots.push(Dot {
                    cell: o.location,
                    size: 2.0,
                    color: WHITE,
                });
            }
        }
        dots
    }

    /// Draw one map panel: the image scrolled so `centre` sits in the
    /// middle, clipped to the panel, with the dots on top.
    #[allow(clippy::too_many_arguments)]
    fn draw_map_panel(
        &mut self,
        gpu: &Gpu,
        renderer: &mut SpriteRenderer,
        area: Rect,
        index: i32,
        centre: Point,
        dots: &[Dot],
        opacity: f32,
    ) -> bool {
        // MapInfo.MiniMap of 0 means the map ships no image.
        if index <= 0 {
            return false;
        }
        let Some(map) = self.map.as_ref() else {
            return false;
        };
        let (map_w, map_h) = (map.width.max(1) as f32, map.height.max(1) as f32);
        let Some(region) = Self::sprite(
            &mut self.assets,
            renderer,
            gpu,
            lib::MINI_MAP,
            index as u32,
            Surface::Image,
        ) else {
            return false;
        };
        let (img_w, img_h) = (region.width as f32, region.height as f32);
        let scale_x = img_w / map_w;
        let scale_y = img_h / map_h;

        // Zircon `ClipMap`: keep the image over the panel, or centre it
        // when it is smaller than the panel.
        let mut off_x = area.w / 2.0 - scale_x * centre.x as f32;
        let mut off_y = area.h / 2.0 - scale_y * centre.y as f32;
        off_x = if img_w < area.w {
            (area.w - img_w) / 2.0
        } else {
            off_x.clamp(area.w - img_w, 0.0)
        };
        off_y = if img_h < area.h {
            (area.h - img_h) / 2.0
        } else {
            off_y.clamp(area.h - img_h, 0.0)
        };

        let src_x = (-off_x).max(0.0);
        let src_y = (-off_y).max(0.0);
        let dst_x = area.x + off_x.max(0.0);
        let dst_y = area.y + off_y.max(0.0);
        let w = (area.w - off_x.max(0.0)).min(img_w - src_x).max(0.0);
        let h = (area.h - off_y.max(0.0)).min(img_h - src_y).max(0.0);
        if w >= 1.0 && h >= 1.0 {
            let sub = region.sub(src_x as u32, src_y as u32, w as u32, h as u32);
            renderer.draw_scaled(
                sub,
                dst_x,
                dst_y,
                sub.width as f32,
                sub.height as f32,
                [1.0, 1.0, 1.0, opacity],
                Blend::Alpha,
            );
        }
        for d in dots {
            let x = area.x + off_x + scale_x * d.cell.x as f32 - d.size / 2.0;
            let y = area.y + off_y + scale_y * d.cell.y as f32 - d.size / 2.0;
            if x < area.x
                || y < area.y
                || x + d.size > area.x + area.w
                || y + d.size > area.y + area.h
            {
                continue;
            }
            let color = [d.color[0], d.color[1], d.color[2], d.color[3] * opacity];
            renderer.fill_rect(x, y, d.size, d.size, color);
        }
        true
    }

    /// Both map windows, drawn after the HUD so they sit above the world.
    pub(super) fn draw_map_windows(
        &mut self,
        gpu: &Gpu,
        renderer: &mut SpriteRenderer,
        text: &mut TextLayer,
        width: i32,
        height: i32,
        now: u64,
    ) {
        if self.map.is_none() {
            return;
        }
        let Some(user) = self.user() else {
            return;
        };
        let centre = user.location;
        let index = self.mini_map_index;
        let show_mini = self.minimap.mode != MiniMode::Hidden && !self.minimap.big_open;
        if !show_mini && !self.minimap.big_open {
            return;
        }
        let dots = self.map_dots();
        let name = self.map_name.clone();
        let opacity = self.minimap.opacity();

        let mut closed_mini = false;
        let mut closed_big = false;
        if show_mini {
            let win = Rect::new(
                width as f32 - MINI_SIZE - MARGIN,
                MINI_TOP,
                MINI_SIZE,
                MINI_SIZE,
            );
            let area = Rect::new(win.x + 4.0, win.y + 34.0, win.w - 8.0, win.h - 38.0);
            {
                let mut c = Ctx {
                    input: &self.input,
                    assets: &mut self.assets,
                    renderer,
                    gpu,
                    text,
                    now,
                };
                if c.window(win, &name, false) {
                    closed_mini = true;
                }
                c.fill(area, [0.0, 0.0, 0.0, 0.6 * opacity]);
            }
            if !self.draw_map_panel(gpu, renderer, area, index, centre, &dots, opacity) {
                text.draw(
                    "No map image.",
                    11,
                    area.x + 6.0,
                    area.y + 6.0,
                    [160, 160, 160, (160.0 * opacity) as u8],
                );
            }
            text.draw(
                &format!("{}, {}", centre.x, centre.y),
                11,
                area.x + 4.0,
                area.y + area.h - 16.0,
                [255, 255, 200, (255.0 * opacity) as u8],
            );
        }

        if self.minimap.big_open {
            let band = (height as f32 - BIG_TOP - BIG_BOTTOM).max(160.0);
            let win_w = (width as f32 - 80.0).min(900.0);
            let win_h = band.min(700.0);
            let win = Rect::new(
                (width as f32 - win_w) / 2.0,
                BIG_TOP + (band - win_h) / 2.0,
                win_w,
                win_h,
            );
            let area = Rect::new(win.x + 6.0, win.y + 36.0, win.w - 12.0, win.h - 44.0);
            {
                let mut c = Ctx {
                    input: &self.input,
                    assets: &mut self.assets,
                    renderer,
                    gpu,
                    text,
                    now,
                };
                if c.window(win, &format!("Map: {name}"), false) {
                    closed_big = true;
                }
                c.fill(area, [0.0, 0.0, 0.0, 0.85]);
            }
            if !self.draw_map_panel(gpu, renderer, area, index, centre, &dots, 1.0) {
                text.draw_centered(
                    "This map has no map image.",
                    13,
                    area.x + area.w / 2.0,
                    area.y + area.h / 2.0,
                    [200, 200, 200, 255],
                );
            }
            text.draw(
                &format!("{} ({}, {})  -  X closes", name, centre.x, centre.y),
                12,
                area.x + 6.0,
                area.y + area.h - 18.0,
                [255, 255, 200, 255],
            );
        }
        if closed_mini {
            self.minimap.mode = MiniMode::Hidden;
        }
        if closed_big {
            self.minimap.big_open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MiniMap, MiniMode};

    #[test]
    fn cycle_matches_zircon_order() {
        let mut m = MiniMap {
            mode: MiniMode::Shown,
            big_open: false,
        };
        m.cycle();
        assert_eq!(m.mode, MiniMode::Dimmed);
        m.cycle();
        assert_eq!(m.mode, MiniMode::Hidden);
        m.cycle();
        assert_eq!(m.mode, MiniMode::Shown);
    }
}
