use eframe::egui::{self, RichText, Vec2};
use optirar_core::t;

#[derive(Default)]
pub struct ToolbarAction {
    pub open_clicked: bool,
    pub create_clicked: bool,
    pub extract_clicked: bool,
    pub test_clicked: bool,
    pub view_clicked: bool,
    pub info_clicked: bool,
}

pub struct ToolbarView;

impl ToolbarView {
    pub fn show(ui: &mut egui::Ui, has_archive: bool, has_selection: bool, is_busy: bool) -> ToolbarAction {
        let mut action = ToolbarAction::default();

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            let tool = |ui: &mut egui::Ui, enabled: bool, icon: &str, key: &str, tip: &str| {
                ui.add_enabled(
                    enabled && !is_busy,
                    egui::Button::new(RichText::new(format!("{}  {}", icon, t(key)))).min_size(Vec2::new(0.0, 30.0)),
                )
                .on_hover_text(t(tip))
                .clicked()
            };

            action.open_clicked = tool(ui, true, "📂", "toolbar.open", "tips.open");
            action.create_clicked = tool(ui, true, "➕", "toolbar.add_files", "tips.add");
            ui.separator();
            let extract_key = if has_selection { "toolbar.extract_selected" } else { "toolbar.extract_all" };
            action.extract_clicked = tool(ui, has_archive, "📤", extract_key, "tips.extract");
            action.test_clicked = tool(ui, has_archive, "🛡", "toolbar.test", "tips.test");
            action.view_clicked = tool(ui, has_selection, "👁", "toolbar.view", "tips.view");
            action.info_clicked = tool(ui, has_archive, "ℹ", "toolbar.info", "tips.info");
        });

        action
    }
}
