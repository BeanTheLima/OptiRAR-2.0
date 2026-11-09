use eframe::egui::{self, Align2, Color32, FontId, Key, Modifiers, Rect, RichText, Sense, Vec2};
use optirar_core::t;
use optirar_core::types::format_bytes;
use optirar_index::catalog::ArchiveCatalog;
use std::collections::{HashMap, HashSet};

use crate::theme;

#[derive(Debug, Clone)]
pub struct TableDisplayItem {
    /// Name shown in the list (relative to the current folder; full path in search mode).
    pub name: String,
    /// Files: archive entry path. Folders: "a/b/" (trailing slash).
    pub full_path: String,
    pub is_dir: bool,
    pub is_parent_nav: bool,
    /// Catalog indices covered by this row (a folder covers all of its descendants).
    pub indices: Vec<usize>,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: Option<u32>,
    pub modified: Option<i64>,
    pub encrypted: bool,
}

impl TableDisplayItem {
    fn ratio(&self) -> f32 {
        if self.uncompressed_size > 0 {
            self.compressed_size as f32 / self.uncompressed_size as f32 * 100.0
        } else {
            100.0
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortColumn {
    #[default]
    Name,
    Size,
    Packed,
    Ratio,
    Modified,
    Crc,
}

/// Listing state for the archive explorer. The row list is rebuilt only when the
/// folder, search, sort order or catalog changes — never per frame.
#[derive(Default)]
pub struct TableState {
    pub nav_path: String,
    pub sort: SortColumn,
    pub descending: bool,
    pub items: Vec<TableDisplayItem>,
    anchor: Option<usize>,
    /// A row drag began; the OS drag starts once the pointer leaves the list (like Explorer).
    drag_armed: bool,
    cache_key: Option<(String, String, SortColumn, bool)>,
}

impl TableState {
    pub fn reset(&mut self) {
        self.nav_path.clear();
        self.items.clear();
        self.anchor = None;
        self.cache_key = None;
    }

    pub fn is_searching(&self) -> bool {
        self.cache_key.as_ref().map(|k| !k.1.is_empty()).unwrap_or(false)
    }

    /// Rows of the current view that are fully selected.
    pub fn selected_rows<'a>(&'a self, selected: &HashSet<usize>) -> Vec<&'a TableDisplayItem> {
        self.items
            .iter()
            .filter(|it| !it.is_parent_nav && is_row_selected(it, selected))
            .collect()
    }

    fn refresh(&mut self, cat: &ArchiveCatalog, search: &str) {
        let key = (self.nav_path.clone(), search.to_string(), self.sort, self.descending);
        if self.cache_key.as_ref() == Some(&key) {
            return;
        }
        self.items = build_items(cat, &self.nav_path, search);
        sort_items(&mut self.items, self.sort, self.descending);
        self.anchor = None;
        self.cache_key = Some(key);
    }
}

fn is_row_selected(item: &TableDisplayItem, selected: &HashSet<usize>) -> bool {
    !item.indices.is_empty() && item.indices.iter().all(|i| selected.contains(i))
}

pub fn parent_of(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    match trimmed.rfind('/') {
        Some(p) => format!("{}/", &trimmed[..p]),
        None => String::new(),
    }
}

fn file_item(idx: usize, e: &optirar_core::types::ArchiveEntry, name: String) -> TableDisplayItem {
    TableDisplayItem {
        name,
        full_path: e.path.clone(),
        is_dir: false,
        is_parent_nav: false,
        indices: vec![idx],
        uncompressed_size: e.uncompressed_size,
        compressed_size: e.compressed_size,
        crc32: e.crc32,
        modified: e.modified_timestamp,
        encrypted: e.is_encrypted,
    }
}

pub fn build_items(cat: &ArchiveCatalog, nav: &str, query: &str) -> Vec<TableDisplayItem> {
    let mut out = Vec::new();

    if !query.is_empty() {
        let q = query.to_lowercase();
        for (idx, e) in cat.entries.iter().enumerate() {
            if e.is_directory {
                continue;
            }
            let p = e.path.to_lowercase();
            let hit = match q.strip_prefix('*') {
                Some(suffix) => p.ends_with(suffix),
                None => p.contains(&q),
            };
            if hit {
                out.push(file_item(idx, e, e.path.replace('\\', "/")));
            }
        }
        return out;
    }

    let nav = nav.trim_matches('/');
    if !nav.is_empty() {
        out.push(TableDisplayItem {
            name: t("table.parent_dir"),
            full_path: parent_of(nav),
            is_dir: true,
            is_parent_nav: true,
            indices: Vec::new(),
            uncompressed_size: 0,
            compressed_size: 0,
            crc32: None,
            modified: None,
            encrypted: false,
        });
    }
    let prefix = if nav.is_empty() { String::new() } else { format!("{}/", nav) };
    let mut dir_pos: HashMap<String, usize> = HashMap::new();

    for (idx, e) in cat.entries.iter().enumerate() {
        let norm = e.path.replace('\\', "/");
        let clean = norm.trim_matches('/');
        let Some(rel) = clean.strip_prefix(prefix.as_str()) else { continue };
        if rel.is_empty() {
            continue;
        }
        let (head, is_sub) = match rel.find('/') {
            Some(p) => (&rel[..p], true),
            None => (rel, e.is_directory),
        };
        if is_sub {
            let pos = *dir_pos.entry(head.to_string()).or_insert_with(|| {
                out.push(TableDisplayItem {
                    name: head.to_string(),
                    full_path: format!("{}{}/", prefix, head),
                    is_dir: true,
                    is_parent_nav: false,
                    indices: Vec::new(),
                    uncompressed_size: 0,
                    compressed_size: 0,
                    crc32: None,
                    modified: None,
                    encrypted: false,
                });
                out.len() - 1
            });
            let d = &mut out[pos];
            d.indices.push(idx);
            d.uncompressed_size += e.uncompressed_size;
            d.compressed_size += e.compressed_size;
            d.modified = d.modified.max(e.modified_timestamp);
            d.encrypted |= e.is_encrypted;
        } else {
            out.push(file_item(idx, e, rel.to_string()));
        }
    }
    out
}

fn sort_items(items: &mut [TableDisplayItem], col: SortColumn, descending: bool) {
    items.sort_by(|a, b| {
        use std::cmp::Ordering;
        if a.is_parent_nav != b.is_parent_nav {
            return if a.is_parent_nav { Ordering::Less } else { Ordering::Greater };
        }
        if a.is_dir != b.is_dir {
            return b.is_dir.cmp(&a.is_dir);
        }
        let ord = match col {
            SortColumn::Name => natural_cmp(&a.name, &b.name),
            SortColumn::Size => a.uncompressed_size.cmp(&b.uncompressed_size),
            SortColumn::Packed => a.compressed_size.cmp(&b.compressed_size),
            SortColumn::Ratio => a.ratio().partial_cmp(&b.ratio()).unwrap_or(Ordering::Equal),
            SortColumn::Modified => a.modified.cmp(&b.modified),
            SortColumn::Crc => a.crc32.cmp(&b.crc32),
        };
        let ord = ord.then_with(|| natural_cmp(&a.name, &b.name));
        if descending { ord.reverse() } else { ord }
    });
}

/// Case-insensitive compare that orders "file2" before "file10" like Explorer.
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut ai, mut bi) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, _) => return std::cmp::Ordering::Less,
            (_, None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let mut na = String::new();
                while let Some(c) = ai.peek().copied().filter(|c| c.is_ascii_digit()) {
                    na.push(c);
                    ai.next();
                }
                let mut nb = String::new();
                while let Some(c) = bi.peek().copied().filter(|c| c.is_ascii_digit()) {
                    nb.push(c);
                    bi.next();
                }
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase());
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
                ai.next();
                bi.next();
            }
        }
    }
}

