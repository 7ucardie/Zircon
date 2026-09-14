//! The marketplace window (Zircon `MarketPlaceDialog`): a Search tab that
//! browses what everyone has listed and buys, and a Consign tab that lists
//! items out of the bag and takes them back.
//!
//! Zircon's dialog has a third tab, the game-gold Store, which is a separate
//! cash-shop system (`MarketPlaceStoreBuy`, game gold and hunt gold) and is
//! not part of the player auction house; it is not built here.

use mir_proto::{item_type, ClientMessage, ItemInstance, MarketListing, MarketSort, MARKET_PAGE};

use crate::items::ItemCatalog;
use crate::ui::{Button, Ctx, Rect, TextBox};

/// The item-type filters Zircon offers, in its own order. `None` is "any".
pub const TYPE_FILTERS: [(Option<u8>, &str); 9] = [
    (None, "Any"),
    (Some(item_type::WEAPON), "Weapon"),
    (Some(item_type::ARMOUR), "Armour"),
    (Some(item_type::HELMET), "Helmet"),
    (Some(item_type::NECKLACE), "Necklace"),
    (Some(item_type::RING), "Ring"),
    (Some(item_type::BRACELET), "Bracelet"),
    (Some(item_type::CONSUMABLE), "Potion"),
    (Some(item_type::BOOK), "Book"),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    #[default]
    Search,
    Consign,
}

#[derive(Default)]
pub struct MarketState {
    pub open: bool,
    pub tab: Tab,
    /// Results of the last search, and how many it matched in total.
    pub results: Vec<MarketListing>,
    pub total: u32,
    pub page: u32,
    pub sort: usize,
    pub filter: usize,
    /// What this account has listed.
    pub consignments: Vec<MarketListing>,
    /// Row picked on the current tab.
    pub selected: Option<u32>,
    /// Bag slot picked to list.
    pub consign_slot: Option<u8>,
    /// Search name, buy count, consign price, consign count, consign note.
    boxes: Vec<TextBox>,
    buttons: Vec<Button>,
    /// A search is owed to the server (tab opened, filter changed, paged).
    dirty: bool,
}

/// Indices into `boxes`.
const B_NAME: usize = 0;
const B_BUY_COUNT: usize = 1;
const B_PRICE: usize = 2;
const B_COUNT: usize = 3;
const B_NOTE: usize = 4;

impl MarketState {
    pub fn typing(&self) -> bool {
        self.open && self.boxes.iter().any(|b| b.focused)
    }

    /// Ask for a fresh page next frame (after a filter or tab change).
    pub fn refresh(&mut self) {
        self.dirty = true;
    }

    pub fn sort(&self) -> MarketSort {
        MarketSort::ALL[self.sort.min(MarketSort::ALL.len() - 1)].0
    }

    fn filter_type(&self) -> Option<u8> {
        TYPE_FILTERS[self.filter.min(TYPE_FILTERS.len() - 1)].0
    }

    fn search_msg(&self) -> ClientMessage {
        ClientMessage::MarketSearch {
            name: self
                .boxes
                .get(B_NAME)
                .map(|b| b.text.clone())
                .unwrap_or_default(),
            item_type: self.filter_type(),
            sort: self.sort(),
            page: self.page,
        }
    }

    /// How many pages the last search spans (at least one).
    pub fn pages(&self) -> u32 {
        self.total.div_ceil(MARKET_PAGE as u32).max(1)
    }

