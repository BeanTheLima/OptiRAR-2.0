//! Built-in viewer/editor for archive entries: syntax-highlighted code with line numbers,
//! outline, find and save-back; images; hex dump for binaries.

use eframe::egui::{self, text::CCursor, text::CCursorRange, Key, Modifiers, RichText, Vec2};
use egui_extras::syntax_highlighting::{self, CodeTheme};
use optirar_core::i18n::t_format;
use optirar_core::t;
use optirar_core::types::format_bytes;

use crate::theme;

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp", "webp", "ico"];
/// Above this size text is shown read-only without highlighting (TextEdit gets slow).
const MAX_EDIT_BYTES: usize = 2 * 1024 * 1024;
const MAX_HIGHLIGHT_BYTES: usize = 400 * 1024;

pub fn ext_of(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((_, e)) => e.to_lowercase(),
        None => name.to_lowercase(), // Makefile, Dockerfile...
    }
}

pub fn is_image(path: &str) -> bool {
    IMAGE_EXTS.contains(&ext_of(path).as_str())
}

/// Files opened in the built-in viewer on double-click (everything else goes to the default app).
pub fn prefers_builtin(path: &str) -> bool {
    const TEXT: &[&str] = &[
        "txt", "md", "log", "ini", "cfg", "conf", "toml", "yaml", "yml", "json", "xml", "csv", "py", "pyw", "c", "h",
        "cpp", "cc", "cxx", "hpp", "hh", "cs", "java", "kt", "go", "rs", "js", "mjs", "jsx", "ts", "tsx", "html", "htm",
        "css", "scss", "php", "rb", "lua", "sh", "bat", "cmd", "ps1", "sql", "swift", "m", "pl", "r", "dart", "vue",
        "makefile", "dockerfile", "gradle", "properties", "gitignore", "env", "svg",
    ];
    is_image(path) || TEXT.contains(&ext_of(path).as_str())
}

fn language_name(ext: &str) -> &'static str {
    match ext {
        "py" | "pyw" => "Python",
        "c" => "C",
        "h" | "hpp" | "hh" => "C/C++ Header",
        "cpp" | "cc" | "cxx" => "C++",
        "cs" => "C#",
        "java" => "Java",
        "kt" => "Kotlin",
        "go" => "Go",
        "rs" => "Rust",
        "js" | "mjs" | "jsx" => "JavaScript",
        "ts" | "tsx" => "TypeScript",
        "html" | "htm" => "HTML",
        "css" | "scss" => "CSS",
        "json" => "JSON",
        "xml" | "svg" => "XML",
        "md" => "Markdown",
        "sh" => "Shell",
        "bat" | "cmd" => "Batch",
        "ps1" => "PowerShell",
        "sql" => "SQL",
        "php" => "PHP",
        "rb" => "Ruby",
        "lua" => "Lua",
        "yaml" | "yml" => "YAML",
        "toml" | "ini" | "cfg" | "conf" => "Config",
        _ => "Text",
    }
}

/// syntect has no TypeScript/Kotlin grammar by default: borrow the closest one.
fn syntax_token(ext: &str) -> &str {
    match ext {
        "ts" | "tsx" | "mjs" | "jsx" | "vue" => "js",
        "kt" | "gradle" => "java",
        "hh" | "hpp" | "cxx" => "cpp",
        "pyw" => "py",
        "htm" => "html",
        "scss" => "css",
        "yml" => "yaml",
        "ps1" | "cmd" => "bat",
        "toml" | "cfg" | "conf" | "properties" | "env" => "ini",
        "svg" => "xml",
        other => other,
    }
}

pub enum ViewerContent {
    Text {
        text: String,
        saved: String,
        /// Decoded with replacement characters: saving would corrupt the file.
        lossy: bool,
        crlf: bool,
        bom: bool,
    },
    Image {
        bytes: egui::load::Bytes,
    },
    Binary {
        hex: String,
    },
}

pub fn classify(path: &str, bytes: Vec<u8>) -> ViewerContent {
    if is_image(path) {
        return ViewerContent::Image { bytes: egui::load::Bytes::from(bytes) };
    }
    let head = &bytes[..bytes.len().min(8192)];
    let utf16 = bytes.starts_with(&[0xFF, 0xFE]);
    if !head.contains(&0) || utf16 {
        let bom = bytes.starts_with(&[0xEF, 0xBB, 0xBF]);
        let (text, lossy) = if utf16 {
            let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            (String::from_utf16_lossy(&units), true)
        } else {
            let body = if bom { &bytes[3..] } else { &bytes[..] };
            match std::str::from_utf8(body) {
                Ok(s) => (s.to_string(), false),
                Err(_) => (String::from_utf8_lossy(body).into_owned(), true),
            }
        };
        let crlf = text.contains("\r\n");
        let text = if crlf { text.replace("\r\n", "\n") } else { text };
        return ViewerContent::Text { saved: text.clone(), text, lossy, crlf, bom };
    }
    ViewerContent::Binary { hex: hex_dump(&bytes[..bytes.len().min(64 * 1024)]) }
}