/// "YYYY-MM-DD HH:MM" from UNIX seconds (UTC-less local DOS time as stored in archives).
pub fn format_timestamp(ts: i64) -> String {
    let days = ts.div_euclid(86400);
    let secs = ts.rem_euclid(86400);
    // civil_from_days (Howard Hinnant)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, m, d, secs / 3600, (secs % 3600) / 60)
}

#[derive(Default)]
pub struct TableAction {
    pub navigate_to: Option<String>,
    /// Double-click / Enter: built-in viewer for code, text and images, else the default app.
    pub open_entry: Option<usize>,
    /// Always the built-in viewer/editor.
    pub view_entry: Option<usize>,
    /// Always the system's default application.
    pub open_external: Option<usize>,
    pub delete_selected: bool,
    pub extract_selected_dialog: bool,
    pub extract_selected_here: bool,
    pub extract_selected_subfolder: bool,
    pub extract_all_dialog: bool,
    pub start_drag: bool,
    pub show_properties: bool,
}

const COLS: [(SortColumn, &str, f32); 5] = [
    (SortColumn::Size, "table.size", 96.0),
    (SortColumn::Packed, "table.packed", 96.0),
    (SortColumn::Ratio, "table.ratio", 60.0),
    (SortColumn::Modified, "table.date", 128.0),
    (SortColumn::Crc, "table.crc32", 80.0),
];
const ROW_H: f32 = 24.0;
const PAD: f32 = 6.0;

