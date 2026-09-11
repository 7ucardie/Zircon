//! In-game windows: inventory, character (equipment), NPC dialog and shop.
//! Layouts follow Zircon's `InventoryDialog`, `CharacterDialog`,
//! `NPCDialog` and `NPCGoodsDialog`; the first two use the standard window
//! chrome because this asset set lacks their dedicated panel images.

use mir_proto::{
    item_type, parse_dialog, ClientMessage, DialogPart, Good, Grid, ItemInstance, EQUIPMENT_SIZE,
    INVENTORY_SIZE,
};

use crate::assets::lib;
use crate::gfx::Blend;
use crate::items::{stat_name, type_name, ItemCatalog, ItemDef};
use crate::ui::{Button, Ctx, Rect, GOLD};

const CELL: f32 = 36.0;
const PITCH: f32 = 37.0;

#[allow(dead_code)]
pub struct NpcDialog {
    pub npc: mir_proto::ObjectId,
    pub page: i32,
    pub parts: Vec<DialogPart>,
    pub dialog_type: i32,
    pub goods: Vec<Good>,
    pub sell_types: Vec<u8>,
    pub selected_good: Option<usize>,
    pub last_button: u64,
}

impl NpcDialog {
    pub fn new(
        npc: mir_proto::ObjectId,
        page: i32,
        say: &str,
        dialog_type: i32,
        goods: Vec<Good>,
        sell_types: Vec<u8>,
    ) -> NpcDialog {
        NpcDialog {
            npc,
            page,
            parts: parse_dialog(say),
            dialog_type,
            goods,
            sell_types,
            selected_good: None,
            last_button: 0,
        }
    }
}

#[derive(Default)]
pub struct WindowState {
    pub inventory_open: bool,
    pub character_open: bool,
    pub npc: Option<NpcDialog>,
    /// Slot picked up with the mouse, waiting for a destination.
    pub carrying: Option<(Grid, u8)>,
    pub tooltip: Option<(i32, f32, f32)>,
    buy_button: Option<Button>,
    close_all_hint: bool,
}

pub struct Bag<'a> {
    pub inventory: &'a [Option<ItemInstance>],
    pub equipment: &'a [Option<ItemInstance>],
    pub gold: u64,
    pub weights: &'a mir_proto::Weights,
    pub stats: &'a mir_proto::PlayerStats,
    pub catalog: &'a ItemCatalog,
}

/// Draw the item icon (StoreItems) centred in a cell, plus the count.
fn draw_item_cell(
    c: &mut Ctx,
    catalog: &ItemCatalog,
    r: Rect,
    item: Option<&ItemInstance>,
    hover: bool,
    selected: bool,
) {
    let bg = if selected {
        [125.0 / 255.0, 1.0, 125.0 / 255.0, 0.5]
    } else {
        [0.0, 0.0, 0.0, 0.55]
    };
    c.fill(r, bg);
    c.border(
        r,
        if hover || selected {
            [0, 255, 0, 255]
        } else {
            [99, 83, 50, 255]
        },
    );
    let Some(item) = item else { return };
    let Some(def) = catalog.get(item.info) else {
        return;
    };
    if let Some(info) = c.assets.info(lib::STORE_ITEMS, def.image as u32) {
        let (w, h) = (info.width as f32, info.height as f32);
        let scale = (r.w / w).min(r.h / h).min(1.0);
        let (dw, dh) = (w * scale, h * scale);
        if let Some(sprite) = c.sprite(lib::STORE_ITEMS, def.image as u32) {
            c.renderer.draw_scaled(
                sprite,
                r.x + (r.w - dw) / 2.0,
                r.y + (r.h - dh) / 2.0,
                dw,
                dh,
                [1.0, 1.0, 1.0, 1.0],
                Blend::Alpha,
            );
        }
    }
    if item.count > 1 {
        let s = item.count.to_string();
        let w = c.text.width(&s, 11);
        c.text.draw(
            &s,
            11,
            r.x + r.w - w - 2.0,
            r.y + r.h - 14.0,
            [255, 255, 255, 255],
        );
    }
}

