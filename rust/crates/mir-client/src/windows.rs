//! In-game windows: inventory, character (equipment), NPC dialog and shop.
//! Layouts follow Zircon's `InventoryDialog`, `CharacterDialog`,
//! `NPCDialog` and `NPCGoodsDialog`; the first two use the standard window
//! chrome because this asset set lacks their dedicated panel images.

use mir_proto::{
    item_type, parse_dialog, BeltLink, ClientMessage, DialogPart, Good, Grid, ItemInstance,
    EQUIPMENT_SIZE, INVENTORY_SIZE, MAX_BELT,
};

use crate::assets::lib;
use crate::gfx::Blend;
use crate::items::{stat_name, type_name, ItemCatalog, ItemDef};
use crate::menu::{self, MenuAction, MenuState};
use crate::ui::{Button, Ctx, Rect, TextBox, GOLD};
use mir_proto::ObjectId;

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
    pub quests: Vec<mir_proto::NpcQuest>,
    pub selected_good: Option<usize>,
    pub last_button: u64,
    /// First visible goods row (Zircon scrolls by pixels; we scroll by rows).
    pub goods_scroll: usize,
}

impl NpcDialog {
    pub fn new(
        npc: mir_proto::ObjectId,
        page: i32,
        say: &str,
        dialog_type: i32,
        goods: Vec<Good>,
        sell_types: Vec<u8>,
        quests: Vec<mir_proto::NpcQuest>,
    ) -> NpcDialog {
        NpcDialog {
            npc,
            page,
            parts: parse_dialog(say),
            dialog_type,
            goods,
            sell_types,
            quests,
            selected_good: None,
            last_button: 0,
            goods_scroll: 0,
        }
    }
}

/// Zircon `ItemInfo.ShouldLinkInfo`: stackables, consumables and scrolls
/// link by item type; everything else links the specific item.
pub fn link_for(
    catalog: &ItemCatalog,
    inventory: &[Option<ItemInstance>],
    inv_slot: u8,
    belt_slot: u8,
) -> BeltLink {
    let Some(item) = inventory.get(inv_slot as usize).and_then(|i| i.as_ref()) else {
        return BeltLink::empty(belt_slot);
    };
    let by_info = catalog
        .get(item.info)
        .map(|d| {
            d.stack_size > 1
                || d.item_type == item_type::CONSUMABLE
                || d.item_type == item_type::SCROLL
        })
        .unwrap_or(false);
    BeltLink {
        slot: belt_slot,
        info: if by_info { Some(item.info) } else { None },
        item: if by_info { None } else { Some(item.id) },
    }
}

/// The bag slot a belt link resolves to (first item of the type, or the
/// specific item), as Zircon's belt `UseItem` does.
pub fn belt_inventory_slot(inventory: &[Option<ItemInstance>], link: &BeltLink) -> Option<u8> {
    if let Some(info) = link.info {
        return inventory
            .iter()
            .position(|i| i.as_ref().map(|i| i.info == info).unwrap_or(false))
            .map(|p| p as u8);
    }
    let id = link.item?;
    inventory
        .iter()
        .position(|i| i.as_ref().map(|i| i.id == id).unwrap_or(false))
        .map(|p| p as u8)
}

/// What a belt cell shows: an item link shows that item, an info link shows
/// the type with the total count in the bag.
fn belt_cell_item(inventory: &[Option<ItemInstance>], link: &BeltLink) -> Option<ItemInstance> {
    if let Some(info) = link.info {
        let count: u32 = inventory
            .iter()
            .flatten()
            .filter(|i| i.info == info)
            .map(|i| i.count)
            .sum();
        return Some(ItemInstance {
            id: 0,
            info,
            count,
            durability: 0,
            max_durability: 0,
            added: Vec::new(),
        });
    }
    let id = link.item?;
    inventory.iter().flatten().find(|i| i.id == id).cloned()
}

pub struct WindowState {
    pub inventory_open: bool,
    pub character_open: bool,
    pub skills_open: bool,
    pub quests_open: bool,
    pub quest_scroll: f32,
    /// Storage window (B): account storage, usable in safe zones.
    pub storage_open: bool,
    trade_gold: Option<TextBox>,
    trade_buttons: Vec<Button>,
    request_buttons: Vec<Button>,
    marriage_buttons: Vec<Button>,
    /// Mail window (M): mailbox on the left, the selected mail or the
    /// compose form on the right.
    pub mail_open: bool,
    /// Refine page draft: type, quality and the chosen cells.
    refine_type: u8,
    refine_quality: u8,
    refine_ores: Vec<(Grid, u8, u32)>,
    refine_items: Vec<(Grid, u8, u32)>,
    refine_specials: Vec<(Grid, u8, u32)>,
    refine_buttons: Vec<Button>,
    retrieve_buttons: Vec<Button>,
    /// Companion page: name box, one Adopt/Unlock per offer, three per
    /// owned companion; companion window (N) with Take buttons.
    pub companion_open: bool,
    companion_name: Option<TextBox>,
    companion_offer_buttons: Vec<Button>,
    companion_own_buttons: Vec<Button>,
    companion_take: Vec<Button>,
    pub mail_selected: Option<u32>,
    mail_compose: bool,
    mail_boxes: Vec<TextBox>,
    mail_buttons: Vec<Button>,
    mail_take: Vec<Button>,
    /// Attachments for the mail being composed: (grid, slot, count).
    pub mail_attach: Vec<(Grid, u8, u32)>,
    /// Guild window (G): create, members, notice, invite, kick, leave.
    pub guild_open: bool,
    guild_name: Option<TextBox>,
    guild_notice: Option<TextBox>,
    guild_buttons: Vec<Button>,
    guild_kick: Vec<Button>,
    guild_invite_buttons: Vec<Button>,
    /// Group window (P): members, invite box, allow toggle.
    pub group_open: bool,
    group_name: Option<TextBox>,
    group_buttons: Vec<Button>,
    group_kick: Vec<Button>,
    invite_buttons: Vec<Button>,
    /// The belt is shown by default (Zircon `BeltDialog`, toggled with Z).
    pub belt_open: bool,
    /// Bag slot under the mouse this frame (for belt binding with digit keys).
    pub hover_inventory: Option<u8>,
    revive_button: Option<Button>,
    pub skill_scroll: f32,
    /// Magic whose icon is under the mouse in the skill window (for key binding).
    pub hover_magic: Option<u16>,
    pub npc: Option<NpcDialog>,
    /// Slot picked up with the mouse, waiting for a destination.
    pub carrying: Option<(Grid, u8)>,
    pub tooltip: Option<(i32, f32, f32)>,
    buy_button: Option<Button>,
    /// HUD menu buttons, help, exit, currency, auto potion, drop filter.
    pub menu: MenuState,
    close_all_hint: bool,
    auto_button_done: bool,
    magic_tip: Option<(u16, f32, f32)>,
}

impl WindowState {
    /// True while a window text box has keyboard focus.
    pub fn typing(&self) -> bool {
        (self.group_open && self.group_name.as_ref().is_some_and(|b| b.focused))
            || (self.guild_open
                && (self.guild_name.as_ref().is_some_and(|b| b.focused)
                    || self.guild_notice.as_ref().is_some_and(|b| b.focused)))
            || self.trade_gold.as_ref().is_some_and(|b| b.focused)
            || (self.mail_open && self.mail_boxes.iter().any(|b| b.focused))
            || (self.npc.is_some() && self.companion_name.as_ref().is_some_and(|b| b.focused))
            || self.menu.typing()
    }
}

impl Default for WindowState {
    fn default() -> WindowState {
        WindowState {
            inventory_open: false,
            character_open: false,
            skills_open: false,
            quests_open: false,
            storage_open: false,
            trade_gold: None,
            trade_buttons: Vec::new(),
            request_buttons: Vec::new(),
            marriage_buttons: Vec::new(),
            mail_open: false,
            refine_type: 2,
            refine_quality: 2,
            refine_ores: Vec::new(),
            refine_items: Vec::new(),
            refine_specials: Vec::new(),
            refine_buttons: Vec::new(),
            retrieve_buttons: Vec::new(),
            companion_open: false,
            companion_name: None,
            companion_offer_buttons: Vec::new(),
            companion_own_buttons: Vec::new(),
            companion_take: Vec::new(),
            mail_selected: None,
            mail_compose: false,
            mail_boxes: Vec::new(),
            mail_buttons: Vec::new(),
            mail_take: Vec::new(),
            mail_attach: Vec::new(),
            guild_open: false,
            guild_name: None,
            guild_notice: None,
            guild_buttons: Vec::new(),
            guild_kick: Vec::new(),
            guild_invite_buttons: Vec::new(),
            group_open: false,
            group_name: None,
            group_buttons: Vec::new(),
            group_kick: Vec::new(),
            invite_buttons: Vec::new(),
            quest_scroll: 0.0,
            belt_open: true,
            hover_inventory: None,
            revive_button: None,
            skill_scroll: 0.0,
            hover_magic: None,
            npc: None,
            carrying: None,
            tooltip: None,
            buy_button: None,
            menu: MenuState::default(),
            close_all_hint: false,
            auto_button_done: false,
            magic_tip: None,
        }
    }
}