fn hex_dump(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 4);
    for (i, chunk) in bytes.chunks(16).enumerate() {
        out.push_str(&format!("{:08X}  ", i * 16));
        for j in 0..16 {
            match chunk.get(j) {
                Some(b) => out.push_str(&format!("{:02X} ", b)),
                None => out.push_str("   "),
            }
        }
        out.push(' ');
        out.extend(chunk.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' }));
        out.push('\n');
    }
    out
}

/// Functions / classes / headings, for the outline panel: (0-based line, label).
pub fn outline(ext: &str, text: &str) -> Vec<(usize, String)> {
    fn ident(s: &str) -> String {
        s.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':' || *c == '~').collect()
    }
    fn strip_any<'a>(mut s: &'a str, words: &[&str]) -> &'a str {
        loop {
            let before = s;
            for w in words {
                if let Some(r) = s.strip_prefix(w) {
                    s = r.trim_start();
                }
            }
            if s == before {
                return s;
            }
        }
    }
    const C_KEYWORDS: &[&str] = &["if", "for", "while", "switch", "return", "else", "do", "case", "sizeof", "catch"];

    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if out.len() >= 3000 {
            break;
        }
        let t = line.trim_start();
        let top_level = t.len() == line.len();
        let sym = match ext {
            "py" | "pyw" => {
                let t = strip_any(t, &["async "]);
                if let Some(r) = t.strip_prefix("def ") {
                    Some(format!("{}ƒ {}()", "  ".repeat((line.len() - t.len()) / 4), ident(r)))
                } else {
                    t.strip_prefix("class ").map(|r| format!("{}◆ {}", "  ".repeat((line.len() - t.len()) / 4), ident(r).trim_end_matches(':')))
                }
            }
            "rs" => {
                let t = strip_any(t, &["pub(crate) ", "pub(super) ", "pub ", "async ", "unsafe ", "const ", "extern \"C\" "]);
                if let Some(r) = t.strip_prefix("fn ") {
                    Some(format!("ƒ {}()", ident(r)))
                } else {
                    ["struct ", "enum ", "trait ", "impl", "mod "]
                        .iter()
                        .find(|k| t.starts_with(**k))
                        .map(|_| format!("◆ {}", t.trim_end_matches(['{', ' ', ';']).trim()))
                }
            }
            "js" | "mjs" | "jsx" | "ts" | "tsx" => {
                let t = strip_any(t, &["export ", "default ", "async "]);
                if let Some(r) = t.strip_prefix("function ") {
                    Some(format!("ƒ {}()", ident(r.trim_start_matches('*').trim_start())))
                } else if let Some(r) = t.strip_prefix("class ") {
                    Some(format!("◆ {}", ident(r)))
                } else if let Some(r) = t.strip_prefix("const ").filter(|r| r.contains("=>")) {
                    Some(format!("ƒ {}", ident(r)))
                } else {
                    None
                }
            }
            "go" => t.strip_prefix("func ").map(|r| format!("ƒ {}", r.trim_end_matches(['{', ' ']))),
            "md" => t.starts_with('#').then(|| t.to_string()),
            "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "hh" | "cs" | "java" | "kt" => {
                if let Some(r) = ["class ", "struct ", "namespace ", "interface ", "enum "].iter().find_map(|k| strip_any(t, &["public ", "private ", "static ", "typedef "]).strip_prefix(k)) {
                    (!t.ends_with(';')).then(|| format!("◆ {}", ident(r)))
                } else if (top_level || matches!(ext, "cs" | "java" | "kt"))
                    && t.contains('(')
                    && !t.ends_with(';')
                    && !t.starts_with(['#', '/', '*', '}', '{'])
                    && !t.contains('=')
                {
                    let before = t[..t.find('(').unwrap_or(0)].trim_end();
                    let name = before.rsplit([' ', '*', '&']).next().unwrap_or("");
                    (!name.is_empty() && !C_KEYWORDS.contains(&name) && name.chars().all(|c| c.is_alphanumeric() || "_:~".contains(c)))
                        .then(|| format!("ƒ {}()", name))
                } else {
                    None
                }
            }
            _ => None,
        };
        if let Some(s) = sym {
            out.push((i, s));
        }
    }
    out
}