fn tooltip(
    c: &mut Ctx,
    def: &ItemDef,
    item: Option<&ItemInstance>,
    x: f32,
    y: f32,
    price: Option<u64>,
) {
    let mut lines: Vec<(String, [u8; 4])> = vec![(def.name.clone(), [255, 255, 255, 255])];
    lines.push((type_name(def.item_type).to_string(), [200, 200, 200, 255]));
    for (k, v) in &def.stats {
        if let Some(n) = stat_name(*k) {
            lines.push((format!("{n} +{v}"), [180, 220, 255, 255]));
        }
    }
    if def.required_amount > 0 && def.required_type == 0 {
        lines.push((
            format!("Requires level {}", def.required_amount),
            [255, 200, 120, 255],
        ));
    }
    if let Some(i) = item {
        if i.max_durability > 0 {
            lines.push((
                format!("Durability {}/{}", i.durability, i.max_durability),
                [200, 200, 200, 255],
            ));
        }
    }
    lines.push((format!("Weight {}", def.weight), [200, 200, 200, 255]));
    if let Some(p) = price {
        lines.push((format!("Price {p} gold"), [255, 230, 120, 255]));
    } else if def.price > 0 {
        lines.push((format!("Value {} gold", def.price), [200, 200, 200, 255]));
    }
    if !def.description.is_empty() {
        lines.push((def.description.clone(), [170, 170, 170, 255]));
    }
    let w = lines
        .iter()
        .map(|(l, _)| c.text.width(l, 12))
        .fold(0.0, f32::max)
        + 16.0;
    let h = lines.len() as f32 * 16.0 + 8.0;
    let r = Rect::new(x + 16.0, y + 8.0, w, h);
    c.fill(r, [0.0, 0.0, 0.0, 0.85]);
    c.border(r, GOLD);
    for (i, (l, col)) in lines.iter().enumerate() {
        c.text
            .draw(l, 12, r.x + 8.0, r.y + 4.0 + i as f32 * 16.0, *col);
    }
}

/// Zircon `CharacterDialog` equipment slot rectangles (relative to the window).
fn equipment_rects() -> [(usize, Rect, Option<u32>); 16] {
    use mir_proto::slot::*;
    [
        (WEAPON, Rect::new(58.0, 122.0, 65.0, 90.0), None),
        (ARMOUR, Rect::new(120.0, 123.0, 70.0, 150.0), None),
        (HELMET, Rect::new(140.0, 90.0, 35.0, 35.0), None),
        (TORCH, Rect::new(10.0, 196.0, 36.0, 36.0), Some(38)),
        (NECKLACE, Rect::new(10.0, 157.0, 36.0, 36.0), Some(33)),
        (BRACELET_L, Rect::new(244.0, 157.0, 36.0, 36.0), Some(32)),
        (BRACELET_R, Rect::new(283.0, 157.0, 36.0, 36.0), Some(32)),
        (RING_L, Rect::new(244.0, 196.0, 36.0, 36.0), Some(31)),
        (RING_R, Rect::new(283.0, 196.0, 36.0, 36.0), Some(31)),
        (SHOES, Rect::new(10.0, 235.0, 36.0, 36.0), Some(36)),
        (POISON, Rect::new(244.0, 274.0, 36.0, 36.0), Some(40)),
        (AMULET, Rect::new(283.0, 235.0, 36.0, 36.0), Some(39)),
        (12, Rect::new(244.0, 235.0, 36.0, 36.0), Some(81)),
        (13, Rect::new(283.0, 118.0, 36.0, 36.0), Some(82)),
        (14, Rect::new(244.0, 118.0, 36.0, 36.0), Some(104)),
        (SHIELD, Rect::new(170.0, 170.0, 36.0, 36.0), None),
    ]
}