pub struct TableView;

impl TableView {
    pub fn show(
        ui: &mut egui::Ui,
        cat: &ArchiveCatalog,
        state: &mut TableState,
        search_query: &str,
        selected: &mut HashSet<usize>,
        archive_stem: &str,
        writable: bool,
    ) -> TableAction {
        let mut action = TableAction::default();
        state.refresh(cat, search_query);

        // 1. Breadcrumbs
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let up_enabled = !state.nav_path.is_empty() && search_query.is_empty();
            if ui.add_enabled(up_enabled, egui::Button::new("⬆")).on_hover_text(t("explorer.up")).clicked() {
                action.navigate_to = Some(parent_of(&state.nav_path));
            }
            ui.add_space(6.0);
            if ui.link(RichText::new(format!("📦 {}", archive_stem)).strong()).clicked() {
                action.navigate_to = Some(String::new());
            }
            let mut accumulated = String::new();
            for part in state.nav_path.trim_matches('/').split('/').filter(|p| !p.is_empty()) {
                accumulated.push_str(part);
                accumulated.push('/');
                ui.label(RichText::new(" › ").color(theme::weak_text(ui)));
                if ui.link(part).clicked() {
                    action.navigate_to = Some(accumulated.clone());
                }
            }
            if !search_query.is_empty() {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(optirar_core::i18n::t_format(
                        "explorer.search_results",
                        &[("query", search_query), ("count", &state.items.len().to_string())],
                    ))
                    .color(theme::WARNING),
                );
            }
        });
        ui.add_space(2.0);

        let full_w = ui.available_width();
        let fixed: f32 = COLS.iter().map(|c| c.2).sum();
        let name_w = (full_w - fixed - PAD * 2.0).max(160.0);
        let visuals = ui.visuals().clone();
        let text_color = visuals.text_color();
        let weak = visuals.weak_text_color();
        let font = FontId::proportional(13.5);

        // 2. Header (click to sort)
        let (hdr_rect, _) = ui.allocate_exact_size(Vec2::new(full_w, ROW_H), Sense::hover());
        ui.painter().rect_filled(hdr_rect, 0.0, visuals.faint_bg_color);
        let mut x = hdr_rect.left() + PAD;
        let mut header_cols: Vec<(SortColumn, String, f32)> = vec![(SortColumn::Name, t("table.name"), name_w)];
        header_cols.extend(COLS.iter().map(|(c, k, w)| (*c, t(k), *w)));
        for (col, label, w) in header_cols {
            let r = Rect::from_min_size(egui::pos2(x, hdr_rect.top()), Vec2::new(w, ROW_H));
            let resp = ui.interact(r, ui.id().with(("hdr", col as u8)), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, 0.0, visuals.widgets.hovered.weak_bg_fill);
            }
            let arrow = if state.sort == col { if state.descending { " ▼" } else { " ▲" } } else { "" };
            let align = if col == SortColumn::Name { Align2::LEFT_CENTER } else { Align2::RIGHT_CENTER };
            let tx = if col == SortColumn::Name { r.left() + 4.0 } else { r.right() - 8.0 };
            ui.painter().with_clip_rect(r).text(
                egui::pos2(tx, r.center().y),
                align,
                format!("{}{}", label, arrow),
                FontId::proportional(13.0),
                text_color,
            );
            if resp.clicked() {
                if state.sort == col {
                    state.descending = !state.descending;
                } else {
                    state.sort = col;
                    state.descending = false;
                }
                state.cache_key = None;
            }
            x += w;
        }
        ui.painter().hline(hdr_rect.x_range(), hdr_rect.bottom(), visuals.widgets.noninteractive.bg_stroke);

        // 3. Keyboard shortcuts for the list (only when no text field has focus)
        let no_focus = ui.ctx().memory(|m| m.focused().is_none());
        if no_focus {
            let (enter, back, select_all, delete) = ui.ctx().input_mut(|i| {
                (
                    i.consume_key(Modifiers::NONE, Key::Enter),
                    i.consume_key(Modifiers::NONE, Key::Backspace),
                    i.consume_key(Modifiers::COMMAND, Key::A),
                    i.consume_key(Modifiers::NONE, Key::Delete),
                )
            });
            if delete && writable && !selected.is_empty() {
                action.delete_selected = true;
            }
            if select_all {
                for it in &state.items {
                    selected.extend(it.indices.iter().copied());
                }
            }
            if back && !state.nav_path.is_empty() {
                action.navigate_to = Some(parent_of(&state.nav_path));
            }
            if enter {
                if let Some(it) = state.selected_rows(selected).first() {
                    if it.is_dir {
                        action.navigate_to = Some(it.full_path.clone());
                    } else {
                        action.open_entry = it.indices.first().copied();
                    }
                }
            }
        }

        // 4. Virtualised rows
        let items = &state.items;
        let modifiers = ui.ctx().input(|i| i.modifiers);
        let mut new_anchor = None;
        let mut arm_drag = false;
        let mut list_rect = egui::Rect::NOTHING;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let scroll = egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show_rows(ui, ROW_H, items.len(), |ui, range| {
                    for i in range {
                        let item = &items[i];
                        let is_sel = !item.is_parent_nav && is_row_selected(item, selected);
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(full_w, ROW_H), Sense::click_and_drag());

                        let bg = if is_sel {
                            visuals.selection.bg_fill
                        } else if resp.hovered() {
                            visuals.widgets.hovered.weak_bg_fill
                        } else if i % 2 == 1 {
                            visuals.faint_bg_color
                        } else {
                            Color32::TRANSPARENT
                        };
                        ui.painter().rect_filled(rect, 0.0, bg);
                        let fg = if is_sel { visuals.selection.stroke.color } else { text_color };

                        // Name column
                        let name_rect = Rect::from_min_size(egui::pos2(rect.left() + PAD, rect.top()), Vec2::new(name_w, ROW_H));
                        let p = ui.painter().with_clip_rect(name_rect.shrink2(Vec2::new(0.0, 1.0)));
                        let (icon, icon_color) = if item.is_parent_nav {
                            ("⤴", weak)
                        } else if item.is_dir {
                            ("📁", Color32::from_rgb(230, 175, 50))
                        } else {
                            ("📄", weak)
                        };
                        p.text(egui::pos2(name_rect.left() + 2.0, rect.center().y), Align2::LEFT_CENTER, icon, font.clone(), icon_color);
                        let lock = if item.encrypted { "🔒 " } else { "" };
                        p.text(
                            egui::pos2(name_rect.left() + 24.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            format!("{}{}", lock, item.name),
                            font.clone(),
                            fg,
                        );

                        // Numeric columns
                        if !item.is_parent_nav {
                            let cells = [
                                format_bytes(item.uncompressed_size),
                                format_bytes(item.compressed_size),
                                format!("{:.0}%", item.ratio()),
                                item.modified.map(format_timestamp).unwrap_or_default(),
                                if item.is_dir { String::new() } else { item.crc32.map(|c| format!("{:08X}", c)).unwrap_or_default() },
                            ];
                            let mut cx = name_rect.right();
                            for ((_, _, w), text) in COLS.iter().zip(cells) {
                                cx += w;
                                ui.painter().text(
                                    egui::pos2(cx - 8.0, rect.center().y),
                                    Align2::RIGHT_CENTER,
                                    text,
                                    FontId::proportional(12.5),
                                    if is_sel { fg } else { weak },
                                );
                            }
                        }

                        // Interaction
                        let select_only = |sel: &mut HashSet<usize>| {
                            sel.clear();
                            sel.extend(item.indices.iter().copied());
                        };

                        if resp.double_clicked() {
                            if item.is_dir {
                                action.navigate_to = Some(item.full_path.clone());
                            } else {
                                action.open_entry = item.indices.first().copied();
                            }
                        } else if resp.clicked() && !item.is_parent_nav {
                            if modifiers.command {
                                if is_sel {
                                    for idx in &item.indices {
                                        selected.remove(idx);
                                    }
                                } else {
                                    selected.extend(item.indices.iter().copied());
                                }
                                new_anchor = Some(i);
                            } else if modifiers.shift && state.anchor.is_some() {
                                let a = state.anchor.unwrap_or(i);
                                selected.clear();
                                for it in &items[a.min(i)..=a.max(i)] {
                                    selected.extend(it.indices.iter().copied());
                                }
                            } else {
                                select_only(selected);
                                new_anchor = Some(i);
                            }
                        } else if resp.clicked() && item.is_parent_nav {
                            selected.clear();
                        }

                        if resp.secondary_clicked() && !item.is_parent_nav && !is_sel {
                            select_only(selected);
                            new_anchor = Some(i);
                        }

                        if resp.drag_started() && !item.is_parent_nav {
                            if !is_sel {
                                select_only(selected);
                            }
                            arm_drag = true;
                        }

                        if !item.is_parent_nav {
                            resp.context_menu(|ui| {
                                Self::row_menu(ui, item, archive_stem, writable, &mut action);
                            });
                        }
                    }
                });

            list_rect = scroll.inner_rect;
            // Blank area below rows: clear selection / generic menu
            let blank = Rect::from_min_max(
                egui::pos2(scroll.inner_rect.left(), scroll.inner_rect.top() + scroll.content_size.y),
                scroll.inner_rect.max,
            );
            if blank.height() > 4.0 {
                let resp = ui.interact(blank, ui.id().with("blank"), Sense::click());
                if resp.clicked() {
                    selected.clear();
                }
                resp.context_menu(|ui| {
                    if ui.button(format!("☑ {}", t("ctx.select_all"))).clicked() {
                        for it in items {
                            selected.extend(it.indices.iter().copied());
                        }
                        ui.close_menu();
                    }
                    if ui.button(format!("📦 {}", t("toolbar.extract_all"))).clicked() {
                        action.extract_all_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button(format!("ℹ {}", t("ctx.properties"))).clicked() {
                        action.show_properties = true;
                        ui.close_menu();
                    }
                });
            }
        });
        if new_anchor.is_some() {
            state.anchor = new_anchor;
        }
        if arm_drag {
            state.drag_armed = true;
        }
        if state.drag_armed {
            let (down, pos) = ui.ctx().input(|i| (i.pointer.primary_down(), i.pointer.latest_pos()));
            if !down {
                state.drag_armed = false;
            } else if pos.is_some_and(|p| !list_rect.contains(p)) {
                state.drag_armed = false;
                action.start_drag = true;
            }
        }

        action
    }

    fn row_menu(ui: &mut egui::Ui, item: &TableDisplayItem, archive_stem: &str, writable: bool, action: &mut TableAction) {
        ui.set_min_width(240.0);
        if item.is_dir {
            if ui.button(RichText::new(format!("📂 {}", t("ctx.open_folder"))).strong()).clicked() {
                action.navigate_to = Some(item.full_path.clone());
                ui.close_menu();
            }
        } else {
            if ui.button(RichText::new(format!("📝 {}", t("ctx.view_edit"))).strong()).clicked() {
                action.view_entry = item.indices.first().copied();
                ui.close_menu();
            }
            if ui.button(format!("🗁 {}", t("ctx.open_external"))).clicked() {
                action.open_external = item.indices.first().copied();
                ui.close_menu();
            }
        }
        ui.separator();
        if ui.button(format!("📤 {}", t("ctx.extract_to"))).clicked() {
            action.extract_selected_dialog = true;
            ui.close_menu();
        }
        if ui.button(format!("📥 {}", t("ctx.extract_here"))).clicked() {
            action.extract_selected_here = true;
            ui.close_menu();
        }
        let sub_label = optirar_core::i18n::t_format("ctx.extract_to_folder", &[("name", archive_stem)]);
        if ui.button(format!("📁 {}", sub_label)).clicked() {
            action.extract_selected_subfolder = true;
            ui.close_menu();
        }
        ui.separator();
        if ui.button(format!("📋 {}", t("ctx.copy_name"))).clicked() {
            ui.ctx().copy_text(item.name.clone());
            ui.close_menu();
        }
        if ui.button(format!("📋 {}", t("ctx.copy_path"))).clicked() {
            ui.ctx().copy_text(item.full_path.clone());
            ui.close_menu();
        }
        if writable {
            ui.separator();
            if ui.button(RichText::new(format!("🗑 {}", t("ctx.delete"))).color(theme::ERROR)).clicked() {
                action.delete_selected = true;
                ui.close_menu();
            }
        }
        ui.separator();
        if ui.button(format!("ℹ {}", t("ctx.properties"))).clicked() {
            action.show_properties = true;
            ui.close_menu();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use optirar_core::types::{ArchiveEntry, ArchiveFormat};

    fn entry(path: &str, dir: bool) -> ArchiveEntry {
        ArchiveEntry {
            index: 0,
            path: path.into(),
            uncompressed_size: 10,
            compressed_size: 5,
            is_directory: dir,
            is_encrypted: false,
            crc32: None,
            hash_hex: None,
            modified_timestamp: None,
            windows_attributes: None,
            unix_mode: None,
        }
    }

    #[test]
    fn folders_are_merged_with_explicit_dir_entries() {
        let cat = ArchiveCatalog::new(
            ArchiveFormat::Zip,
            vec![entry("a/", true), entry("a/x.txt", false), entry("a/b/y.txt", false), entry("z.txt", false)],
            false,
        );
        let root = build_items(&cat, "", "");
        assert_eq!(root.len(), 2);
        let a = root.iter().find(|i| i.name == "a").unwrap();
        assert!(a.is_dir);
        assert_eq!(a.indices, vec![0, 1, 2]);
        assert_eq!(a.uncompressed_size, 30);

        let inside = build_items(&cat, "a/", "");
        let names: Vec<_> = inside.iter().filter(|i| !i.is_parent_nav).map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["x.txt", "b"]);
        assert_eq!(parent_of("a/b/"), "a/");
        assert_eq!(parent_of("a/"), "");
    }

    #[test]
    fn natural_sort_and_dates() {
        assert_eq!(natural_cmp("file2", "file10"), std::cmp::Ordering::Less);
        assert_eq!(format_timestamp(1709296210), "2024-03-01 12:30");
    }
}
