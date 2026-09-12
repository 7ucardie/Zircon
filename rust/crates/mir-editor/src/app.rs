//! The editor window: collection list, filterable table, detail form with
//! sprite preview and related records, save with backup, undo.

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use mir_formats::mirdb::{MirDb, Record, Value};

use crate::preview::{self, Previews};
use crate::value;

/// Snapshot of one collection's rows for undo.
struct Undo {
    collection: usize,
    records: Vec<Record>,
    next_index: i32,
}

pub struct Editor {
    db_path: PathBuf,
    db: MirDb,
    previews: Previews,
    dirty: bool,
    status: String,
    /// Selected collection index.
    selected: usize,
    filter: String,
    /// (property index, ascending).
    sort: Option<(usize, bool)>,
    /// Selected record index within the collection.
    row: Option<usize>,
    /// Pending jump to (collection, record) requested from a link.
    jump: Option<(usize, usize)>,
    undo: Vec<Undo>,
    /// Text being edited in the form for complex values: property -> text.
    form_text: HashMap<usize, String>,
    form_error: Option<String>,
    monster_direction: u8,
}

const NAME_PROPS: [&str; 7] = [
    "ItemName",
    "MonsterName",
    "NPCName",
    "Name",
    "FileName",
    "Description",
    "RegionName",
];

impl Editor {
    pub fn open(db_path: PathBuf, assets: PathBuf) -> anyhow::Result<Editor> {
        let db = MirDb::load(&db_path)
            .map_err(|e| anyhow::anyhow!("loading {}: {e}", db_path.display()))?;
        let mut ed = Editor {
            status: format!(
                "{} collections loaded from {}",
                db.collections.len(),
                db_path.display()
            ),
            db_path,
            db,
            previews: Previews::new(assets),
            dirty: false,
            selected: 0,
            filter: String::new(),
            sort: None,
            row: None,
            jump: None,
            undo: Vec::new(),
            form_text: HashMap::new(),
            form_error: None,
            monster_direction: 4,
        };
        if let Ok(name) = std::env::var("ZIRCON_EDITOR_COLLECTION") {
            if let Some(i) = ed
                .db
                .collections
                .iter()
                .position(|c| c.short_name() == name)
            {
                ed.selected = i;
            }
        }
        if let Ok(f) = std::env::var("ZIRCON_EDITOR_FILTER") {
            ed.filter = f;
        }
        if let Ok(r) = std::env::var("ZIRCON_EDITOR_ROW") {
            let rows = ed.visible_rows();
            ed.row = r.parse::<usize>().ok().and_then(|i| rows.get(i).copied());
        }
        Ok(ed)
    }

    // ---- data helpers -------------------------------------------------------

    /// The collection an integer property points at, by naming convention
    /// (`Item` -> `ItemInfo`, `Region` -> `MapRegion`, `Page` -> `NPCPage`).
    fn fk_target(&self, prop: &str) -> Option<usize> {
        if prop == "Index" {
            return None;
        }
        let candidates = [
            format!("{prop}Info"),
            format!("Map{prop}"),
            format!("NPC{prop}"),
            prop.to_string(),
        ];
        self.db
            .collections
            .iter()
            .position(|c| candidates.iter().any(|n| c.short_name() == *n))
    }