    /// Draw the window. Returns true while the mouse is over it.
    pub fn draw(
        &mut self,
        c: &mut Ctx,
        catalog: &ItemCatalog,
        inventory: &[Option<ItemInstance>],
        gold: u64,
        width: i32,
        out: &mut Vec<ClientMessage>,
    ) -> bool {
        if !self.open {
            return false;
        }
        if self.boxes.is_empty() {
            self.boxes = vec![
                TextBox::new(Rect::new(0.0, 0.0, 150.0, 22.0), 40),
                TextBox::new(Rect::new(0.0, 0.0, 60.0, 22.0), 9),
                TextBox::new(Rect::new(0.0, 0.0, 100.0, 22.0), 12),
                TextBox::new(Rect::new(0.0, 0.0, 60.0, 22.0), 9),
                TextBox::new(Rect::new(0.0, 0.0, 300.0, 22.0), 150),
            ];
        }
        if self.buttons.is_empty() {
            self.buttons = vec![
                Button::default_style(0.0, 0.0, 80.0, "Search"),
                Button::default_style(0.0, 0.0, 70.0, "Buy"),
                Button::default_style(0.0, 0.0, 60.0, "Prev"),
                Button::default_style(0.0, 0.0, 60.0, "Next"),
                Button::default_style(0.0, 0.0, 70.0, "List"),
                Button::default_style(0.0, 0.0, 80.0, "Cancel"),
            ];
        }
        let win = Rect::new(width as f32 / 2.0 - 320.0, 40.0, 640.0, 440.0);
        let mouse = c.input.mouse;
        let over = win.contains(mouse.0, mouse.1);
        if c.window(win, "Market Place", true) {
            self.open = false;
            return over;
        }

        // ---- Tabs ----
        let mut tx = win.x + 12.0;
        for (tab, label) in [(Tab::Search, "Search"), (Tab::Consign, "Consign")] {
            let r = Rect::new(tx, win.y + 30.0, 84.0, 22.0);
            let hot = r.contains(mouse.0, mouse.1);
            let on = self.tab == tab;
            c.fill(
                r,
                if on {
                    [0.35, 0.3, 0.18, 0.95]
                } else if hot {
                    [0.2, 0.2, 0.2, 0.8]
                } else {
                    [0.1, 0.1, 0.1, 0.7]
                },
            );
            c.border(r, [198, 166, 99, 255]);
            c.text.draw_centered(
                label,
                12,
                r.x + r.w / 2.0,
                r.y + 4.0,
                if on {
                    [255, 255, 200, 255]
                } else {
                    [190, 190, 190, 255]
                },
            );
            if hot && c.input.lmb_pressed && !on {
                self.tab = tab;
                self.selected = None;
                self.dirty = true;
            }
            tx += 90.0;
        }
        c.text.draw(
            &format!("Gold: {gold}"),
            12,
            win.x + win.w - 150.0,
            win.y + 34.0,
            [255, 220, 120, 255],
        );

        let fy = win.y + win.h - 3.0 - 42.0 + 8.0;
        match self.tab {
            Tab::Search => self.draw_search(c, catalog, win, fy, out),
            Tab::Consign => self.draw_consign(c, catalog, inventory, win, fy, out),
        }
        if self.dirty {
            self.dirty = false;
            out.push(self.search_msg());
        }
        over
    }