pub struct Bag<'a> {
    pub inventory: &'a [Option<ItemInstance>],
    pub equipment: &'a [Option<ItemInstance>],
    pub gold: u64,
    pub weights: &'a mir_proto::Weights,
    pub stats: &'a PlayerView,
    pub catalog: &'a ItemCatalog,
    pub magics: &'a [mir_proto::MagicSummary],
    pub toggles: &'a std::collections::HashSet<u16>,
    pub cooldowns: &'a std::collections::HashMap<u16, u64>,
    pub belt: &'a [BeltLink],
    pub use_item_time: u64,
    pub dead: bool,
    pub quests: &'a [mir_proto::UserQuestSummary],
    pub player_name: &'a str,
    pub group: &'a [(ObjectId, String)],
    pub allow_group: bool,
    pub group_invite: Option<&'a str>,
    pub user: Option<ObjectId>,
    pub storage: &'a [Option<ItemInstance>],
    pub trade: Option<&'a TradeState>,
    pub trade_request: Option<&'a str>,
    pub guild: Option<&'a mir_proto::GuildSummary>,
    pub guild_invite: Option<(&'a str, &'a str)>,
    pub mail: &'a [mir_proto::MailSummary],
    pub partner: Option<&'a str>,
    pub wedding_ring: Option<u32>,
    pub marriage_invite: Option<&'a str>,
    pub refines: &'a [mir_proto::RefineSummary],
    pub companions: &'a [mir_proto::CompanionSummary],
    pub companion_shop: &'a [mir_proto::CompanionOffer],
    pub currencies: &'a [mir_proto::CurrencySummary],
    /// Castles as (index, name, owner) and the one under conquest.
    pub castles: &'a [(i32, String, String)],
    pub conquest: Option<i32>,
}

/// An open trade as the client sees it.
pub struct TradeState {
    pub partner: String,
    /// My offered cells: (grid, slot, count).
    pub my_items: Vec<(Grid, u8, u32)>,
    pub my_gold: u64,
    pub their_items: Vec<ItemInstance>,
    pub their_gold: u64,
    /// I pressed Confirm and the server has not unlocked it since.
    pub confirmed: bool,
}

/// The few player facts windows need.
pub struct PlayerView {
    pub level: u8,
    pub class: u8,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub min_dc: i32,
    pub max_dc: i32,
    pub min_ac: i32,
    pub max_ac: i32,
    pub accuracy: i32,
    pub agility: i32,
    /// Held fame title ("" when none).
    pub fame_title: String,
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
        for (k, v) in &i.added {
            if *k == 52 {
                let element = [
                    "",
                    "Fire",
                    "Ice",
                    "Lightning",
                    "Wind",
                    "Holy",
                    "Dark",
                    "Phantom",
                ];
                lines.push((
                    format!("Element: {}", element.get(*v as usize).unwrap_or(&"?")),
                    [255, 180, 80, 255],
                ));
            } else if let Some(n) = stat_name(*k) {
                lines.push((format!("{n} +{v} (refined)"), [255, 180, 80, 255]));
            }
        }
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
        self.hover_inventory = None;
        let _ = height;

        // ---- Quest log (L) ----
        if self.quests_open {
            let win = Rect::new(width as f32 - 380.0 - 10.0, 30.0, 380.0, 460.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
                self.quest_scroll = (self.quest_scroll - c.input.wheel * 18.0).max(0.0);
            }
            let closed = c.window(win, "Quests", false);
            // (line, colour, quest this line belongs to when it is a title)
            let mut rows: Vec<(String, [u8; 4], Option<i32>)> = Vec::new();
            if bag.quests.is_empty() {
                rows.push((
                    "No quests. Talk to NPCs to find some.".into(),
                    [200, 200, 200, 255],
                    None,
                ));
            }
            let mut sorted: Vec<&mir_proto::UserQuestSummary> = bag.quests.iter().collect();
            sorted.sort_by_key(|q| (q.completed, q.quest));
            for q in sorted {
                let Some(def) = bag.catalog.quest(q.quest) else {
                    continue;
                };
                let title_col = if q.completed {
                    [120, 120, 120, 255]
                } else {
                    [255, 255, 0, 255]
                };
                rows.push((
                    format!(
                        "{} {}{}",
                        if q.track { "[x]" } else { "[ ]" },
                        def.name,
                        if q.completed { " (done)" } else { "" }
                    ),
                    title_col,
                    Some(q.quest),
                ));
                let text = if q.completed {
                    bag.catalog
                        .quest_text(def, &def.completed_text, bag.player_name)
                } else {
                    bag.catalog
                        .quest_text(def, &def.progress_text, bag.player_name)
                };
                for line in wrap_text(c, &text, 340.0, 12) {
                    rows.push((line, [220, 220, 220, 255], None));
                }
                if !q.completed {
                    for (line, col) in crate::overlay::quest_task_lines(def, q, bag.catalog) {
                        rows.push((line, col, None));
                    }
                    let rewards: Vec<String> = def
                        .rewards
                        .iter()
                        .filter(|r| r.class & (1 << bag.stats.class) != 0)
                        .map(|r| {
                            format!(
                                "{}{} x{}",
                                if r.choice { "(choice) " } else { "" },
                                bag.catalog.name(r.item),
                                r.amount
                            )
                        })
                        .collect();
                    if !rewards.is_empty() {
                        rows.push((
                            format!("  Reward: {}", rewards.join(", ")),
                            [160, 200, 255, 255],
                            None,
                        ));
                    }
                }
                rows.push((String::new(), [0, 0, 0, 0], None));
            }
            let max_scroll = (rows.len() as f32 * 18.0 - 400.0).max(0.0);
            self.quest_scroll = self.quest_scroll.min(max_scroll);
            let top = win.y + 40.0;
            c.text.draw(
                "Click a quest to track it.",
                10,
                win.x + 20.0,
                win.y + win.h - 24.0,
                [160, 160, 160, 255],
            );
            for (i, (line, col, quest)) in rows.iter().enumerate() {
                let y = top + i as f32 * 18.0 - self.quest_scroll;
                if y < top - 1.0 || y + 18.0 > win.y + win.h - 10.0 {
                    continue;
                }
                if line.is_empty() {
                    continue;
                }
                // Titles toggle tracking (Zircon `ClientUserQuest.Track`).
                if let Some(id) = quest {
                    let row = Rect::new(win.x + 16.0, y - 1.0, win.w - 32.0, 18.0);
                    if row.contains(mouse.0, mouse.1) {
                        c.fill(row, [0.25, 0.25, 0.35, 0.6]);
                        if c.input.lmb_pressed {
                            let track = bag
                                .quests
                                .iter()
                                .find(|q| q.quest == *id)
                                .map(|q| !q.track)
                                .unwrap_or(true);
                            out.push(ClientMessage::QuestTrack { quest: *id, track });
                        }
                    }
                }
                c.text.draw(line, 12, win.x + 20.0, y, *col);
            }
            if closed {
                self.quests_open = false;
            }
        }

        // ---- Storage (B): 10 columns, account storage ----
        if self.storage_open {
            let cols = 10;
            let rows = bag.storage.len().div_ceil(cols).max(1);
            let win = Rect::new(
                10.0,
                40.0,
                20.0 + cols as f32 * PITCH + 20.0,
                39.0 + rows as f32 * PITCH + 50.0,
            );
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            let closed = c.window(win, "Storage", true);
            for i in 0..bag.storage.len() {
                let (col, row) = (i % cols, i / cols);
                let r = Rect::new(
                    win.x + 20.0 + col as f32 * PITCH,
                    win.y + 39.0 + row as f32 * PITCH,
                    CELL,
                    CELL,
                );
                let hover = r.contains(mouse.0, mouse.1);
                let item = bag.storage[i].as_ref();
                let selected = self.carrying == Some((Grid::Storage, i as u8));
                draw_item_cell(c, bag.catalog, r, item, hover, selected);
                if hover {
                    if let Some(it) = item {
                        self.tooltip = Some((it.info, mouse.0, mouse.1));
                    }
                    if c.input.lmb_pressed {
                        self.click_slot(Grid::Storage, i as u8, item.is_some(), out);
                    }
                }
            }
            c.text.draw(
                "Safe zones only. Pick up a bag item and click a slot to store it.",
                10,
                win.x + 20.0,
                win.y + win.h - 3.0 - 42.0 + 12.0,
                [160, 160, 160, 255],
            );
            if closed {
                self.storage_open = false;
                self.carrying = None;
            }
        }