    fn record_name(&self, collection: usize, record: &Record) -> String {
        let c = &self.db.collections[collection];
        for n in NAME_PROPS {
            if let Some(v) = c.get(record, n).and_then(Value::as_str) {
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
        format!("#{}", c.index(record))
    }

    fn position_of_index(&self, collection: usize, index: i32) -> Option<usize> {
        let c = &self.db.collections[collection];
        c.records.iter().position(|r| c.index(r) == index)
    }

    /// Record indices matching the filter, in sort order.
    fn visible_rows(&self) -> Vec<usize> {
        let c = &self.db.collections[self.selected];
        let needle = self.filter.trim().to_lowercase();
        let index_needle: Option<i32> = needle.parse().ok();
        let mut rows: Vec<usize> = c
            .records
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                if needle.is_empty() {
                    return true;
                }
                if index_needle == Some(c.index(r)) {
                    return true;
                }
                r.values.iter().any(|v| match v {
                    Value::Str(s) => s.to_lowercase().contains(&needle),
                    _ => false,
                })
            })
            .map(|(i, _)| i)
            .collect();
        if let Some((prop, asc)) = self.sort {
            rows.sort_by(|a, b| {
                let va = &c.records[*a].values[prop];
                let vb = &c.records[*b].values[prop];
                let ord = match (va.as_f64(), vb.as_f64()) {
                    (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                    _ => value::summary(va).cmp(&value::summary(vb)),
                };
                if asc {
                    ord
                } else {
                    ord.reverse()
                }
            });
        }
        rows
    }

    fn snapshot(&mut self) {
        let c = &self.db.collections[self.selected];
        self.undo.push(Undo {
            collection: self.selected,
            records: c.records.clone(),
            next_index: c.next_index,
        });
        if self.undo.len() > 50 {
            self.undo.remove(0);
        }
        self.dirty = true;
    }

    fn undo(&mut self) {
        if let Some(u) = self.undo.pop() {
            let c = &mut self.db.collections[u.collection];
            c.records = u.records;
            c.next_index = u.next_index;
            self.selected = u.collection;
            self.row = self.row.filter(|r| *r < c.records.len());
            self.form_text.clear();
            self.status = "Undone".into();
        }
    }

    fn save(&mut self) {
        match self.db.save(&self.db_path) {
            Ok(Some(bak)) => {
                self.dirty = false;
                self.status = format!("Saved; previous file kept as {}", bak.display());
            }
            Ok(None) => {
                self.dirty = false;
                self.status = "Saved".into();
            }
            Err(e) => self.status = format!("Save failed: {e}"),
        }
    }

    fn reload(&mut self) {
        match MirDb::load(&self.db_path) {
            Ok(db) => {
                self.db = db;
                self.dirty = false;
                self.undo.clear();
                self.form_text.clear();
                self.row = None;
                self.status = "Reloaded from disk".into();
            }
            Err(e) => self.status = format!("Reload failed: {e}"),
        }
    }

    // ---- ui -----------------------------------------------------------------

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if let Some((c, r)) = self.jump.take() {
            self.selected = c;
            self.row = Some(r);
            self.filter.clear();
            self.form_text.clear();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            self.save();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
            self.undo();
        }
        self.top_bar(ui);
        self.collections_panel(ui);
        self.detail_panel(ui);
        egui::CentralPanel::default().show(ui, |ui| self.table(ui));
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("top").show(ui, |ui| {
            ui.horizontal(|ui| {
                let save = ui.add_enabled(self.dirty, egui::Button::new("Save (Cmd+S)"));
                if save.clicked() {
                    self.save();
                }
                if ui.button("Reload").clicked() {
                    self.reload();
                }
                if ui
                    .add_enabled(!self.undo.is_empty(), egui::Button::new("Undo (Cmd+Z)"))
                    .clicked()
                {
                    self.undo();
                }
                ui.separator();
                ui.label("Filter:");
                ui.add(egui::TextEdit::singleline(&mut self.filter).desired_width(220.0));
                if ui.button("Clear").clicked() {
                    self.filter.clear();
                }
                ui.separator();
                if ui.button("Add").clicked() {
                    self.snapshot();
                    let c = &mut self.db.collections[self.selected];
                    c.push_record(Vec::new());
                    self.row = Some(c.records.len() - 1);
                    self.filter.clear();
                }
                let has_row = self.row.is_some();
                if ui
                    .add_enabled(has_row, egui::Button::new("Duplicate"))
                    .clicked()
                {
                    let r = self.row.unwrap();
                    self.snapshot();
                    let c = &mut self.db.collections[self.selected];
                    let mut copy = c.records[r].clone();
                    let index = c.next_index;
                    c.next_index += 1;
                    c.set(&mut copy, "Index", Value::Int(index as i64));
                    c.records.push(copy);
                    self.row = Some(c.records.len() - 1);
                }
                if ui
                    .add_enabled(has_row, egui::Button::new("Delete"))
                    .clicked()
                {
                    let r = self.row.unwrap();
                    self.snapshot();
                    self.db.collections[self.selected].records.remove(r);
                    self.row = None;
                    self.form_text.clear();
                }
                ui.separator();
                let dirty = if self.dirty { " (unsaved changes)" } else { "" };
                ui.label(format!("{}{dirty}", self.status));
            });
        });
    }