    fn draw_search(
        &mut self,
        c: &mut Ctx,
        catalog: &ItemCatalog,
        win: Rect,
        fy: f32,
        out: &mut Vec<ClientMessage>,
    ) {
        let mouse = c.input.mouse;
        let top = win.y + 60.0;
        c.text
            .draw("Name", 11, win.x + 12.0, top + 4.0, [220, 220, 220, 255]);
        self.boxes[B_NAME].rect = Rect::new(win.x + 52.0, top, 150.0, 22.0);
        self.boxes[B_NAME].update(c);

        // Item type and sort cycle on click, the way Zircon's combo boxes do
        // without the drop-down.
        let tr = Rect::new(win.x + 212.0, top, 96.0, 22.0);
        if cycle(c, tr, TYPE_FILTERS[self.filter].1) {
            self.filter = (self.filter + 1) % TYPE_FILTERS.len();
            self.page = 0;
            self.dirty = true;
        }
        let sr = Rect::new(win.x + 316.0, top, 120.0, 22.0);
        if cycle(c, sr, MarketSort::ALL[self.sort].1) {
            self.sort = (self.sort + 1) % MarketSort::ALL.len();
            self.page = 0;
            self.dirty = true;
        }
        let b = &mut self.buttons[0];
        b.pos = (win.x + 444.0, top);
        b.enabled = true;
        if b.update(c) {
            self.page = 0;
            self.dirty = true;
        }

        // ---- Results ----
        let rows = self.results.clone();
        let mut y = top + 34.0;
        for l in &rows {
            let r = Rect::new(win.x + 12.0, y, win.w - 24.0, 32.0);
            let hot = r.contains(mouse.0, mouse.1);
            let on = self.selected == Some(l.index);
            c.fill(
                r,
                if on {
                    [0.3, 0.3, 0.5, 0.85]
                } else if hot {
                    [0.22, 0.22, 0.3, 0.6]
                } else {
                    [0.0, 0.0, 0.0, 0.35]
                },
            );
            crate::windows::draw_item_cell(
                c,
                catalog,
                Rect::new(r.x + 2.0, r.y + 2.0, 28.0, 28.0),
                Some(&l.item),
                false,
                false,
            );
            let name = catalog.name(l.item.info);
            let label = if l.item.count > 1 {
                format!("{name} x{}", l.item.count)
            } else {
                name
            };
            c.text
                .draw(&label, 12, r.x + 38.0, r.y + 2.0, [255, 255, 255, 255]);
            c.text.draw(
                &format!("{} each", l.price),
                11,
                r.x + 38.0,
                r.y + 17.0,
                [255, 220, 120, 255],
            );
            c.text.draw(
                &l.seller,
                11,
                r.x + r.w - 220.0,
                r.y + 2.0,
                if l.is_owner {
                    [150, 255, 150, 255]
                } else {
                    [200, 200, 255, 255]
                },
            );
            c.text.draw(
                &l.message,
                10,
                r.x + r.w - 220.0,
                r.y + 17.0,
                [160, 160, 160, 255],
            );
            c.text.draw(
                &format!("{}", l.price * l.item.count as u64),
                12,
                r.x + r.w - 70.0,
                r.y + 9.0,
                [255, 220, 120, 255],
            );
            if hot && c.input.lmb_pressed {
                self.selected = Some(l.index);
                self.boxes[B_BUY_COUNT].text = "1".into();
            }
            y += 34.0;
        }
        if rows.is_empty() {
            c.text.draw(
                "Nothing matched. Try a wider filter.",
                12,
                win.x + 16.0,
                top + 44.0,
                [180, 180, 180, 255],
            );
        }

        // ---- Footer: paging and buy ----
        c.text.draw(
            &format!(
                "{} listings, page {}/{}",
                self.total,
                self.page + 1,
                self.pages()
            ),
            11,
            win.x + 12.0,
            fy + 5.0,
            [200, 200, 200, 255],
        );
        let pages = self.pages();
        let b = &mut self.buttons[2];
        b.pos = (win.x + 180.0, fy);
        b.enabled = self.page > 0;
        if b.update(c) && b.enabled {
            self.page -= 1;
            self.dirty = true;
        }
        let b = &mut self.buttons[3];
        b.pos = (win.x + 246.0, fy);
        b.enabled = self.page + 1 < pages;
        if b.update(c) && b.enabled {
            self.page += 1;
            self.dirty = true;
        }
        let picked = self
            .selected
            .and_then(|i| rows.iter().find(|l| l.index == i))
            .cloned();
        c.text
            .draw("Count", 11, win.x + 330.0, fy + 5.0, [220, 220, 220, 255]);
        self.boxes[B_BUY_COUNT].rect = Rect::new(win.x + 376.0, fy, 60.0, 22.0);
        self.boxes[B_BUY_COUNT].update(c);
        self.boxes[B_BUY_COUNT]
            .text
            .retain(|ch| ch.is_ascii_digit());
        let count: u32 = self.boxes[B_BUY_COUNT].text.parse().unwrap_or(0);
        let b = &mut self.buttons[1];
        b.pos = (win.x + 446.0, fy);
        // Zircon refuses your own listing, so the button is simply off.
        b.enabled = picked
            .as_ref()
            .is_some_and(|l| !l.is_owner && count > 0 && count <= l.item.count);
        if b.update(c) && b.enabled {
            if let Some(l) = &picked {
                out.push(ClientMessage::MarketBuy {
                    index: l.index,
                    count,
                });
                self.dirty = true;
            }
        }
        if let Some(l) = &picked {
            c.text.draw(
                &format!("Total {}", l.price * u64::from(count)),
                11,
                win.x + 530.0,
                fy + 5.0,
                [255, 220, 120, 255],
            );
        }
    }