#[derive(Default)]
pub struct ViewerAction {
    pub save_to_archive: Option<Vec<u8>>,
    pub save_as: Option<Vec<u8>>,
    pub open_external: bool,
}

pub struct ViewerState {
    pub open: bool,
    pub index: usize,
    pub path: String,
    pub size: u64,
    pub truncated: bool,
    pub content: ViewerContent,
    /// Archive format supports writing the entry back (ZIP).
    pub archive_writable: bool,
    ext: String,
    wrap: bool,
    real_size: bool,
    find: String,
    find_from: usize,
    find_count: usize,
    outline: Vec<(usize, String)>,
    outline_dirty: bool,
    pending_select: Option<(usize, usize)>,
    focus_find: bool,
}

impl ViewerState {
    pub fn new(index: usize, path: String, size: u64, truncated: bool, bytes: Vec<u8>, archive_writable: bool) -> Self {
        let mut content = classify(&path, bytes);
        if let ViewerContent::Text { text, saved, .. } = &mut content {
            if text.len() > MAX_EDIT_BYTES {
                let mut cut = MAX_EDIT_BYTES;
                while !text.is_char_boundary(cut) {
                    cut -= 1;
                }
                text.truncate(cut);
                saved.truncate(cut);
            }
        }
        Self {
            open: true,
            index,
            ext: ext_of(&path),
            path,
            size,
            truncated,
            content,
            archive_writable,
            wrap: false,
            real_size: false,
            find: String::new(),
            find_from: 0,
            find_count: 0,
            outline: Vec::new(),
            outline_dirty: true,
            pending_select: None,
            focus_find: false,
        }
    }

    fn read_only(&self) -> bool {
        match &self.content {
            ViewerContent::Text { lossy, text, .. } => *lossy || self.truncated || text.len() >= MAX_EDIT_BYTES,
            _ => true,
        }
    }

    pub fn is_modified(&self) -> bool {
        matches!(&self.content, ViewerContent::Text { text, saved, .. } if text != saved)
    }

    pub fn mark_saved(&mut self) {
        if let Some(len) = self.encoded().map(|b| b.len() as u64) {
            self.size = len;
        }
        if let ViewerContent::Text { text, saved, .. } = &mut self.content {
            *saved = text.clone();
        }
    }

    fn encoded(&self) -> Option<Vec<u8>> {
        match &self.content {
            ViewerContent::Text { text, crlf, bom, .. } => {
                let body = if *crlf { text.replace('\n', "\r\n") } else { text.clone() };
                let mut out = if *bom { vec![0xEF, 0xBB, 0xBF] } else { Vec::new() };
                out.extend_from_slice(body.as_bytes());
                Some(out)
            }
            _ => None,
        }
    }

    /// Select the next match of `find` after the cursor (case-insensitive).
    fn find_next(&mut self) {
        let ViewerContent::Text { text, .. } = &self.content else { return };
        if self.find.is_empty() {
            return;
        }
        let hay: Vec<char> = text.chars().flat_map(|c| c.to_lowercase().next()).collect();
        let needle: Vec<char> = self.find.chars().flat_map(|c| c.to_lowercase().next()).collect();
        let positions: Vec<usize> = hay.windows(needle.len()).enumerate().filter(|(_, w)| *w == needle.as_slice()).map(|(i, _)| i).collect();
        self.find_count = positions.len();
        if let Some(&p) = positions.iter().find(|&&p| p >= self.find_from).or(positions.first()) {
            self.pending_select = Some((p, p + needle.len()));
            self.find_from = p + 1;
        }
    }

    fn goto_line(&mut self, line: usize) {
        let ViewerContent::Text { text, .. } = &self.content else { return };
        let start: usize = text.split('\n').take(line).map(|l| l.chars().count() + 1).sum();
        let len = text.split('\n').nth(line).map(|l| l.chars().count()).unwrap_or(0);
        self.pending_select = Some((start, start + len));
    }