    fn collections_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("collections")
            .default_size(220.0)
            .show(ui, |ui| {
                ui.heading("Collections");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for i in 0..self.db.collections.len() {
                        let c = &self.db.collections[i];
                        let label = format!("{} ({})", c.short_name(), c.records.len());
                        if ui.selectable_label(self.selected == i, label).clicked()
                            && self.selected != i
                        {
                            self.selected = i;
                            self.row = None;
                            self.sort = None;
                            self.form_text.clear();
                        }
                    }
                });
            });
    }

    fn table(&mut self, ui: &mut egui::Ui) {
        let rows = self.visible_rows();
        let props: Vec<(String, String)> = self.db.collections[self.selected]
            .mapping
            .properties
            .iter()
            .map(|p| (p.name.clone(), p.type_name.clone()))
            .collect();
        ui.heading(format!(
            "{}  —  {} of {} rows",
            self.db.collections[self.selected].short_name(),
            rows.len(),
            self.db.collections[self.selected].records.len()
        ));
        let mut changed = false;
        let mut clicked_row = None;
        let mut jump = None;
        let mut sort_click = None;
        let selected = self.selected;
        let current = self.row;
        let sort = self.sort;
        let fk: Vec<Option<usize>> = props.iter().map(|(n, _)| self.fk_target(n)).collect();
        let text_height = 20.0;
        let mut builder = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(26.0));
        for (name, ty) in &props {
            let w = match ty.as_str() {
                "System.String" if name.contains("Description") => 260.0,
                "System.String" => 150.0,
                "System.Boolean" => 60.0,
                _ => 80.0,
            };
            builder = builder.column(Column::initial(w).at_least(40.0).clip(true));
        }
        builder
            .header(text_height, |mut header| {
                header.col(|_| {});
                for (pi, (name, _)) in props.iter().enumerate() {
                    header.col(|ui| {
                        let marker = match sort {
                            Some((p, true)) if p == pi => " ▲",
                            Some((p, false)) if p == pi => " ▼",
                            _ => "",
                        };
                        if ui.button(format!("{name}{marker}")).clicked() {
                            sort_click = Some(pi);
                        }
                    });
                }
            })
            .body(|body| {
                let db = &mut self.db;
                body.rows(text_height, rows.len(), |mut row| {
                    let ri = rows[row.index()];
                    let is_selected = current == Some(ri);
                    row.set_selected(is_selected);
                    row.col(|ui| {
                        if ui.selectable_label(is_selected, "▶").clicked() {
                            clicked_row = Some(ri);
                        }
                    });
                    for (pi, target) in fk.iter().enumerate() {
                        row.col(|ui| {
                            let c = &mut db.collections[selected];
                            let v = &mut c.records[ri].values[pi];
                            if let (Some(target), Some(idx)) = (*target, v.as_i32()) {
                                // Foreign key: value plus link to the named record.
                                if value::cell_widget(ui, v, true) {
                                    changed = true;
                                }
                                if idx != 0 {
                                    let name = db
                                        .collections
                                        .get(target)
                                        .and_then(|t| {
                                            t.records.iter().find(|r| t.index(r) == idx).map(|r| {
                                                NAME_PROPS
                                                    .iter()
                                                    .find_map(|n| {
                                                        t.get(r, n).and_then(Value::as_str)
                                                    })
                                                    .unwrap_or("")
                                                    .to_string()
                                            })
                                        })
                                        .unwrap_or_default();
                                    if ui.link(name).on_hover_text("open").clicked() {
                                        jump = Some((target, idx));
                                    }
                                }
                            } else if value::cell_widget(ui, v, true) {
                                changed = true;
                            }
                        });
                    }
                });
            });
        if let Some(pi) = sort_click {
            self.sort = match self.sort {
                Some((p, true)) if p == pi => Some((pi, false)),
                Some((p, false)) if p == pi => None,
                _ => Some((pi, true)),
            };
        }
        if let Some(r) = clicked_row {
            self.row = Some(r);
            self.form_text.clear();
        }
        if changed {
            // Widgets already mutated the value; keep one undo point per frame.
            if !self.dirty {
                self.dirty = true;
            }
            self.status = "Edited".into();
        }
        if let Some((target, idx)) = jump {
            if let Some(pos) = self.position_of_index(target, idx) {
                self.jump = Some((target, pos));
            }
        }
    }

    fn detail_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("detail")
            .default_size(420.0)
            .show(ui, |ui| {
                let Some(ri) = self.row else {
                    ui.heading("Details");
                    ui.label("Select a row with ▶ to edit it here.");
                    return;
                };
                if ri >= self.db.collections[self.selected].records.len() {
                    self.row = None;
                    return;
                }
                let short = self.db.collections[self.selected].short_name().to_string();
                let name = self.record_name(
                    self.selected,
                    &self.db.collections[self.selected].records[ri],
                );
                ui.heading(format!("{short}: {name}"));
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.preview(ui, &short, ri);
                    ui.separator();
                    self.form(ui, ri);
                    ui.separator();
                    self.relations(ui, ri);
                });
            });
    }

    fn preview(&mut self, ui: &mut egui::Ui, short: &str, ri: usize) {
        let c = &self.db.collections[self.selected];
        let r = &c.records[ri];
        let shown = match short {
            "ItemInfo" => {
                let image = c.int_or(r, "Image", 0) as u32;
                self.previews.show(ui, preview::STORE_ITEMS, image, 2.0)
            }
            "MagicInfo" => {
                let icon = c.int_or(r, "Icon", 0) as u32;
                self.previews.show(ui, preview::MAGIC_ICON, icon, 2.0)
            }
            "NPCInfo" => {
                let image = c.int_or(r, "Image", 0) as u32;
                self.previews.show(ui, preview::NPC, image * 100, 1.0)
            }
            "MonsterInfo" => {
                let image = c.int_or(r, "Image", 0) as u16;
                let dir = self.monster_direction;
                let shown = match Previews::monster_frame(image, dir) {
                    Some((lib, frame)) => self.previews.show(ui, lib, frame, 1.0),
                    None => false,
                };
                ui.horizontal(|ui| {
                    ui.label("Facing:");
                    ui.add(egui::Slider::new(&mut self.monster_direction, 0..=7));
                });
                shown
            }
            _ => return,
        };
        if !shown {
            ui.label("(no sprite for this image)");
        }
    }

    fn form(&mut self, ui: &mut egui::Ui, ri: usize) {
        let props: Vec<(String, String)> = self.db.collections[self.selected]
            .mapping
            .properties
            .iter()
            .map(|p| (p.name.clone(), p.type_name.clone()))
            .collect();
        let mut commit: Option<(usize, String)> = None;
        let mut changed = false;
        egui::Grid::new("form")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                for (pi, (name, ty)) in props.iter().enumerate() {
                    ui.label(name).on_hover_text(ty);
                    let v = &mut self.db.collections[self.selected].records[ri].values[pi];
                    match v {
                        Value::Bool(_) | Value::Int(_) | Value::UInt(_) | Value::Float(_) => {
                            if value::cell_widget(ui, v, true) {
                                changed = true;
                            }
                        }
                        Value::Str(_) => {
                            if value::cell_widget(ui, v, false) {
                                changed = true;
                            }
                        }
                        other => {
                            let text = self
                                .form_text
                                .entry(pi)
                                .or_insert_with(|| value::to_text(other));
                            let resp = ui
                                .add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY));
                            if resp.lost_focus() {
                                commit = Some((pi, text.clone()));
                            }
                        }
                    }
                    ui.end_row();
                }
            });
        if changed {
            self.dirty = true;
            self.status = "Edited".into();
        }
        if let Some((pi, text)) = commit {
            let c = &mut self.db.collections[self.selected];
            let current = &c.records[ri].values[pi];
            match value::from_text(current, &text) {
                Some(v) if v != *current => {
                    c.records[ri].values[pi] = v;
                    self.dirty = true;
                    self.form_error = None;
                    self.status = "Edited".into();
                }
                Some(_) => {}
                None => {
                    self.form_error = Some(format!("Cannot parse '{text}' for {}", props[pi].0));
                }
            }
        }
        if let Some(e) = &self.form_error {
            ui.colored_label(egui::Color32::from_rgb(220, 80, 80), e);
        }
    }

    /// Records in other collections that point at this one.
    fn relations(&mut self, ui: &mut egui::Ui, ri: usize) {
        let short = self.db.collections[self.selected].short_name().to_string();
        let index = self.db.collections[self.selected]
            .index(&self.db.collections[self.selected].records[ri]);
        let mut jump = None;
        for ci in 0..self.db.collections.len() {
            if ci == self.selected {
                continue;
            }
            let fk_props: Vec<usize> = self.db.collections[ci]
                .mapping
                .properties
                .iter()
                .enumerate()
                .filter(|(_, p)| p.type_name == "System.Int32")
                .filter(|(_, p)| {
                    self.fk_target(&p.name)
                        .map(|t| self.db.collections[t].short_name() == short)
                        .unwrap_or(false)
                })
                .map(|(i, _)| i)
                .collect();
            if fk_props.is_empty() {
                continue;
            }
            let c = &self.db.collections[ci];
            let hits: Vec<usize> = c
                .records
                .iter()
                .enumerate()
                .filter(|(_, r)| {
                    fk_props
                        .iter()
                        .any(|p| r.values[*p].as_i32() == Some(index))
                })
                .map(|(i, _)| i)
                .collect();
            if hits.is_empty() {
                continue;
            }
            ui.collapsing(format!("{} ({})", c.short_name(), hits.len()), |ui| {
                for h in hits.iter().take(200) {
                    let r = &c.records[*h];
                    let summary: Vec<String> = c
                        .mapping
                        .properties
                        .iter()
                        .zip(&r.values)
                        .filter(|(p, _)| {
                            p.name != "Index"
                                && !fk_props
                                    .contains(&c.property_index(&p.name).unwrap_or(usize::MAX))
                        })
                        .take(4)
                        .map(|(p, v)| format!("{}={}", p.name, value::summary(v)))
                        .collect();
                    if ui.link(summary.join("  ")).clicked() {
                        jump = Some((ci, *h));
                    }
                }
                if hits.len() > 200 {
                    ui.label(format!("… {} more", hits.len() - 200));
                }
            });
        }
        if let Some(j) = jump {
            self.jump = Some(j);
        }
    }
}