    fn draw_consign(
        &mut self,
        c: &mut Ctx,
        catalog: &ItemCatalog,
        inventory: &[Option<ItemInstance>],
        win: Rect,
        fy: f32,
        out: &mut Vec<ClientMessage>,
    ) {
        let mouse = c.input.mouse;
        let top = win.y + 60.0;
        // ---- Left: the bag, to pick what to sell ----
        c.text
            .draw("Your bag", 12, win.x + 12.0, top, [255, 255, 200, 255]);
        let cols = 6;
        for (i, cell) in inventory.iter().enumerate().take(36) {
            let r = Rect::new(
                win.x + 12.0 + (i % cols) as f32 * 32.0,
                top + 18.0 + (i / cols) as f32 * 32.0,
                30.0,
                30.0,
            );
            let hot = r.contains(mouse.0, mouse.1);
            let on = self.consign_slot == Some(i as u8);
            crate::windows::draw_item_cell(c, catalog, r, cell.as_ref(), hot, on);
            if hot && c.input.lmb_pressed && cell.is_some() {
                self.consign_slot = Some(i as u8);
                self.boxes[B_COUNT].text =
                    cell.as_ref().map(|it| it.count).unwrap_or(1).to_string();
            }
        }

        // ---- Right: the listing form ----
        let rx = win.x + 12.0 + cols as f32 * 32.0 + 16.0;
        let chosen = self
            .consign_slot
            .and_then(|s| inventory.get(s as usize))
            .and_then(|c| c.as_ref())
            .cloned();
        let label = chosen
            .as_ref()
            .map(|it| format!("{} x{}", catalog.name(it.info), it.count))
            .unwrap_or_else(|| "Pick a bag item".into());
        c.text.draw(&label, 12, rx, top, [255, 255, 255, 255]);
        let fields = [
            ("Price each", B_PRICE),
            ("Count", B_COUNT),
            ("Note", B_NOTE),
        ];
        let mut by = top + 22.0;
        for (name, i) in fields {
            c.text.draw(name, 11, rx, by + 4.0, [220, 220, 220, 255]);
            let w = if i == B_NOTE { 230.0 } else { 100.0 };
            self.boxes[i].rect = Rect::new(rx + 74.0, by, w, 22.0);
            self.boxes[i].update(c);
            if i != B_NOTE {
                self.boxes[i].text.retain(|ch| ch.is_ascii_digit());
            }
            by += 28.0;
        }
        let price: u64 = self.boxes[B_PRICE].text.parse().unwrap_or(0);
        let count: u32 = self.boxes[B_COUNT].text.parse().unwrap_or(0);
        // Zircon takes its cut when the item sells, not when it is listed, so
        // show the seller what they would actually receive.
        let gross = price * u64::from(count);
        c.text.draw(
            &format!(
                "Sells for {gross}, you receive {} after {}% tax",
                gross - gross * 7 / 100,
                7
            ),
            11,
            rx,
            by + 2.0,
            [200, 200, 160, 255],
        );
        let b = &mut self.buttons[4];
        b.pos = (rx, by + 22.0);
        b.enabled = chosen
            .as_ref()
            .is_some_and(|it| price > 0 && count > 0 && count <= it.count);
        if b.update(c) && b.enabled {
            if let Some(slot) = self.consign_slot {
                out.push(ClientMessage::MarketConsign {
                    slot,
                    count,
                    price,
                    message: self.boxes[B_NOTE].text.clone(),
                });
                self.consign_slot = None;
                self.boxes[B_PRICE].text.clear();
                self.boxes[B_COUNT].text.clear();
            }
        }

        // ---- Below: what is already listed ----
        let listed = self.consignments.clone();
        c.text.draw(
            &format!("Listed by you ({})", listed.len()),
            12,
            win.x + 12.0,
            top + 210.0,
            [255, 255, 200, 255],
        );
        let mut y = top + 228.0;
        for l in listed.iter().take(4) {
            let r = Rect::new(win.x + 12.0, y, win.w - 24.0, 24.0);
            let hot = r.contains(mouse.0, mouse.1);
            let on = self.selected == Some(l.index);
            c.fill(
                r,
                if on {
                    [0.3, 0.3, 0.5, 0.85]
                } else if hot {
                    [0.22, 0.22, 0.3, 0.6]
                } else {
                    [0.0, 0.0, 0.0, 0.35]
                },
            );
            c.text.draw(
                &format!(
                    "{} x{}  at {} each",
                    catalog.name(l.item.info),
                    l.item.count,
                    l.price
                ),
                11,
                r.x + 6.0,
                r.y + 5.0,
                [255, 255, 255, 255],
            );
            if hot && c.input.lmb_pressed {
                self.selected = Some(l.index);
            }
            y += 26.0;
        }
        if listed.is_empty() {
            c.text.draw(
                "You have nothing listed.",
                11,
                win.x + 16.0,
                y + 2.0,
                [180, 180, 180, 255],
            );
        }
        let picked = self
            .selected
            .and_then(|i| listed.iter().find(|l| l.index == i))
            .cloned();
        let b = &mut self.buttons[5];
        b.pos = (win.x + 12.0, fy);
        b.enabled = picked.is_some();
        if b.update(c) && b.enabled {
            if let Some(l) = &picked {
                out.push(ClientMessage::MarketCancelConsign {
                    index: l.index,
                    count: l.item.count,
                });
                self.selected = None;
            }
        }
        c.text.draw(
            "Listing is free. Tax is taken from the sale. Safe zones only.",
            11,
            win.x + 104.0,
            fy + 5.0,
            [170, 170, 170, 255],
        );
    }
}