impl WindowState {
    /// Draw all open windows. Returns true when the mouse is over a window
    /// (so the world should ignore clicks) and pushes messages to send.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        c: &mut Ctx,
        bag: &Bag,
        width: i32,
        height: i32,
        out: &mut Vec<ClientMessage>,
    ) -> bool {
        let mouse = c.input.mouse;
        let mut over = false;
        self.tooltip = None;
        let _ = height;

        // ---- NPC dialog (top-left, Zircon chrome 380/381/382) ----
        let mut npc_height = 0.0;
        if let Some(d) = &mut self.npc {
            // Lay out text to find the needed height.
            let text_w = 350.0;
            let (lines, _) = layout_parts(c, &d.parts, text_w);
            let text_h = lines.len() as f32 * 18.0;
            let overflow = text_h - 140.0 - 64.0 + 35.0 + 45.0;
            let rows = if overflow > 0.0 {
                ((overflow / 20.0) as usize).min(6)
            } else {
                0
            };
            let h = 140.0 + 64.0 + rows as f32 * 20.0;
            npc_height = h;
            let r = Rect::new(0.0, 0.0, 380.0, h);
            c.draw(lib::GAME_INTER, 380, 0.0, 0.0);
            for i in 0..rows {
                c.draw(lib::GAME_INTER, 381, 0.0, 140.0 + i as f32 * 20.0);
            }
            c.draw(lib::GAME_INTER, 382, 0.0, 140.0 + rows as f32 * 20.0);
            if r.contains(mouse.0, mouse.1) {
                over = true;
            }
            let close = Rect::new(380.0 - 21.0 - 3.0, 3.0, 21.0, 21.0);
            let close_hover = close.contains(mouse.0, mouse.1);
            c.draw(
                lib::INTERFACE,
                15,
                close.x,
                close.y + if close_hover { 1.0 } else { 0.0 },
            );
            let mut clicked_button = None;
            let mut y = 45.0;
            for line in &lines {
                let mut x = 15.0;
                for (word, button) in line {
                    let w = c.text.width(word, 13);
                    match button {
                        Some(id) => {
                            let hit = Rect::new(x, y, w, 18.0).contains(mouse.0, mouse.1);
                            let col = if hit {
                                [255, 60, 60, 255]
                            } else {
                                [255, 255, 0, 255]
                            };
                            c.text.draw(word, 13, x, y, col);
                            if hit && c.input.lmb_released && c.now >= d.last_button + 300 {
                                clicked_button = Some(*id);
                            }
                        }
                        None => c.text.draw(word, 13, x, y, [255, 255, 255, 255]),
                    }
                    x += w;
                }
                y += 18.0;
            }
            let mut close_now = (close_hover && c.input.lmb_released) || c.input.escape;
            if let Some(id) = clicked_button {
                d.last_button = c.now;
                if id == 0 {
                    close_now = true;
                } else {
                    out.push(ClientMessage::NpcButton { button: id });
                }
            }
            if close_now {
                self.npc = None;
                out.push(ClientMessage::NpcClose);
            }
        }

        // ---- Goods (shop) window under the dialog ----
        if let Some(d) = &mut self.npc {
            if d.dialog_type == 1 && !d.goods.is_empty() {
                let rows = d.goods.len().min(7);
                let win = Rect::new(0.0, npc_height, 245.0, 37.0 + rows as f32 * 43.0 + 50.0);
                if win.contains(mouse.0, mouse.1) {
                    over = true;
                }
                let closed = c.window(win, "Goods", true);
                for (i, g) in d.goods.iter().take(7).enumerate() {
                    let row = Rect::new(win.x + 10.0, win.y + 37.0 + i as f32 * 43.0, 219.0, 40.0);
                    let hover = row.contains(mouse.0, mouse.1);
                    let selected = d.selected_good == Some(i);
                    c.fill(
                        row,
                        if selected {
                            [80.0 / 255.0, 80.0 / 255.0, 125.0 / 255.0, 1.0]
                        } else {
                            [25.0 / 255.0, 20.0 / 255.0, 0.0, 1.0]
                        },
                    );
                    c.border(row, if hover { GOLD } else { [99, 83, 50, 255] });
                    let cell = Rect::new(row.x + 2.0, row.y + 2.0, CELL, CELL);
                    let inst = ItemInstance {
                        id: 0,
                        info: g.info,
                        count: 1,
                        durability: 0,
                        max_durability: 0,
                    };
                    draw_item_cell(c, bag.catalog, cell, Some(&inst), false, false);
                    let name = bag.catalog.name(g.info);
                    c.text
                        .draw(&name, 12, row.x + 40.0, row.y + 2.0, [255, 255, 255, 255]);
                    let usable = bag
                        .catalog
                        .get(g.info)
                        .map(|d| {
                            d.required_type != 0 || d.required_amount <= bag.stats.level as i32
                        })
                        .unwrap_or(true);
                    let (req, rcol) = if usable {
                        ("Can use item", [127, 255, 212, 255])
                    } else {
                        ("Cannot use item", [255, 60, 60, 255])
                    };
                    c.text.draw(req, 11, row.x + 40.0, row.y + 22.0, rcol);
                    let price = format!("{}", g.price);
                    let pw = c.text.width(&price, 12);
                    let pcol = if g.price <= bag.gold {
                        [255, 255, 0, 255]
                    } else {
                        [255, 60, 60, 255]
                    };
                    c.text
                        .draw(&price, 12, row.x + row.w - pw - 30.0, row.y + 22.0, pcol);
                    c.draw(lib::STORE_ITEMS, 121, row.x + row.w - 24.0, row.y + 22.0);
                    if hover {
                        if let Some(def) = bag.catalog.get(g.info) {
                            self.tooltip = Some((def.index, mouse.0, mouse.1));
                        }
                        if c.input.lmb_pressed {
                            d.selected_good = Some(i);
                        }
                    }
                }
                let buy = self
                    .buy_button
                    .get_or_insert_with(|| Button::default_style(0.0, 0.0, 80.0, "Buy"));
                buy.pos = (win.x + 30.0, win.y + win.h - 43.0);
                buy.enabled = d.selected_good.is_some();
                if buy.update(c) {
                    if let Some(g) = d.selected_good.and_then(|i| d.goods.get(i)) {
                        out.push(ClientMessage::NpcBuy {
                            info: g.info,
                            count: 1,
                        });
                    }
                }
                if !d.sell_types.is_empty() {
                    c.text.draw(
                        "Right-click bag items to sell",
                        11,
                        win.x + 10.0,
                        win.y + win.h - 62.0,
                        [200, 200, 160, 255],
                    );
                    self.inventory_open = true;
                }
                if closed {
                    self.npc = None;
                    out.push(ClientMessage::NpcClose);
                }
            }
        }
        let sell_types: Vec<u8> = self
            .npc
            .as_ref()
            .map(|d| d.sell_types.clone())
            .unwrap_or_default();

        // ---- Inventory window (right side) ----
        if self.inventory_open {
            let win = Rect::new(width as f32 - 263.0 - 10.0, 40.0, 263.0, 430.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            let closed = c.window(win, "Inventory", true);
            for i in 0..INVENTORY_SIZE {
                let (col, row) = (i % 6, i / 6);
                let r = Rect::new(
                    win.x + 20.0 + col as f32 * PITCH,
                    win.y + 39.0 + row as f32 * PITCH,
                    CELL,
                    CELL,
                );
                let hover = r.contains(mouse.0, mouse.1);
                let item = bag.inventory[i].as_ref();
                let selected = self.carrying == Some((Grid::Inventory, i as u8));
                draw_item_cell(c, bag.catalog, r, item, hover, selected);
                if hover {
                    if let Some(it) = item {
                        self.tooltip = Some((it.info, mouse.0, mouse.1));
                    }
                    if c.input.lmb_pressed {
                        self.click_slot(Grid::Inventory, i as u8, item.is_some(), out);
                    }
                    if c.input.rmb_pressed {
                        if let Some(it) = item {
                            let sellable = bag
                                .catalog
                                .get(it.info)
                                .map(|d| sell_types.contains(&d.item_type))
                                .unwrap_or(false);
                            if !sell_types.is_empty() && sellable {
                                out.push(ClientMessage::NpcSell {
                                    slots: vec![i as u8],
                                });
                            } else {
                                out.push(ClientMessage::ItemUse { slot: i as u8 });
                            }
                        }
                    }
                }
            }
            // Weight bar (GameInter 360) and gold.
            let bar_y = win.y + 355.0;
            if let Some(bar) = c.sprite(lib::GAME_INTER, 360) {
                let pct = (bag.weights.bag as f32 / bag.weights.max_bag.max(1) as f32).min(1.0);
                c.renderer
                    .draw_cropped(bar, win.x + 33.0, bar_y, pct, [1.0, 1.0, 1.0, 1.0]);
            }
            c.text.draw_centered(
                &format!("{}/{}", bag.weights.bag, bag.weights.max_bag),
                11,
                win.x + 131.0,
                bar_y - 2.0,
                [255, 255, 255, 255],
            );
            c.draw(lib::STORE_ITEMS, 121, win.x + 30.0, win.y + 383.0);
            c.text
                .draw("Gold", 12, win.x + 55.0, win.y + 380.0, [218, 165, 32, 255]);
            let gold = format!("{}", bag.gold);
            let gw = c.text.width(&gold, 12);
            c.text.draw(
                &gold,
                12,
                win.x + 240.0 - gw,
                win.y + 380.0,
                [255, 255, 255, 255],
            );
            c.text.draw(
                "Right-click: use / equip",
                10,
                win.x + 30.0,
                win.y + 400.0,
                [160, 160, 160, 255],
            );
            if closed {
                self.inventory_open = false;
                self.carrying = None;
            }
        }

        // ---- Character window (left side) ----
        if self.character_open {
            let win = Rect::new(10.0, 40.0, 320.0, 420.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            let closed = c.window(win, "Character", true);
            for (slot, rr, glyph) in equipment_rects() {
                let r = Rect::new(win.x + rr.x, win.y + rr.y, rr.w, rr.h);
                let hover = r.contains(mouse.0, mouse.1);
                let item = bag.equipment.get(slot).and_then(|s| s.as_ref());
                let selected = self.carrying == Some((Grid::Equipment, slot as u8));
                draw_item_cell(c, bag.catalog, r, item, hover, selected);
                if item.is_none() {
                    if let Some(g) = glyph {
                        if let Some(info) = c.assets.info(lib::INTERFACE, g) {
                            c.draw_tinted(
                                lib::INTERFACE,
                                g,
                                r.x + (r.w - info.width as f32) / 2.0,
                                r.y + (r.h - info.height as f32) / 2.0,
                                [1.0, 1.0, 1.0, 0.25],
                                Blend::Alpha,
                            );
                        }
                    }
                }
                if hover {
                    if let Some(it) = item {
                        self.tooltip = Some((it.info, mouse.0, mouse.1));
                    }
                    if c.input.lmb_pressed {
                        self.click_slot(Grid::Equipment, slot as u8, item.is_some(), out);
                    }
                    if c.input.rmb_pressed && item.is_some() {
                        // Unequip into the first free bag slot.
                        if let Some(free) = bag.inventory.iter().position(|s| s.is_none()) {
                            out.push(ClientMessage::ItemMove {
                                from: Grid::Equipment,
                                from_slot: slot as u8,
                                to: Grid::Inventory,
                                to_slot: free as u8,
                            });
                        }
                    }
                }
            }
            let s = bag.stats;
            let rows = [
                ("Level", s.level.to_string()),
                ("HP", format!("{}/{}", s.hp, s.max_hp)),
                ("MP", format!("{}/{}", s.mp, s.max_mp)),
                ("DC", format!("{}-{}", s.min_dc, s.max_dc)),
                ("AC", format!("{}-{}", s.min_ac, s.max_ac)),
                ("Accuracy", s.accuracy.to_string()),
                ("Agility", s.agility.to_string()),
                (
                    "Wear",
                    format!("{}/{}", bag.weights.wear, bag.weights.max_wear),
                ),
                (
                    "Hand",
                    format!("{}/{}", bag.weights.hand, bag.weights.max_hand),
                ),
            ];
            for (i, (k, v)) in rows.iter().enumerate() {
                let (col, row) = (i % 2, i / 2);
                let x = win.x + 20.0 + col as f32 * 150.0;
                let y = win.y + 319.0 + row as f32 * 18.0;
                c.text.draw(k, 12, x, y, GOLD);
                c.text.draw(v, 12, x + 60.0, y, [255, 255, 255, 255]);
            }
            c.text.draw(
                "Right-click: unequip",
                10,
                win.x + 20.0,
                win.y + 400.0,
                [160, 160, 160, 255],
            );
            if closed {
                self.character_open = false;
                self.carrying = None;
            }
        }

        // Drop the carried item when clicking outside any window.
        if c.input.lmb_pressed && !over {
            self.carrying = None;
        }
        if let Some((info, x, y)) = self.tooltip {
            let price = self
                .npc
                .as_ref()
                .and_then(|d| d.goods.iter().find(|g| g.info == info))
                .map(|g| g.price);
            if let Some(def) = bag.catalog.get(info) {
                tooltip(c, def, None, x, y, price);
            }
        }
        let _ = EQUIPMENT_SIZE;
        let _ = item_type::NOTHING;
        self.close_all_hint = false;
        over
    }

    fn click_slot(&mut self, grid: Grid, slot: u8, has_item: bool, out: &mut Vec<ClientMessage>) {
        match self.carrying {
            None => {
                if has_item {
                    self.carrying = Some((grid, slot));
                }
            }
            Some((from, from_slot)) => {
                if (from, from_slot) != (grid, slot) {
                    out.push(ClientMessage::ItemMove {
                        from,
                        from_slot,
                        to: grid,
                        to_slot: slot,
                    });
                }
                self.carrying = None;
            }
        }
    }
}

/// Word-wrap dialog parts into lines of (word, button id).
type Line = Vec<(String, Option<i32>)>;

fn layout_parts(c: &mut Ctx, parts: &[DialogPart], max_w: f32) -> (Vec<Line>, f32) {
    let mut lines: Vec<Line> = vec![Vec::new()];
    let mut x = 0.0;
    let push = |c: &mut Ctx, lines: &mut Vec<Line>, x: &mut f32, word: String, id: Option<i32>| {
        let w = c.text.width(&word, 13);
        if *x + w > max_w && *x > 0.0 {
            lines.push(Vec::new());
            *x = 0.0;
        }
        *x += w;
        lines.last_mut().unwrap().push((word, id));
    };
    for p in parts {
        match p {
            DialogPart::NewLine => {
                lines.push(Vec::new());
                x = 0.0;
            }
            DialogPart::Text(t) => {
                for word in t.split_inclusive(' ') {
                    push(c, &mut lines, &mut x, word.to_string(), None);
                }
            }
            DialogPart::Button { label, id } => {
                push(c, &mut lines, &mut x, label.clone(), Some(*id));
            }
        }
    }
    (lines, max_w)
}