    pub fn show(&mut self, ctx: &egui::Context) -> ViewerAction {
        let mut action = ViewerAction::default();
        if !self.open {
            return action;
        }
        let modified = self.is_modified();
        let title = format!("{}{}", if modified { "● " } else { "" }, self.path.rsplit('/').next().unwrap_or(&self.path));
        let mut open = true;

        egui::Window::new(title)
            .id(egui::Id::new("optirar_viewer"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(960.0, 640.0))
            .show(ctx, |ui| {
                self.toolbar(ui, &mut action);
                ui.separator();
                match &mut self.content {
                    ViewerContent::Image { bytes } => {
                        let uri = format!("bytes://optirar/{}/{}", self.index, self.path);
                        let img = egui::Image::from_bytes(uri, bytes.clone());
                        if self.real_size {
                            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                                ui.add(img.fit_to_original_size(1.0));
                            });
                        } else {
                            // Fit inside the window without making the window grow.
                            let avail = ui.available_size().min(Vec2::new(940.0, 560.0)).max(Vec2::splat(64.0));
                            ui.centered_and_justified(|ui| {
                                ui.add(img.max_size(avail).maintain_aspect_ratio(true));
                            });
                        }
                    }
                    ViewerContent::Binary { hex } => {
                        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                            ui.add(egui::Label::new(RichText::new(hex.as_str()).monospace()).extend());
                        });
                    }
                    ViewerContent::Text { .. } => self.text_body(ui),
                }
            });

        // Ctrl+S / Ctrl+F while the viewer is open
        let (save, find) = ctx.input_mut(|i| (i.consume_key(Modifiers::COMMAND, Key::S), i.consume_key(Modifiers::COMMAND, Key::F)));
        if save && self.archive_writable && !self.read_only() && self.is_modified() {
            action.save_to_archive = self.encoded();
        }
        if find {
            self.focus_find = true;
        }

        if !open {
            self.open = false;
        }
        action
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, action: &mut ViewerAction) {
        ui.horizontal(|ui| {
            let kind = match &self.content {
                ViewerContent::Text { .. } => language_name(&self.ext).to_string(),
                ViewerContent::Image { .. } => t("viewer.image"),
                ViewerContent::Binary { .. } => t("viewer.binary"),
            };
            ui.label(RichText::new(kind).strong().color(theme::accent_text(ui)));
            ui.label(RichText::new(format_bytes(self.size)).color(theme::weak_text(ui)));
            if let ViewerContent::Text { text, lossy, .. } = &self.content {
                ui.label(RichText::new(t_format("viewer.lines", &[("n", &(text.matches('\n').count() + 1).to_string())])).color(theme::weak_text(ui)));
                if *lossy {
                    ui.label(RichText::new(t("viewer.lossy")).color(theme::WARNING));
                }
            }
            if self.truncated {
                ui.label(RichText::new(t("viewer.truncated")).color(theme::WARNING));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(format!("🗁 {}", t("viewer.open_external"))).clicked() {
                    action.open_external = true;
                }
                let bytes = match &self.content {
                    ViewerContent::Image { bytes } => Some(bytes.to_vec()),
                    _ => self.encoded(),
                };
                if bytes.is_some() && ui.button(format!("💾 {}", t("viewer.save_as"))).clicked() {
                    action.save_as = bytes;
                }
                if matches!(self.content, ViewerContent::Text { .. }) {
                    let can = self.archive_writable && !self.read_only() && self.is_modified();
                    let tip = if !self.archive_writable { t("viewer.save_zip_only") } else { t("viewer.save_tip") };
                    let btn = egui::Button::new(RichText::new(format!("💾 {}", t("viewer.save"))).strong());
                    if ui.add_enabled(can, btn).on_hover_text(tip).on_disabled_hover_text(t("viewer.save_zip_only")).clicked() {
                        action.save_to_archive = self.encoded();
                    }
                    ui.checkbox(&mut self.wrap, t("viewer.wrap"));
                    let resp = ui.add(egui::TextEdit::singleline(&mut self.find).desired_width(160.0).hint_text(format!("🔍 {} (Ctrl+F)", t("viewer.find"))));
                    if std::mem::take(&mut self.focus_find) {
                        resp.request_focus();
                    }
                    if resp.changed() {
                        self.find_from = 0;
                        self.find_count = 0;
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        self.find_next();
                        resp.request_focus();
                    }
                    if !self.find.is_empty() && self.find_count > 0 {
                        ui.label(RichText::new(t_format("viewer.matches", &[("n", &self.find_count.to_string())])).color(theme::weak_text(ui)));
                    }
                } else if matches!(self.content, ViewerContent::Image { .. }) {
                    ui.checkbox(&mut self.real_size, t("viewer.real_size"));
                }
            });
        });
    }

    fn text_body(&mut self, ui: &mut egui::Ui) {
        if self.outline_dirty {
            if let ViewerContent::Text { text, .. } = &self.content {
                self.outline = outline(&self.ext, text);
            }
            self.outline_dirty = false;
        }

        let mut goto = None;
        if !self.outline.is_empty() {
            egui::SidePanel::left("viewer_outline").resizable(true).default_width(190.0).show_inside(ui, |ui| {
                ui.label(RichText::new(t("viewer.outline")).strong());
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for (line, label) in &self.outline {
                        if ui.add(egui::Label::new(RichText::new(label).monospace().size(12.0)).truncate().sense(egui::Sense::click())).on_hover_text(format!("{} {}", t("viewer.line"), line + 1)).clicked() {
                            goto = Some(*line);
                        }
                    }
                });
            });
        }
        if let Some(line) = goto {
            self.goto_line(line);
        }

        let read_only = self.read_only();
        let wrap = self.wrap;
        let lang = syntax_token(&self.ext).to_string();
        let pending = self.pending_select.take();
        let ViewerContent::Text { text, .. } = &mut self.content else { return };
        let highlight = text.len() <= MAX_HIGHLIGHT_BYTES;
        let theme = CodeTheme::from_memory(ui.ctx(), ui.style());

        let mut layouter = |ui: &egui::Ui, code: &str, wrap_width: f32| {
            let mut job = if highlight {
                syntax_highlighting::highlight(ui.ctx(), ui.style(), &theme, code, &lang)
            } else {
                egui::text::LayoutJob::simple(code.to_owned(), egui::TextStyle::Monospace.resolve(ui.style()), ui.visuals().text_color(), f32::INFINITY)
            };
            job.wrap.max_width = if wrap { wrap_width } else { f32::INFINITY };
            ui.fonts(|f| f.layout_job(job))
        };

        let mut changed = false;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    if !wrap {
                        let lines = text.matches('\n').count() + 1;
                        let nums: String = (1..=lines).map(|n| format!("{n}\n")).collect();
                        let nums = nums.trim_end().to_string();
                        egui::Frame::NONE.inner_margin(egui::Margin::symmetric(0, 2)).show(ui, |ui| {
                            ui.add(egui::Label::new(RichText::new(nums).monospace().color(theme::weak_text(ui))).selectable(false));
                        });
                    }
                    let mut output = egui::TextEdit::multiline(text)
                        .code_editor()
                        .interactive(!read_only)
                        .desired_width(f32::INFINITY)
                        .desired_rows(32)
                        .lock_focus(true)
                        .layouter(&mut layouter)
                        .show(ui);
                    changed = output.response.changed();

                    if let Some((a, b)) = pending {
                        let range = CCursorRange::two(CCursor::new(a), CCursor::new(b));
                        output.state.cursor.set_char_range(Some(range));
                        output.state.clone().store(ui.ctx(), output.response.id);
                        output.response.request_focus();
                        let pos = output.galley.pos_from_ccursor(CCursor::new(a)).translate(output.galley_pos.to_vec2());
                        ui.scroll_to_rect(pos.expand2(Vec2::new(40.0, 60.0)), Some(egui::Align::Center));
                    }
                });
            });
        });
        if changed {
            self.outline_dirty = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_finds_functions_and_classes() {
        let py = "class Foo:\n    def bar(self):\n        pass\nasync def main():\n    x = 1\n";
        let o = outline("py", py);
        assert_eq!(o.iter().map(|(l, _)| *l).collect::<Vec<_>>(), vec![0, 1, 3]);
        assert!(o[1].1.contains("bar()"));
        assert_eq!(o[0].1, "◆ Foo");

        let c = "#include <stdio.h>\nstatic int add(int a, int b)\n{\n    if (a) return a;\n}\nint main(void) {\n  return add(1,2);\n}\nint x = f(1);\n";
        let names: Vec<_> = outline("c", c).into_iter().map(|(_, s)| s).collect();
        assert_eq!(names, vec!["ƒ add()", "ƒ main()"]);
    }

    #[test]
    fn classify_keeps_crlf_and_bom_on_save() {
        let st = ViewerState::new(0, "a.py".into(), 0, false, b"\xEF\xBB\xBFx = 1\r\ny = 2\r\n".to_vec(), true);
        assert_eq!(st.encoded().unwrap(), b"\xEF\xBB\xBFx = 1\r\ny = 2\r\n");
        assert!(matches!(classify("a.bin", vec![0, 1, 2]), ViewerContent::Binary { .. }));
        assert!(matches!(classify("a.png", vec![0]), ViewerContent::Image { .. }));
        assert!(prefers_builtin("src/main.cpp") && !prefers_builtin("setup.exe"));
    }
}