        // ---- Trade window ----
        if let Some(t) = bag.trade {
            let win = Rect::new(width as f32 / 2.0 - 200.0, 60.0, 400.0, 300.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            c.window(win, &format!("Trade with {}", t.partner), true);
            let col_w = 180.0;
            c.text.draw(
                "You offer (right-click bag items):",
                11,
                win.x + 16.0,
                win.y + 36.0,
                [255, 255, 0, 255],
            );
            for (i, (grid, slot, count)) in t.my_items.iter().enumerate() {
                let cells = match grid {
                    Grid::Inventory => bag.inventory,
                    Grid::Equipment => bag.equipment,
                    Grid::Storage => bag.storage,
                };
                let name = cells
                    .get(*slot as usize)
                    .and_then(|c| c.as_ref())
                    .map(|it| bag.catalog.name(it.info))
                    .unwrap_or_else(|| "?".into());
                c.text.draw(
                    &format!("{name} x{count}"),
                    11,
                    win.x + 16.0,
                    win.y + 54.0 + i as f32 * 15.0,
                    [255, 255, 255, 255],
                );
            }
            c.text.draw(
                &format!("{} offers:", t.partner),
                11,
                win.x + 16.0 + col_w + 20.0,
                win.y + 36.0,
                [255, 255, 0, 255],
            );
            for (i, it) in t.their_items.iter().enumerate() {
                c.text.draw(
                    &format!("{} x{}", bag.catalog.name(it.info), it.count),
                    11,
                    win.x + 16.0 + col_w + 20.0,
                    win.y + 54.0 + i as f32 * 15.0,
                    [255, 255, 255, 255],
                );
            }
            let gy = win.y + win.h - 3.0 - 42.0 - 56.0;
            c.text.draw(
                &format!("Gold: {}", t.my_gold),
                12,
                win.x + 16.0,
                gy,
                [218, 165, 32, 255],
            );
            c.text.draw(
                &format!("Gold: {}", t.their_gold),
                12,
                win.x + 16.0 + col_w + 20.0,
                gy,
                [218, 165, 32, 255],
            );
            let gold_box = self
                .trade_gold
                .get_or_insert_with(|| TextBox::new(Rect::new(0.0, 0.0, 100.0, 22.0), 12));
            gold_box.rect = Rect::new(win.x + 16.0, gy + 22.0, 100.0, 22.0);
            let submitted = gold_box.update(c);
            gold_box.text.retain(|ch| ch.is_ascii_digit());
            if self.trade_buttons.is_empty() {
                self.trade_buttons = vec![
                    Button::default_style(0.0, 0.0, 70.0, "Set gold"),
                    Button::default_style(0.0, 0.0, 80.0, "Confirm"),
                    Button::default_style(0.0, 0.0, 80.0, "Cancel"),
                ];
            }
            let gold: u64 = gold_box.text.parse().unwrap_or(0);
            let b = &mut self.trade_buttons[0];
            b.pos = (win.x + 122.0, gy + 20.0);
            b.enabled = gold > t.my_gold;
            if (b.update(c) || submitted) && b.enabled {
                out.push(ClientMessage::TradeAddGold { gold });
            }
            let fy = win.y + win.h - 3.0 - 42.0 + 8.0;
            let b = &mut self.trade_buttons[1];
            b.pos = (win.x + 110.0, fy);
            b.enabled = !t.confirmed;
            b.label = Some(if t.confirmed {
                "Waiting...".into()
            } else {
                "Confirm".into()
            });
            if b.update(c) {
                out.push(ClientMessage::TradeConfirm);
            }
            let b = &mut self.trade_buttons[2];
            b.pos = (win.x + 210.0, fy);
            if b.update(c) {
                out.push(ClientMessage::TradeClose);
            }
        } else if let Some(b) = &mut self.trade_gold {
            b.text.clear();
            b.focused = false;
        }

        // ---- Trade request prompt ----
        if let Some(from) = bag.trade_request {
            let win = Rect::new(width as f32 / 2.0 - 150.0, 240.0, 300.0, 100.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            c.window(win, "Trade request", false);
            c.text.draw(
                &format!("{from} wants to trade with you."),
                12,
                win.x + 16.0,
                win.y + 40.0,
                [255, 255, 255, 255],
            );
            if self.request_buttons.is_empty() {
                self.request_buttons = vec![
                    Button::default_style(0.0, 0.0, 80.0, "Accept"),
                    Button::default_style(0.0, 0.0, 80.0, "Decline"),
                ];
            }
            let b = &mut self.request_buttons[0];
            b.pos = (win.x + 60.0, win.y + 66.0);
            if b.update(c) {
                out.push(ClientMessage::TradeResponse { accept: true });
            }
            let b = &mut self.request_buttons[1];
            b.pos = (win.x + 160.0, win.y + 66.0);
            if b.update(c) {
                out.push(ClientMessage::TradeResponse { accept: false });
            }
        }

        // ---- Marriage proposal prompt ----
        if let Some(from) = bag.marriage_invite {
            let win = Rect::new(width as f32 / 2.0 - 160.0, 480.0, 320.0, 100.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            c.window(win, "Proposal", false);
            c.text.draw(
                &format!("{from} asks you to marry them (500,000 gold each)."),
                11,
                win.x + 16.0,
                win.y + 40.0,
                [255, 255, 255, 255],
            );
            if self.marriage_buttons.is_empty() {
                self.marriage_buttons = vec![
                    Button::default_style(0.0, 0.0, 80.0, "Accept"),
                    Button::default_style(0.0, 0.0, 80.0, "Decline"),
                    Button::default_style(0.0, 0.0, 120.0, "To partner"),
                ];
            }
            let b = &mut self.marriage_buttons[0];
            b.pos = (win.x + 70.0, win.y + 66.0);
            if b.update(c) {
                out.push(ClientMessage::MarriageResponse { accept: true });
            }
            let b = &mut self.marriage_buttons[1];
            b.pos = (win.x + 170.0, win.y + 66.0);
            if b.update(c) {
                out.push(ClientMessage::MarriageResponse { accept: false });
            }
        }

        // ---- Mail (M) ----
        if self.mail_open {
            let win = Rect::new(width as f32 / 2.0 - 260.0, 40.0, 520.0, 360.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            let closed = c.window(win, "Mail", true);
            if self.mail_buttons.is_empty() {
                self.mail_buttons = vec![
                    Button::default_style(0.0, 0.0, 80.0, "Compose"),
                    Button::default_style(0.0, 0.0, 70.0, "Delete"),
                    Button::default_style(0.0, 0.0, 70.0, "Send"),
                    Button::default_style(0.0, 0.0, 90.0, "Take gold"),
                ];
            }
            if self.mail_boxes.is_empty() {
                self.mail_boxes = vec![
                    TextBox::new(Rect::new(0.0, 0.0, 200.0, 22.0), 20),
                    TextBox::new(Rect::new(0.0, 0.0, 200.0, 22.0), 30),
                    TextBox::new(Rect::new(0.0, 0.0, 300.0, 22.0), 300),
                    TextBox::new(Rect::new(0.0, 0.0, 100.0, 22.0), 12),
                ];
            }
            // Left: the mailbox.
            let list_w = 190.0;
            c.text.draw(
                &format!("Mailbox ({})", bag.mail.len()),
                12,
                win.x + 14.0,
                win.y + 36.0,
                [255, 255, 200, 255],
            );
            let mut y = win.y + 56.0;
            for m in bag.mail.iter().rev().take(14) {
                let r = Rect::new(win.x + 12.0, y - 2.0, list_w, 18.0);
                let hover = r.contains(mouse.0, mouse.1);
                let selected = self.mail_selected == Some(m.index);
                if selected {
                    c.fill(r, [0.3, 0.3, 0.5, 0.8]);
                } else if hover {
                    c.fill(r, [0.25, 0.25, 0.35, 0.6]);
                }
                let col = if m.opened {
                    [200, 200, 200, 255]
                } else {
                    [255, 255, 0, 255]
                };
                let mut label = format!("{}: {}", m.sender, m.subject);
                if !m.items.is_empty() || m.gold > 0 {
                    label.push_str(" *");
                }
                c.text.draw(&label, 11, win.x + 16.0, y, col);
                if hover && c.input.lmb_pressed {
                    self.mail_selected = Some(m.index);
                    self.mail_compose = false;
                    if !m.opened {
                        out.push(ClientMessage::MailOpened { index: m.index });
                    }
                }
                y += 18.0;
            }
            let fy = win.y + win.h - 3.0 - 42.0 + 8.0;
            let b = &mut self.mail_buttons[0];
            b.pos = (win.x + 12.0, fy);
            b.label = Some(if self.mail_compose {
                "Back".into()
            } else {
                "Compose".into()
            });
            if b.update(c) {
                self.mail_compose = !self.mail_compose;
                if self.mail_compose {
                    self.mail_selected = None;
                }
            }
            // Right: the selected mail, or the compose form.
            let rx = win.x + list_w + 30.0;
            let rw = win.w - list_w - 44.0;
            if self.mail_compose {
                let labels = ["To:", "Subject:", "Message:", "Gold:"];
                let mut by = win.y + 40.0;
                for (i, label) in labels.iter().enumerate() {
                    c.text.draw(label, 11, rx, by + 3.0, [220, 220, 220, 255]);
                    let bw = if i == 2 { rw - 70.0 } else { 160.0 };
                    let bx = self.mail_boxes.get_mut(i).unwrap();
                    bx.rect = Rect::new(rx + 64.0, by, bw, 22.0);
                    bx.update(c);
                    if i == 3 {
                        bx.text.retain(|ch| ch.is_ascii_digit());
                    }
                    by += 28.0;
                }
                c.text.draw(
                    "Attachments (right-click bag items, 5 max):",
                    10,
                    rx,
                    by + 4.0,
                    [160, 160, 160, 255],
                );
                by += 18.0;
                let mut remove = None;
                for (i, (grid, slot, count)) in self.mail_attach.iter().enumerate() {
                    let cells = match grid {
                        Grid::Inventory => bag.inventory,
                        Grid::Equipment => bag.equipment,
                        Grid::Storage => bag.storage,
                    };
                    let name = cells
                        .get(*slot as usize)
                        .and_then(|c| c.as_ref())
                        .map(|it| bag.catalog.name(it.info))
                        .unwrap_or_else(|| "?".into());
                    let r = Rect::new(rx, by, rw, 16.0);
                    let hover = r.contains(mouse.0, mouse.1);
                    c.text.draw(
                        &format!("{name} x{count}  (click to remove)"),
                        11,
                        rx,
                        by,
                        if hover {
                            [255, 150, 150, 255]
                        } else {
                            [255, 255, 255, 255]
                        },
                    );
                    if hover && c.input.lmb_pressed {
                        remove = Some(i);
                    }
                    by += 16.0;
                }
                if let Some(i) = remove {
                    self.mail_attach.remove(i);
                }
                let recipient = self.mail_boxes[0].text.trim().to_string();
                let b = &mut self.mail_buttons[2];
                b.pos = (rx, fy);
                b.enabled = !recipient.is_empty();
                if b.update(c) && b.enabled {
                    out.push(ClientMessage::MailSend {
                        recipient,
                        subject: self.mail_boxes[1].text.clone(),
                        message: self.mail_boxes[2].text.clone(),
                        gold: self.mail_boxes[3].text.parse().unwrap_or(0),
                        items: self.mail_attach.clone(),
                    });
                    for bx in &mut self.mail_boxes {
                        bx.text.clear();
                    }
                    self.mail_attach.clear();
                    self.mail_compose = false;
                }
            } else if let Some(m) = self
                .mail_selected
                .and_then(|i| bag.mail.iter().find(|m| m.index == i))
            {
                c.text.draw(
                    &format!("From {}: {}", m.sender, m.subject),
                    12,
                    rx,
                    win.y + 40.0,
                    [255, 255, 0, 255],
                );
                let mut by = win.y + 60.0;
                for line in wrap_text(c, &m.message, rw, 11).into_iter().take(8) {
                    c.text.draw(&line, 11, rx, by, [220, 220, 220, 255]);
                    by += 14.0;
                }
                by += 8.0;
                if self.mail_take.len() != m.items.len() {
                    self.mail_take = m
                        .items
                        .iter()
                        .map(|_| Button::default_style(0.0, 0.0, 50.0, "Take"))
                        .collect();
                }
                for (i, it) in m.items.iter().enumerate() {
                    c.text.draw(
                        &format!("{} x{}", bag.catalog.name(it.info), it.count),
                        11,
                        rx,
                        by + 2.0,
                        [255, 255, 255, 255],
                    );
                    let b = &mut self.mail_take[i];
                    b.pos = (rx + rw - 56.0, by - 2.0);
                    if b.update(c) {
                        out.push(ClientMessage::MailGetItem {
                            index: m.index,
                            slot: i as u8,
                        });
                    }
                    by += 22.0;
                }
                if m.gold > 0 {
                    c.text.draw(
                        &format!("{} gold", m.gold),
                        11,
                        rx,
                        by + 2.0,
                        [218, 165, 32, 255],
                    );
                    let b = &mut self.mail_buttons[3];
                    b.pos = (rx + rw - 96.0, by - 2.0);
                    if b.update(c) {
                        out.push(ClientMessage::MailGetItem {
                            index: m.index,
                            slot: 255,
                        });
                    }
                }
                let b = &mut self.mail_buttons[1];
                b.pos = (rx, fy);
                b.enabled = m.items.is_empty() && m.gold == 0;
                if b.update(c) && b.enabled {
                    out.push(ClientMessage::MailDelete { index: m.index });
                }
            } else {
                c.text.draw(
                    "Select a mail, or Compose a new one.",
                    11,
                    rx,
                    win.y + 40.0,
                    [160, 160, 160, 255],
                );
            }
            if closed {
                self.mail_open = false;
            }
        }

        // ---- Guild (G) ----
        if self.guild_open {
            let win = Rect::new(width as f32 - 340.0 - 10.0, 30.0, 340.0, 400.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            let title = bag
                .guild
                .map(|g| format!("Guild: {}", g.name))
                .unwrap_or_else(|| "Guild".into());
            let closed = c.window(win, &title, true);
            let name_box = self
                .guild_name
                .get_or_insert_with(|| TextBox::new(Rect::new(0.0, 0.0, 150.0, 22.0), 15));
            if self.guild_buttons.is_empty() {
                self.guild_buttons = vec![
                    Button::default_style(0.0, 0.0, 70.0, "Create"),
                    Button::default_style(0.0, 0.0, 60.0, "Invite"),
                    Button::default_style(0.0, 0.0, 60.0, "Leave"),
                    Button::default_style(0.0, 0.0, 80.0, "Set notice"),
                ];
            }
            let fy = win.y + win.h - 3.0 - 42.0 + 8.0;
            match bag.guild {
                None => {
                    c.text.draw(
                        "You are not in a guild. Founding one costs",
                        12,
                        win.x + 16.0,
                        win.y + 40.0,
                        [220, 220, 220, 255],
                    );
                    c.text.draw(
                        "7,500,000 gold plus 1,000,000 per member slot (10 here).",
                        12,
                        win.x + 16.0,
                        win.y + 58.0,
                        [220, 220, 220, 255],
                    );
                    name_box.rect = Rect::new(win.x + 16.0, fy + 2.0, 150.0, 22.0);
                    let submitted = name_box.update(c);
                    let name = name_box.text.trim().to_string();
                    let b = &mut self.guild_buttons[0];
                    b.pos = (win.x + 176.0, fy);
                    b.enabled = !name.is_empty();
                    if (b.update(c) || submitted) && b.enabled {
                        out.push(ClientMessage::GuildCreate { name, members: 10 });
                    }
                }
                Some(g) => {
                    let me = g.members.iter().find(|m| m.index == g.user_index);
                    let leader = me.is_some_and(|m| m.permission == -1);
                    let can_invite =
                        me.is_some_and(|m| m.permission == -1 || m.permission & 2 != 0);
                    let can_notice =
                        me.is_some_and(|m| m.permission == -1 || m.permission & 1 != 0);
                    c.text.draw(
                        &format!(
                            "Members {}/{}   Funds {}   Tax {}%",
                            g.members.len(),
                            g.member_limit,
                            g.funds,
                            g.tax
                        ),
                        11,
                        win.x + 16.0,
                        win.y + 38.0,
                        [255, 255, 200, 255],
                    );
                    let mut y = win.y + 56.0;
                    for line in wrap_text(c, &g.notice, 300.0, 11).into_iter().take(2) {
                        c.text
                            .draw(&line, 11, win.x + 16.0, y, [200, 200, 255, 255]);
                        y += 14.0;
                    }
                    // Castle and wars (Zircon GuildDialog war tab).
                    let mut status = String::new();
                    if !g.castle.is_empty() {
                        status.push_str(&format!("Holds {}. ", g.castle));
                    }
                    for (enemy, secs) in &g.wars {
                        status.push_str(&format!("War with {enemy} ({}m). ", secs / 60));
                    }
                    for (index, name, owner) in bag.castles {
                        let state = if bag.conquest == Some(*index) {
                            "under siege"
                        } else if owner.is_empty() {
                            "unclaimed"
                        } else {
                            owner.as_str()
                        };
                        status.push_str(&format!("{name}: {state}. "));
                    }
                    if !status.is_empty() {
                        c.text
                            .draw(&status, 11, win.x + 16.0, y, [255, 200, 120, 255]);
                    }
                    y = win.y + 104.0;
                    if self.guild_kick.len() != g.members.len() {
                        self.guild_kick = g
                            .members
                            .iter()
                            .map(|_| Button::default_style(0.0, 0.0, 50.0, "Kick"))
                            .collect();
                    }
                    for (i, m) in g.members.iter().enumerate().take(9) {
                        let col = if m.index == g.user_index {
                            [255, 255, 0, 255]
                        } else if m.online {
                            [255, 255, 255, 255]
                        } else {
                            [140, 140, 140, 255]
                        };
                        c.text.draw(
                            &format!("{} - {}", m.name, m.rank),
                            11,
                            win.x + 16.0,
                            y,
                            col,
                        );
                        if leader && m.index != g.user_index {
                            let b = &mut self.guild_kick[i];
                            b.pos = (win.x + win.w - 66.0, y - 2.0);
                            if b.update(c) {
                                out.push(ClientMessage::GuildKickMember { index: m.index });
                            }
                        }
                        y += 18.0;
                    }
                    // War on the named guild, and a conquest request for
                    // the first castle (StartWar / leader permissions).
                    let can_war = me.is_some_and(|m| m.permission == -1 || m.permission & 128 != 0);
                    let wy = win.y + win.h - 3.0 - 42.0 - 60.0;
                    if self.guild_buttons.len() < 8 {
                        self.guild_buttons.push(Button::default_style(
                            0.0,
                            0.0,
                            90.0,
                            "Declare war",
                        ));
                        self.guild_buttons.push(Button::default_style(
                            0.0,
                            0.0,
                            120.0,
                            "Request conquest",
                        ));
                        self.guild_buttons
                            .push(Button::default_style(0.0, 0.0, 60.0, "Gates"));
                        self.guild_buttons
                            .push(Button::default_style(0.0, 0.0, 70.0, "Repair"));
                    }
                    if !g.castle.is_empty() {
                        let b = &mut self.guild_buttons[6];
                        b.pos = (win.x + 16.0, wy - 26.0);
                        if b.update(c) {
                            out.push(ClientMessage::GuildToggleCastleGates);
                        }
                        let b = &mut self.guild_buttons[7];
                        b.pos = (win.x + 82.0, wy - 26.0);
                        b.enabled = leader;
                        if b.update(c) && b.enabled {
                            out.push(ClientMessage::GuildRepairCastleGates);
                        }
                    }
                    let name_now = name_box.text.trim().to_string();
                    let b = &mut self.guild_buttons[4];
                    b.pos = (win.x + 16.0, wy);
                    b.enabled = can_war && !name_now.is_empty();
                    if b.update(c) && b.enabled {
                        out.push(ClientMessage::GuildWar {
                            name: name_now.clone(),
                        });
                    }
                    let b = &mut self.guild_buttons[5];
                    b.pos = (win.x + 112.0, wy);
                    b.enabled = leader
                        && g.castle.is_empty()
                        && !bag.castles.is_empty()
                        && bag.conquest.is_none();
                    if b.update(c) && b.enabled {
                        if let Some((index, _, _)) = bag.castles.first() {
                            out.push(ClientMessage::GuildRequestConquest { index: *index });
                        }
                    }
                    // Footer: invite box, Leave, and the notice box above.
                    let notice_box = self
                        .guild_notice
                        .get_or_insert_with(|| TextBox::new(Rect::new(0.0, 0.0, 220.0, 22.0), 200));
                    let ny = win.y + win.h - 3.0 - 42.0 - 30.0;
                    notice_box.rect = Rect::new(win.x + 16.0, ny, 220.0, 22.0);
                    let notice_submitted = notice_box.update(c);
                    let b = &mut self.guild_buttons[3];
                    b.pos = (win.x + 244.0, ny - 2.0);
                    b.enabled = can_notice;
                    if (b.update(c) || notice_submitted) && b.enabled {
                        out.push(ClientMessage::GuildEditNotice {
                            notice: notice_box.text.clone(),
                        });
                    }
                    name_box.rect = Rect::new(win.x + 16.0, fy + 2.0, 150.0, 22.0);
                    let submitted = name_box.update(c);
                    let name = name_box.text.trim().to_string();
                    let b = &mut self.guild_buttons[1];
                    b.pos = (win.x + 176.0, fy);
                    b.enabled = can_invite && !name.is_empty();
                    if (b.update(c) || submitted) && b.enabled {
                        out.push(ClientMessage::GuildInviteMember { name });
                        if let Some(nb) = &mut self.guild_name {
                            nb.text.clear();
                        }
                    }
                    let b = &mut self.guild_buttons[2];
                    b.pos = (win.x + 244.0, fy);
                    b.enabled = true;
                    if b.update(c) {
                        out.push(ClientMessage::GuildLeave);
                    }
                }
            }
            if closed {
                self.guild_open = false;
            }
        }

        // ---- Guild invite prompt ----
        if let Some((from, guild)) = bag.guild_invite {
            let win = Rect::new(width as f32 / 2.0 - 160.0, 360.0, 320.0, 100.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            c.window(win, "Guild invite", false);
            c.text.draw(
                &format!("{from} invites you to join {guild}."),
                12,
                win.x + 16.0,
                win.y + 40.0,
                [255, 255, 255, 255],
            );
            if self.guild_invite_buttons.is_empty() {
                self.guild_invite_buttons = vec![
                    Button::default_style(0.0, 0.0, 80.0, "Accept"),
                    Button::default_style(0.0, 0.0, 80.0, "Decline"),
                ];
            }
            let b = &mut self.guild_invite_buttons[0];
            b.pos = (win.x + 70.0, win.y + 66.0);
            if b.update(c) {
                out.push(ClientMessage::GuildResponse { accept: true });
            }
            let b = &mut self.guild_invite_buttons[1];
            b.pos = (win.x + 170.0, win.y + 66.0);
            if b.update(c) {
                out.push(ClientMessage::GuildResponse { accept: false });
            }
        }

        // ---- Group (P) ----
        if self.group_open {
            // Left of the guild window so both can be open at once.
            let win = Rect::new(width as f32 - 300.0 - 10.0 - 360.0, 30.0, 300.0, 330.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            let closed = c.window(win, "Group", true);
            let leader = bag.group.first().map(|(id, _)| *id);
            let i_lead = leader.is_some() && leader == bag.user;
            if bag.group.is_empty() {
                c.text.draw(
                    "Not in a group.",
                    12,
                    win.x + 16.0,
                    win.y + 40.0,
                    [200, 200, 200, 255],
                );
            }
            if self.group_kick.len() != bag.group.len() {
                self.group_kick = bag
                    .group
                    .iter()
                    .map(|_| Button::default_style(0.0, 0.0, 60.0, "Kick"))
                    .collect();
            }
            for (i, (id, name)) in bag.group.iter().enumerate() {
                let y = win.y + 40.0 + i as f32 * 22.0;
                let label = if Some(*id) == leader {
                    format!("{name} (leader)")
                } else {
                    name.clone()
                };
                let col = if Some(*id) == bag.user {
                    [255, 255, 0, 255]
                } else {
                    [255, 255, 255, 255]
                };
                c.text.draw(&label, 12, win.x + 16.0, y, col);
                if i_lead && Some(*id) != bag.user {
                    let b = &mut self.group_kick[i];
                    b.pos = (win.x + win.w - 76.0, y - 2.0);
                    if b.update(c) {
                        out.push(ClientMessage::GroupRemove { name: name.clone() });
                    }
                }
            }
            // Invite box + buttons in the footer.
            let fy = win.y + win.h - 3.0 - 42.0 + 8.0;
            let name_box = self
                .group_name
                .get_or_insert_with(|| TextBox::new(Rect::new(0.0, 0.0, 130.0, 22.0), 20));
            name_box.rect = Rect::new(win.x + 12.0, fy + 2.0, 130.0, 22.0);
            let submitted = name_box.update(c);
            if self.group_buttons.is_empty() {
                self.group_buttons = vec![
                    Button::default_style(0.0, 0.0, 60.0, "Invite"),
                    Button::default_style(0.0, 0.0, 60.0, "Leave"),
                    Button::default_style(0.0, 0.0, 120.0, "Allow: off"),
                ];
            }
            let name = name_box.text.trim().to_string();
            let b = &mut self.group_buttons[0];
            b.pos = (win.x + 148.0, fy);
            b.enabled = !name.is_empty() && (bag.group.is_empty() || i_lead);
            if (b.update(c) || submitted) && b.enabled {
                out.push(ClientMessage::GroupInvite { name: name.clone() });
                if let Some(nb) = &mut self.group_name {
                    nb.text.clear();
                }
            }
            let b = &mut self.group_buttons[1];
            b.pos = (win.x + 214.0, fy);
            b.enabled = !bag.group.is_empty();
            if b.update(c) {
                out.push(ClientMessage::GroupRemove {
                    name: bag.player_name.to_string(),
                });
            }
            let b = &mut self.group_buttons[2];
            b.pos = (win.x + 12.0, win.y + win.h - 3.0 - 42.0 - 30.0);
            b.label = Some(if bag.allow_group {
                "Allow group: on".into()
            } else {
                "Allow group: off".into()
            });
            if b.update(c) {
                out.push(ClientMessage::GroupSwitch {
                    allow: !bag.allow_group,
                });
            }
            if closed {
                self.group_open = false;
            }
        }

        // ---- Group invite prompt ----
        if let Some(from) = bag.group_invite {
            let win = Rect::new(width as f32 / 2.0 - 150.0, 120.0, 300.0, 100.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            c.window(win, "Group invite", false);
            c.text.draw(
                &format!("{from} wants to group with you."),
                12,
                win.x + 16.0,
                win.y + 40.0,
                [255, 255, 255, 255],
            );
            if self.invite_buttons.is_empty() {
                self.invite_buttons = vec![
                    Button::default_style(0.0, 0.0, 80.0, "Accept"),
                    Button::default_style(0.0, 0.0, 80.0, "Decline"),
                ];
            }
            let b = &mut self.invite_buttons[0];
            b.pos = (win.x + 60.0, win.y + 66.0);
            if b.update(c) {
                out.push(ClientMessage::GroupResponse { accept: true });
            }
            let b = &mut self.invite_buttons[1];
            b.pos = (win.x + 160.0, win.y + 66.0);
            if b.update(c) {
                out.push(ClientMessage::GroupResponse { accept: false });
            }
        }

        // ---- NPC dialog (top-left, Zircon chrome 380/381/382) ----
        let mut npc_height = 0.0;
        if let Some(d) = &mut self.npc {
            // Lay out text to find the needed height.
            let text_w = 350.0;
            let (lines, _) = layout_parts(c, &d.parts, text_w);
            let text_h = (lines.len() + d.quests.len()) as f32 * 18.0;
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
            // Quest lines (Zircon shows them as links under the text).
            for q in &d.quests {
                let (label, col) = match q.state {
                    0 => (format!("[Accept] {}", q.name), [255, 255, 0, 255]),
                    2 => (format!("[Complete] {}", q.name), [80, 255, 80, 255]),
                    _ => (format!("(In progress) {}", q.name), [180, 180, 180, 255]),
                };
                let w = c.text.width(&label, 13);
                let hit = Rect::new(15.0, y, w, 18.0).contains(mouse.0, mouse.1);
                let col = if hit && q.state != 1 {
                    [255, 60, 60, 255]
                } else {
                    col
                };
                c.text.draw(&label, 13, 15.0, y, col);
                if hit && c.input.lmb_released && c.now >= d.last_button + 300 {
                    d.last_button = c.now;
                    match q.state {
                        0 => out.push(ClientMessage::QuestAccept { quest: q.quest }),
                        2 => {
                            // First reward that offers a choice for this class, if any.
                            let choice = bag
                                .catalog
                                .quest(q.quest)
                                .and_then(|def| {
                                    def.rewards
                                        .iter()
                                        .find(|r| r.choice && r.class & (1 << bag.stats.class) != 0)
                                        .map(|r| r.index)
                                })
                                .unwrap_or(0);
                            out.push(ClientMessage::QuestComplete {
                                quest: q.quest,
                                choice,
                            });
                        }
                        _ => {}
                    }
                }
                y += 18.0;
            }
            let mut close_now = (close_hover && c.input.lmb_released) || c.input.escape;
            // Developer automation: press a dialog button once.
            if clicked_button.is_none() && !self.auto_button_done {
                if let Some(b) = std::env::var("ZIRCON_AUTO_NPC_BUTTON")
                    .ok()
                    .and_then(|v| v.parse().ok())
                {
                    self.auto_button_done = true;
                    clicked_button = Some(b);
                }
            }
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

        // ---- Refine / retrieve / companion pages under the dialog ----
        let page_type = self.npc.as_ref().map(|d| d.dialog_type).unwrap_or(0);
        if page_type != 3 {
            self.refine_ores.clear();
            self.refine_items.clear();
            self.refine_specials.clear();
        }
        if page_type == 3 {
            let win = Rect::new(0.0, npc_height, 380.0, 300.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            c.window(win, "Refine", true);
            let weapon = bag
                .equipment
                .first()
                .and_then(|w| w.as_ref())
                .map(|w| bag.catalog.name(w.info))
                .unwrap_or_else(|| "no weapon equipped".into());
            c.text.draw(
                &format!("Weapon: {weapon}   Cost: 50,000 gold"),
                11,
                win.x + 12.0,
                win.y + 36.0,
                [255, 255, 200, 255],
            );
            if self.refine_buttons.is_empty() {
                let types = [
                    "Dura", "DC", "SP", "Fire", "Ice", "Light", "Wind", "Holy", "Dark", "Phan",
                ];
                let qualities = ["Rush", "Quick", "Std", "Careful", "Precise"];
                for t in types {
                    self.refine_buttons
                        .push(Button::default_style(0.0, 0.0, 34.0, t));
                }
                for q in qualities {
                    self.refine_buttons
                        .push(Button::default_style(0.0, 0.0, 60.0, q));
                }
                self.refine_buttons
                    .push(Button::default_style(0.0, 0.0, 110.0, "Refine"));
            }
            c.text.draw(
                "Type:",
                11,
                win.x + 12.0,
                win.y + 58.0,
                [220, 220, 220, 255],
            );
            for i in 0..10 {
                let b = &mut self.refine_buttons[i];
                b.pos = (win.x + 12.0 + i as f32 * 36.0, win.y + 72.0);
                b.selected = self.refine_type == (i + 1) as u8;
                if b.update(c) {
                    self.refine_type = (i + 1) as u8;
                }
            }
            c.text.draw(
                "Quality (1 min, 30 min, 1 h, 6 h, 1 day):",
                11,
                win.x + 12.0,
                win.y + 104.0,
                [220, 220, 220, 255],
            );
            for i in 0..5 {
                let b = &mut self.refine_buttons[10 + i];
                b.pos = (win.x + 12.0 + i as f32 * 64.0, win.y + 118.0);
                b.selected = self.refine_quality == i as u8;
                if b.update(c) {
                    self.refine_quality = i as u8;
                }
            }
            c.text.draw(
                "Right-click bag items: black iron ore (5), common jewellery (3), special (1).",
                10,
                win.x + 12.0,
                win.y + 150.0,
                [160, 160, 160, 255],
            );
            let mut y = win.y + 166.0;
            let mut remove: Option<(usize, usize)> = None;
            for (li, (label, list)) in [
                ("Ore", &self.refine_ores),
                ("Items", &self.refine_items),
                ("Special", &self.refine_specials),
            ]
            .iter()
            .enumerate()
            {
                let names: Vec<String> = list
                    .iter()
                    .map(|(_, s, n)| {
                        bag.inventory
                            .get(*s as usize)
                            .and_then(|c| c.as_ref())
                            .map(|it| format!("{} x{n}", bag.catalog.name(it.info)))
                            .unwrap_or_else(|| "?".into())
                    })
                    .collect();
                let r = Rect::new(win.x + 12.0, y, win.w - 24.0, 16.0);
                let hover = r.contains(mouse.0, mouse.1);
                c.text.draw(
                    &format!("{label}: {}", names.join(", ")),
                    11,
                    win.x + 12.0,
                    y,
                    if hover && !list.is_empty() {
                        [255, 150, 150, 255]
                    } else {
                        [255, 255, 255, 255]
                    },
                );
                if hover && c.input.lmb_pressed && !list.is_empty() {
                    remove = Some((li, list.len() - 1));
                }
                y += 18.0;
            }
            if let Some((li, i)) = remove {
                match li {
                    0 => {
                        self.refine_ores.remove(i);
                    }
                    1 => {
                        self.refine_items.remove(i);
                    }
                    _ => {
                        self.refine_specials.remove(i);
                    }
                }
            }
            let fy = win.y + win.h - 3.0 - 42.0 + 8.0;
            let b = &mut self.refine_buttons[15];
            b.pos = (win.x + 12.0, fy);
            b.enabled = bag.equipment.first().is_some_and(|w| w.is_some());
            if b.update(c) && b.enabled {
                out.push(ClientMessage::NpcRefine {
                    refine_type: self.refine_type,
                    quality: self.refine_quality,
                    ores: self.refine_ores.clone(),
                    items: self.refine_items.clone(),
                    specials: self.refine_specials.clone(),
                });
                self.refine_ores.clear();
                self.refine_items.clear();
                self.refine_specials.clear();
            }
        }
        if page_type == 4 {
            let rows = bag.refines.len().max(1);
            let win = Rect::new(0.0, npc_height, 380.0, 60.0 + rows as f32 * 22.0 + 46.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            c.window(win, "Refined weapons", true);
            if bag.refines.is_empty() {
                c.text.draw(
                    "Nothing in the furnace.",
                    11,
                    win.x + 12.0,
                    win.y + 40.0,
                    [200, 200, 200, 255],
                );
            }
            if self.retrieve_buttons.len() != bag.refines.len() {
                self.retrieve_buttons = bag
                    .refines
                    .iter()
                    .map(|_| Button::default_style(0.0, 0.0, 70.0, "Retrieve"))
                    .collect();
            }
            let types = [
                "",
                "Durability",
                "DC",
                "Spell power",
                "Fire",
                "Ice",
                "Lightning",
                "Wind",
                "Holy",
                "Dark",
                "Phantom",
            ];
            for (i, r) in bag.refines.iter().enumerate() {
                let y = win.y + 40.0 + i as f32 * 22.0;
                let ready = r.ready_in_ms == 0;
                let when = if ready {
                    "ready".to_string()
                } else {
                    let s = r.ready_in_ms / 1000;
                    format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
                };
                c.text.draw(
                    &format!(
                        "{} ({}) {}% - {when}",
                        bag.catalog.name(r.weapon.info),
                        types.get(r.refine_type as usize).unwrap_or(&""),
                        r.chance
                    ),
                    11,
                    win.x + 12.0,
                    y,
                    if ready {
                        [255, 255, 255, 255]
                    } else {
                        [180, 180, 180, 255]
                    },
                );
                let b = &mut self.retrieve_buttons[i];
                b.pos = (win.x + win.w - 84.0, y - 2.0);
                b.enabled = ready;
                if b.update(c) && b.enabled {
                    out.push(ClientMessage::NpcRefineRetrieve { index: r.index });
                }
            }
        }
        if page_type == 5 {
            let rows = bag.companion_shop.len() + bag.companions.len();
            let win = Rect::new(0.0, npc_height, 400.0, 90.0 + rows as f32 * 22.0 + 46.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            c.window(win, "Companions", true);
            let name_box = self
                .companion_name
                .get_or_insert_with(|| TextBox::new(Rect::new(0.0, 0.0, 140.0, 22.0), 15));
            c.text.draw(
                "Name for a new companion:",
                11,
                win.x + 12.0,
                win.y + 38.0,
                [220, 220, 220, 255],
            );
            name_box.rect = Rect::new(win.x + 180.0, win.y + 34.0, 140.0, 22.0);
            name_box.update(c);
            let name = name_box.text.trim().to_string();
            if self.companion_offer_buttons.len() != bag.companion_shop.len() {
                self.companion_offer_buttons = bag
                    .companion_shop
                    .iter()
                    .map(|_| Button::default_style(0.0, 0.0, 64.0, "Adopt"))
                    .collect();
            }
            let mut y = win.y + 66.0;
            for (i, o) in bag.companion_shop.iter().enumerate() {
                c.text.draw(
                    &format!("{} - {} {}", o.name, o.price, o.currency),
                    11,
                    win.x + 12.0,
                    y,
                    if o.unlocked {
                        [255, 255, 255, 255]
                    } else {
                        [160, 160, 160, 255]
                    },
                );
                let b = &mut self.companion_offer_buttons[i];
                b.pos = (win.x + win.w - 78.0, y - 2.0);
                b.label = Some(if o.unlocked {
                    "Adopt".into()
                } else {
                    "Unlock".into()
                });
                b.enabled = !o.unlocked || !name.is_empty();
                if b.update(c) && b.enabled {
                    if o.unlocked {
                        out.push(ClientMessage::CompanionAdopt {
                            index: o.index,
                            name: name.clone(),
                        });
                        if let Some(nb) = &mut self.companion_name {
                            nb.text.clear();
                        }
                    } else {
                        out.push(ClientMessage::CompanionUnlock { index: o.index });
                    }
                }
                y += 22.0;
            }
            if self.companion_own_buttons.len() != bag.companions.len() * 3 {
                self.companion_own_buttons = bag
                    .companions
                    .iter()
                    .flat_map(|_| {
                        [
                            Button::default_style(0.0, 0.0, 60.0, "Out"),
                            Button::default_style(0.0, 0.0, 60.0, "Store"),
                            Button::default_style(0.0, 0.0, 64.0, "Release"),
                        ]
                    })
                    .collect();
            }
            for (i, cmp) in bag.companions.iter().enumerate() {
                c.text.draw(
                    &format!(
                        "{} the {} Lv{}{}",
                        cmp.name,
                        cmp.kind,
                        cmp.level,
                        if cmp.active { " (out)" } else { "" }
                    ),
                    11,
                    win.x + 12.0,
                    y,
                    [255, 255, 0, 255],
                );
                let b = &mut self.companion_own_buttons[i * 3];
                b.pos = (win.x + win.w - 204.0, y - 2.0);
                b.enabled = !cmp.active;
                if b.update(c) && b.enabled {
                    out.push(ClientMessage::CompanionRetrieve { index: cmp.index });
                }
                let b = &mut self.companion_own_buttons[i * 3 + 1];
                b.pos = (win.x + win.w - 142.0, y - 2.0);
                b.enabled = cmp.active;
                if b.update(c) && b.enabled {
                    out.push(ClientMessage::CompanionStore);
                }
                let b = &mut self.companion_own_buttons[i * 3 + 2];
                b.pos = (win.x + win.w - 78.0, y - 2.0);
                b.enabled = cmp.items.is_empty();
                if b.update(c) && b.enabled {
                    out.push(ClientMessage::CompanionRelease { index: cmp.index });
                }
                y += 22.0;
            }
        } else if let Some(b) = &mut self.companion_name {
            b.focused = false;
        }

        // ---- Companion window (N): the active companion and its bag ----
        if self.companion_open {
            let win = Rect::new(width as f32 / 2.0 - 160.0, 80.0, 320.0, 300.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            let closed = c.window(win, "Companion", true);
            match bag.companions.iter().find(|cmp| cmp.active) {
                None => c.text.draw(
                    "No companion is out. Visit the companion keeper.",
                    11,
                    win.x + 12.0,
                    win.y + 40.0,
                    [200, 200, 200, 255],
                ),
                Some(cmp) => {
                    c.text.draw(
                        &format!("{} the {}  Lv {}", cmp.name, cmp.kind, cmp.level),
                        12,
                        win.x + 12.0,
                        win.y + 38.0,
                        [255, 255, 0, 255],
                    );
                    c.text.draw(
                        &format!(
                            "Exp {}/{}   Hunger {}   Bag {}/{} ({}/{} weight)",
                            cmp.experience,
                            cmp.max_experience,
                            cmp.hunger,
                            cmp.items.len(),
                            cmp.bag_size,
                            cmp.bag_weight,
                            cmp.max_weight
                        ),
                        10,
                        win.x + 12.0,
                        win.y + 56.0,
                        [220, 220, 220, 255],
                    );
                    if self.companion_take.len() != cmp.items.len() {
                        self.companion_take = cmp
                            .items
                            .iter()
                            .map(|_| Button::default_style(0.0, 0.0, 50.0, "Take"))
                            .collect();
                    }
                    let mut y = win.y + 78.0;
                    for (i, it) in cmp.items.iter().enumerate().take(9) {
                        c.text.draw(
                            &format!("{} x{}", bag.catalog.name(it.info), it.count),
                            11,
                            win.x + 12.0,
                            y + 2.0,
                            [255, 255, 255, 255],
                        );
                        let b = &mut self.companion_take[i];
                        b.pos = (win.x + win.w - 64.0, y - 2.0);
                        if b.update(c) {
                            out.push(ClientMessage::CompanionBagTake {
                                index: cmp.index,
                                slot: i as u8,
                            });
                        }
                        y += 22.0;
                    }
                    if cmp.hunger <= 0 {
                        c.text.draw(
                            "Hungry: feed it to keep it picking up.",
                            10,
                            win.x + 12.0,
                            win.y + win.h - 3.0 - 42.0 + 12.0,
                            [255, 120, 120, 255],
                        );
                    }
                }
            }
            if closed {
                self.companion_open = false;
            }
        }

        // ---- Goods (shop) window under the dialog ----
        if let Some(d) = &mut self.npc {
            if d.dialog_type == 1 && !d.goods.is_empty() {
                let rows = d.goods.len().min(7);
                let win = Rect::new(0.0, npc_height, 245.0, 37.0 + rows as f32 * 43.0 + 50.0);
                let max_scroll = d.goods.len().saturating_sub(7);
                if win.contains(mouse.0, mouse.1) {
                    over = true;
                    if c.input.wheel != 0.0 && max_scroll > 0 {
                        let next = d.goods_scroll as f32 - c.input.wheel;
                        d.goods_scroll = next.round().clamp(0.0, max_scroll as f32) as usize;
                    }
                }
                let closed = c.window(win, "Goods", true);
                if max_scroll > 0 {
                    // Scrollbar on the right of the list (Zircon `DXVScrollBar`).
                    let track =
                        Rect::new(win.x + 231.0, win.y + 37.0, 8.0, rows as f32 * 43.0 - 3.0);
                    c.fill(track, [0.0, 0.0, 0.0, 0.5]);
                    let thumb_h = (track.h * rows as f32 / d.goods.len() as f32).max(12.0);
                    let thumb_y =
                        track.y + (track.h - thumb_h) * d.goods_scroll as f32 / max_scroll as f32;
                    c.fill(
                        Rect::new(track.x, thumb_y, track.w, thumb_h),
                        [198.0 / 255.0, 166.0 / 255.0, 99.0 / 255.0, 1.0],
                    );
                }
                let first = d.goods_scroll.min(max_scroll);
                for (i, g) in d.goods.iter().enumerate().skip(first).take(7) {
                    let row = Rect::new(
                        win.x + 10.0,
                        win.y + 37.0 + (i - first) as f32 * 43.0,
                        219.0,
                        40.0,
                    );
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
                        added: Vec::new(),
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
                        win.x + 118.0,
                        win.y + win.h - 38.0,
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
                        self.hover_inventory = Some(i as u8);
                    }
                    if c.input.lmb_pressed {
                        self.click_slot(Grid::Inventory, i as u8, item.is_some(), out);
                    }
                    if c.input.rmb_pressed {
                        if let Some(it) = item {
                            if let Some(t) = bag.trade {
                                if !t
                                    .my_items
                                    .iter()
                                    .any(|(g, s, _)| *g == Grid::Inventory && *s == i as u8)
                                {
                                    out.push(ClientMessage::TradeAddItem {
                                        grid: Grid::Inventory,
                                        slot: i as u8,
                                        count: it.count,
                                    });
                                }
                                continue;
                            }
                            // Wedding-ring NPC page: a ring becomes the ring.
                            if self.npc.as_ref().is_some_and(|d| d.dialog_type == 6) {
                                out.push(ClientMessage::MarriageMakeRing { slot: i as u8 });
                                continue;
                            }
                            if self.npc.as_ref().is_some_and(|d| d.dialog_type == 3) {
                                // Refine page: sort the item into the furnace lists.
                                let cell = (Grid::Inventory, i as u8, it.count);
                                let def = bag.catalog.get(it.info);
                                let already = self
                                    .refine_ores
                                    .iter()
                                    .chain(&self.refine_items)
                                    .chain(&self.refine_specials)
                                    .any(|(g, s, _)| *g == Grid::Inventory && *s == i as u8);
                                if !already {
                                    if def.is_some_and(|d| d.effect == 20) {
                                        if self.refine_ores.len() < 5 {
                                            self.refine_ores.push(cell);
                                        }
                                    } else if def.is_some_and(|d| matches!(d.item_type, 6..=8)) {
                                        if self.refine_items.len() < 3 {
                                            self.refine_items.push(cell);
                                        }
                                    } else if def.is_some_and(|d| d.item_type == 17)
                                        && self.refine_specials.is_empty()
                                    {
                                        self.refine_specials.push((Grid::Inventory, i as u8, 1));
                                    }
                                }
                                continue;
                            }
                            if self.mail_open && self.mail_compose {
                                if self.mail_attach.len() < 5
                                    && !self
                                        .mail_attach
                                        .iter()
                                        .any(|(g, s, _)| *g == Grid::Inventory && *s == i as u8)
                                {
                                    self.mail_attach.push((Grid::Inventory, i as u8, it.count));
                                }
                                continue;
                            }
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
                (
                    "Fame",
                    if s.fame_title.is_empty() {
                        "-".to_string()
                    } else {
                        s.fame_title.clone()
                    },
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
            if let Some(partner) = bag.partner {
                let ring_on = bag.wedding_ring.is_some_and(|r| {
                    bag.equipment
                        .get(7)
                        .and_then(|c| c.as_ref())
                        .is_some_and(|it| it.id == r)
                });
                c.text.draw(
                    &format!(
                        "Married to {partner}{}",
                        if ring_on { " (wedding ring on)" } else { "" }
                    ),
                    11,
                    win.x + 160.0,
                    win.y + 400.0,
                    [255, 150, 200, 255],
                );
                if self.marriage_buttons.is_empty() {
                    self.marriage_buttons = vec![
                        Button::default_style(0.0, 0.0, 80.0, "Accept"),
                        Button::default_style(0.0, 0.0, 80.0, "Decline"),
                        Button::default_style(0.0, 0.0, 120.0, "To partner"),
                    ];
                }
                let b = &mut self.marriage_buttons[2];
                b.pos = (win.x + 180.0, win.y + win.h - 3.0 - 42.0 + 8.0);
                b.enabled = ring_on;
                if b.update(c) {
                    out.push(ClientMessage::MarriageTeleport);
                }
            }
            if closed {
                self.character_open = false;
                self.carrying = None;
            }
        }

        // ---- Skill window (Zircon MagicDialog 419x511, standard chrome here) ----
        self.hover_magic = None;
        if self.skills_open {
            let win = Rect::new((width as f32 - 419.0) / 2.0, 30.0, 419.0, 511.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
                self.skill_scroll = (self.skill_scroll - c.input.wheel * 59.0).max(0.0);
            }
            let closed = c.window(win, "Skills", false);
            let class = bag.stats.class;
            let list = bag.catalog.class_magics(class);
            let max_scroll = ((list.len() as f32) * 59.0 - 460.0).max(0.0);
            self.skill_scroll = self.skill_scroll.min(max_scroll);
            let top = win.y + 40.0;
            let first = (self.skill_scroll / 59.0) as usize;
            for (i, def) in list.iter().enumerate().skip(first).take(8) {
                let y = top + (i as f32 * 59.0 - self.skill_scroll);
                if y + 54.0 > win.y + win.h - 6.0 {
                    break;
                }
                let cell = Rect::new(win.x + 25.0, y, 369.0, 54.0);
                c.panel(cell, [16, 8, 8, 255]);
                let known = bag.magics.iter().find(|m| m.magic == def.magic);
                let usable = bag.stats.level as i32 >= def.need_level[0];
                if known.is_some() {
                    let border = match def.school {
                        1 => 860,
                        2 => 861,
                        3 => 862,
                        4 => 870,
                        5 => 871,
                        6 => 872,
                        7 => 873,
                        10 => 874,
                        8 => 880,
                        9 => 881,
                        11 => 883,
                        12 => 890,
                        13 => 891,
                        14 => 892,
                        _ => 0,
                    };
                    if border > 0 {
                        c.draw(lib::GAME_INTER2, border, cell.x + 4.0, cell.y + 4.0);
                    }
                }
                let icon = Rect::new(cell.x + 9.0, cell.y + 9.0, 36.0, 36.0);
                let alpha = if known.is_some() {
                    1.0
                } else if usable {
                    0.6
                } else {
                    0.3
                };
                c.draw_tinted(
                    lib::MAGIC_ICON,
                    def.icon as u32,
                    icon.x,
                    icon.y,
                    [1.0, 1.0, 1.0, alpha],
                    Blend::Alpha,
                );
                if let Some(m) = known {
                    if m.key > 0 {
                        c.text.draw(
                            &format!("F{}", m.key),
                            11,
                            icon.x + 18.0,
                            icon.y + 22.0,
                            [127, 255, 212, 255],
                        );
                    }
                    if bag.toggles.contains(&def.magic) {
                        c.border(icon, [0, 255, 0, 255]);
                    }
                }
                c.text.draw(
                    &def.name,
                    13,
                    cell.x + 55.0,
                    cell.y + 1.0,
                    [255, 255, 255, 255],
                );
                match known {
                    Some(m) => {
                        c.text.draw(
                            &format!("Level: {}", m.level),
                            12,
                            cell.x + 57.0,
                            cell.y + 30.0,
                            GOLD,
                        );
                        let max = def.experience.get(m.level as usize).copied().unwrap_or(0);
                        let (label, pct) = if m.level >= 3 {
                            ("Experience: Max".to_string(), 1.0)
                        } else if max > 0 {
                            (
                                format!("Experience: {}/{}", m.experience, max),
                                (m.experience as f32 / max as f32).min(1.0),
                            )
                        } else {
                            (String::new(), 0.0)
                        };
                        let lw = c.text.width(&label, 11);
                        c.text.draw(
                            &label,
                            11,
                            cell.x + cell.w - lw - 6.0,
                            cell.y + 17.0,
                            [200, 200, 200, 255],
                        );
                        if let Some(bar) = c.sprite(lib::GAME_INTER2, 812) {
                            c.renderer.draw_cropped(
                                bar,
                                cell.x + 110.0,
                                cell.y + 36.0,
                                pct,
                                [1.0, 1.0, 1.0, 1.0],
                            );
                        }
                        let need = def.need_level.get(m.level as usize).copied().unwrap_or(0);
                        if m.level < 3 && (bag.stats.level as i32) < need {
                            c.text.draw(
                                &format!("Required Level: {need}"),
                                11,
                                cell.x + 57.0,
                                cell.y + 17.0,
                                [255, 80, 80, 255],
                            );
                        }
                    }
                    None => {
                        c.text.draw(
                            "Not Learned",
                            12,
                            cell.x + 57.0,
                            cell.y + 17.0,
                            [255, 80, 80, 255],
                        );
                        let col = if usable {
                            [120, 255, 120, 255]
                        } else {
                            [255, 80, 80, 255]
                        };
                        c.text.draw(
                            &format!("Required Level: {}", def.need_level[0]),
                            11,
                            cell.x + 57.0,
                            cell.y + 33.0,
                            col,
                        );
                    }
                }
                if icon.contains(mouse.0, mouse.1) {
                    self.hover_magic = Some(def.magic);
                    self.magic_tip = Some((def.magic, mouse.0, mouse.1));
                    if known.is_some() && c.input.lmb_pressed {
                        out.push(ClientMessage::MagicKey {
                            magic: def.magic,
                            key: 0,
                        });
                    }
                }
            }
            c.text.draw(
                "Hover a skill and press F1-F11 to bind it; click to unbind",
                11,
                win.x + 25.0,
                win.y + win.h - 22.0,
                [160, 160, 160, 255],
            );
            if closed {
                self.skills_open = false;
            }
        }
        if let Some((magic, x, y)) = self.magic_tip.take() {
            if let Some(def) = bag.catalog.magic(magic) {
                let known = bag.magics.iter().find(|m| m.magic == magic);
                let mut lines: Vec<(String, [u8; 4])> =
                    vec![(def.name.clone(), [255, 255, 0, 255])];
                match known {
                    Some(m) => lines.push((
                        format!("Current Level: {}  Cost: {} MP", m.level, def.cost(m.level)),
                        [120, 255, 120, 255],
                    )),
                    None => lines.push(("Not learned".into(), [255, 80, 80, 255])),
                }
                for (i, (need, exp)) in def.need_level.iter().zip(def.experience.iter()).enumerate()
                {
                    let col = if (bag.stats.level as i32) < *need {
                        [255, 80, 80, 255]
                    } else {
                        [200, 200, 200, 255]
                    };
                    lines.push((
                        format!("Rank {}: Level {need}, Experience {exp}", i + 1),
                        col,
                    ));
                }
                if !def.description.is_empty() {
                    lines.push((def.description.clone(), [245, 222, 179, 255]));
                }
                let w = lines
                    .iter()
                    .map(|(l, _)| c.text.width(l, 12))
                    .fold(0.0, f32::max)
                    + 16.0;
                let h = lines.len() as f32 * 16.0 + 8.0;
                let r = Rect::new((x + 16.0).min(width as f32 - w - 4.0), y + 8.0, w, h);
                c.fill(r, [0.0, 24.0 / 255.0, 48.0 / 255.0, 0.8]);
                c.border(r, [255, 255, 0, 255]);
                for (i, (l, col)) in lines.iter().enumerate() {
                    c.text
                        .draw(l, 12, r.x + 8.0, r.y + 4.0 + i as f32 * 16.0, *col);
                }
            }
        }

        // ---- Spell bar (bottom-left, above the HUD) ----
        {
            let base_y = height as f32 - 96.0 - 44.0;
            for key in 1..=11u8 {
                let r = Rect::new(10.0 + (key as f32 - 1.0) * 37.0, base_y, 36.0, 36.0);
                c.fill(r, [20.0 / 255.0, 20.0 / 255.0, 20.0 / 255.0, 0.6]);
                c.border(r, GOLD);
                if let Some(m) = bag.magics.iter().find(|m| m.key == key) {
                    if let Some(def) = bag.catalog.magic(m.magic) {
                        c.draw_tinted(
                            lib::MAGIC_ICON,
                            def.icon as u32,
                            r.x,
                            r.y,
                            [1.0, 1.0, 1.0, 0.8],
                            Blend::Alpha,
                        );
                        if bag.toggles.contains(&m.magic) {
                            c.border(r, [0, 255, 0, 255]);
                        }
                        if let Some(until) = bag.cooldowns.get(&m.magic) {
                            if *until > c.now {
                                c.fill(r, [50.0 / 255.0, 50.0 / 255.0, 50.0 / 255.0, 0.5]);
                                let secs = (until - c.now).div_ceil(1000);
                                c.text.draw_centered(
                                    &secs.to_string(),
                                    13,
                                    r.x + 18.0,
                                    r.y + 10.0,
                                    GOLD,
                                );
                            }
                        }
                        if r.contains(mouse.0, mouse.1) {
                            self.magic_tip = Some((m.magic, mouse.0, mouse.1));
                        }
                    }
                }
                c.text.draw(
                    &format!("F{key}"),
                    9,
                    r.x + 2.0,
                    r.y + 24.0,
                    [255, 255, 255, 200],
                );
            }
        }

        // ---- Belt (bottom-right, above the HUD): 10 cells keyed 1..9, 0 ----
        if self.belt_open {
            let cells = MAX_BELT as f32;
            let inner = Rect::new(
                width as f32 - 10.0 - (cells * (CELL - 1.0) + 1.0),
                height as f32 - 96.0 - 44.0,
                cells * (CELL - 1.0) + 1.0,
                CELL,
            );
            let win = Rect::new(inner.x - 6.0, inner.y - 6.0, inner.w + 12.0, inner.h + 12.0);
            if win.contains(mouse.0, mouse.1) {
                over = true;
            }
            c.panel(win, [20, 20, 20, 200]);
            for (i, link) in bag.belt.iter().enumerate().take(MAX_BELT) {
                let r = Rect::new(inner.x + i as f32 * (CELL - 1.0), inner.y, CELL, CELL);
                let hover = r.contains(mouse.0, mouse.1);
                let item = belt_cell_item(bag.inventory, link);
                draw_item_cell(c, bag.catalog, r, item.as_ref(), hover, false);
                if item.is_some() && bag.use_item_time > c.now {
                    c.fill(r, [120.0 / 255.0, 120.0 / 255.0, 120.0 / 255.0, 0.6]);
                    let secs = (bag.use_item_time - c.now).div_ceil(1000);
                    c.text
                        .draw_centered(&secs.to_string(), 13, r.x + 18.0, r.y + 10.0, GOLD);
                }
                c.text.draw(
                    &((i + 1) % 10).to_string(),
                    9,
                    r.x + 2.0,
                    r.y + 1.0,
                    [255, 255, 255, 220],
                );
                if !hover {
                    continue;
                }
                if let Some(it) = &item {
                    self.tooltip = Some((it.info, mouse.0, mouse.1));
                }
                if c.input.lmb_pressed {
                    match self.carrying.take() {
                        Some((Grid::Inventory, s)) => {
                            let l = link_for(bag.catalog, bag.inventory, s, i as u8);
                            out.push(ClientMessage::BeltLink {
                                slot: l.slot,
                                info: l.info,
                                item: l.item,
                            });
                        }
                        Some(other) => self.carrying = Some(other),
                        None if item.is_some() => out.push(ClientMessage::BeltLink {
                            slot: i as u8,
                            info: None,
                            item: None,
                        }),
                        None => {}
                    }
                }
                if c.input.rmb_pressed {
                    if let Some(slot) = belt_inventory_slot(bag.inventory, link) {
                        out.push(ClientMessage::ItemUse { slot });
                    }
                }
            }
        }

        // ---- Death: revive button in the middle of the screen ----
        if bag.dead {
            let msg = "You have died.";
            let w = c.text.width(msg, 16);
            let cx = width as f32 / 2.0;
            let cy = height as f32 / 2.0;
            c.text
                .draw(msg, 16, cx - w / 2.0, cy - 60.0, [255, 80, 80, 255]);
            let b = self
                .revive_button
                .get_or_insert_with(|| Button::default_style(0.0, 0.0, 110.0, "Revive"));
            b.pos = (cx - 55.0, cy - 30.0);
            b.enabled = true;
            if b.update(c) {
                out.push(ClientMessage::TownRevive);
            }
        }

        // HUD buttons and the dialogs behind them, on top of everything.
        let mut actions: Vec<MenuAction> = Vec::new();
        over |= self.menu.draw(c, bag, width, height, &mut actions);
        for a in actions {
            match a {
                MenuAction::Toggle(w) => match w {
                    menu::Window::Character => self.character_open = !self.character_open,
                    menu::Window::Inventory => self.inventory_open = !self.inventory_open,
                    menu::Window::Skills => self.skills_open = !self.skills_open,
                    menu::Window::Quests => self.quests_open = !self.quests_open,
                    menu::Window::Mail => self.mail_open = !self.mail_open,
                    menu::Window::Belt => self.belt_open = !self.belt_open,
                    menu::Window::Group => self.group_open = !self.group_open,
                    menu::Window::Guild => self.guild_open = !self.guild_open,
                    menu::Window::Storage => self.storage_open = !self.storage_open,
                    menu::Window::Companion => self.companion_open = !self.companion_open,
                },
                MenuAction::Logout => out.push(ClientMessage::Logout),
                MenuAction::Quit => std::process::exit(0),
            }
        }
        // Drop the carried item when clicking outside any window.
        if c.input.lmb_pressed && !over {
            self.carrying = None;
        }
        if let Some((info, x, y)) = self.tooltip {
            // Its own layer, above every window: a tooltip always wins.
            c.layer();
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

/// Greedy word wrap at `max_w` pixels for `size` text.
fn wrap_text(c: &mut Ctx, text: &str, max_w: f32, size: u32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if c.text.width(&candidate, size) > max_w && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}
