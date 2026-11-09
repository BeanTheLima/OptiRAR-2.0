use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin, Stroke, TextStyle, Vec2};
use std::path::PathBuf;
use std::sync::Arc;

pub const ACCENT: Color32 = Color32::from_rgb(0, 120, 212);
pub const SUCCESS: Color32 = Color32::from_rgb(46, 170, 90);
pub const WARNING: Color32 = Color32::from_rgb(214, 160, 0);
pub const ERROR: Color32 = Color32::from_rgb(220, 60, 50);

/// Accent colour readable as text on the current background.
pub fn accent_text(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(64, 170, 255)
    } else {
        Color32::from_rgb(0, 95, 184)
    }
}

pub fn weak_text(ui: &egui::Ui) -> Color32 {
    ui.visuals().weak_text_color()
}

/// Rounded card used by settings, welcome, tools.
pub fn card(ui: &egui::Ui) -> Frame {
    Frame::NONE
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(12))
}

/// Load Windows system fonts that fully cover Vietnamese (the bundled egui font does not).
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let dir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Windows"))
        .join("Fonts");

    let load = |files: &[&str]| files.iter().find_map(|f| std::fs::read(dir.join(f)).ok());

    if let Some(bytes) = load(&["segoeui.ttf", "tahoma.ttf", "arial.ttf"]) {
        fonts.font_data.insert("sys-ui".into(), Arc::new(FontData::from_owned(bytes)));
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "sys-ui".into());
    }
    if let Some(bytes) = load(&["consola.ttf", "cour.ttf"]) {
        fonts.font_data.insert("sys-mono".into(), Arc::new(FontData::from_owned(bytes)));
        fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "sys-mono".into());
    }
    // Last-resort symbol coverage (arrows, check marks, gear...).
    if let Some(bytes) = load(&["seguisym.ttf"]) {
        fonts.font_data.insert("sys-symbol".into(), Arc::new(FontData::from_owned(bytes)));
        for fam in [FontFamily::Proportional, FontFamily::Monospace] {
            fonts.families.entry(fam).or_default().push("sys-symbol".into());
        }
    }

    ctx.set_fonts(fonts);
}

pub fn configure_visuals(ctx: &egui::Context, is_dark: bool) {
    let mut visuals = if is_dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    visuals.window_corner_radius = CornerRadius::same(6);
    visuals.window_shadow.blur = 12;
    visuals.selection.bg_fill = if is_dark {
        Color32::from_rgb(0, 84, 153)
    } else {
        Color32::from_rgb(204, 228, 247)
    };
    visuals.selection.stroke = Stroke::new(1.0_f32, if is_dark { Color32::WHITE } else { Color32::from_rgb(0, 60, 120) });

    if is_dark {
        visuals.panel_fill = Color32::from_rgb(30, 30, 34);
        visuals.window_fill = Color32::from_rgb(36, 36, 42);
        visuals.faint_bg_color = Color32::from_rgb(38, 38, 44);
        visuals.extreme_bg_color = Color32::from_rgb(22, 22, 26);
        visuals.window_stroke = Stroke::new(1.0_f32, Color32::from_rgb(60, 60, 68));
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(30, 30, 34);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(220, 220, 228));
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(48, 48, 56);
        visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(48, 48, 56);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(60, 60, 72);
        visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(60, 60, 72);
    } else {
        visuals.panel_fill = Color32::from_rgb(246, 246, 248);
        visuals.window_fill = Color32::WHITE;
        visuals.faint_bg_color = Color32::from_rgb(238, 238, 242);
        visuals.extreme_bg_color = Color32::WHITE;
    }
    visuals.widgets.active.bg_fill = ACCENT;
    for w in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(4);
    }

    ctx.set_visuals(visuals);

    ctx.style_mut(|style| {
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.window_margin = Margin::same(12);
        style.spacing.button_padding = Vec2::new(10.0, 4.0);
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(11.5)),
            (TextStyle::Body, FontId::proportional(14.0)),
            (TextStyle::Button, FontId::proportional(14.0)),
            (TextStyle::Heading, FontId::proportional(20.0)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
    });
}