/// A click-to-cycle stand-in for Zircon's `DXComboBox`.
fn cycle(c: &mut Ctx, r: Rect, label: &str) -> bool {
    let hot = r.contains(c.input.mouse.0, c.input.mouse.1);
    c.fill(
        r,
        if hot {
            [0.25, 0.25, 0.25, 0.9]
        } else {
            [0.12, 0.12, 0.12, 0.85]
        },
    );
    c.border(r, [198, 166, 99, 255]);
    c.text
        .draw(label, 11, r.x + 6.0, r.y + 4.0, [235, 235, 235, 255]);
    hot && c.input.lmb_pressed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> MarketState {
        MarketState {
            total: 20,
            ..Default::default()
        }
    }

    #[test]
    fn pages_round_up_and_never_reach_zero() {
        let mut s = state();
        // 20 listings at 9 a page is three pages.
        assert_eq!(s.pages(), 3);
        s.total = 9;
        assert_eq!(s.pages(), 1);
        s.total = 0;
        assert_eq!(s.pages(), 1);
    }

    #[test]
    fn sort_and_filter_cycle_through_zircon_order() {
        let mut s = state();
        assert_eq!(s.sort(), MarketSort::Newest);
        s.sort = 2;
        assert_eq!(s.sort(), MarketSort::HighestPrice);
        // Out of range clamps rather than panicking.
        s.sort = 99;
        assert_eq!(s.sort(), MarketSort::LowestPrice);
        assert_eq!(TYPE_FILTERS[0].0, None);
        assert_eq!(TYPE_FILTERS[1].0, Some(item_type::WEAPON));
    }
}
