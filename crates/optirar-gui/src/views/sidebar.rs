use eframe::egui::{self, Color32, CornerRadius, RichText, Vec2};
use optirar_core::t;

use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavTab {
    #[default]
    Home,
    Explorer,
    Queue,
    Tools,
    Settings,
    About,
}

pub struct SidebarView;

impl SidebarView {
    pub fn show(ui: &mut egui::Ui, current_tab: &mut NavTab, is_collapsed: &mut bool, has_active_queue: bool) {
        let width = if *is_collapsed { 48.0 } else { 196.0 };
        ui.set_min_width(width);
        ui.set_max_width(width);

        ui.vertical(|ui| {
            ui.add_space(6.0);
            let toggle_sym = if *is_collapsed { "»" } else { "«" };
            if ui.small_button(toggle_sym).on_hover_text(t("nav.toggle")).clicked() {
                *is_collapsed = !*is_collapsed;
            }
            ui.add_space(4.0);

            let collapsed = *is_collapsed;
            let mut render_item = |ui: &mut egui::Ui, tab: NavTab, icon: &str, label_key: &str, badge: bool| {
                let is_selected = *current_tab == tab;
                let fg = if is_selected { Color32::WHITE } else { ui.visuals().text_color() };
                let label = if collapsed {
                    icon.to_string()
                } else {
                    format!("{}   {}{}", icon, t(label_key), if badge { "  ●" } else { "" })
                };
                // Left-aligned layout => left-aligned button text.
                let align = if collapsed { egui::Align::Center } else { egui::Align::Min };
                let resp = ui
                    .with_layout(egui::Layout::top_down(align), |ui| {
                        ui.add(
                            egui::Button::new(RichText::new(label).size(14.0).color(fg))
                                .fill(if is_selected { theme::ACCENT } else { Color32::TRANSPARENT })
                                .corner_radius(CornerRadius::same(5))
                                .wrap_mode(egui::TextWrapMode::Truncate)
                                .min_size(Vec2::new(width - 10.0, 34.0)),
                        )
                    })
                    .inner;
                if resp.clicked() {
                    *current_tab = tab;
                }
                if collapsed {
                    resp.on_hover_text(t(label_key));
                }
            };

            render_item(ui, NavTab::Home, "🏠", "nav.home", false);
            render_item(ui, NavTab::Explorer, "📂", "nav.explorer", false);
            render_item(ui, NavTab::Queue, "⏱", "nav.task_queue", has_active_queue);
            render_item(ui, NavTab::Tools, "🛠", "nav.archive_tools", false);
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            render_item(ui, NavTab::Settings, "⚙", "nav.settings", false);
            render_item(ui, NavTab::About, "ℹ", "nav.about", false);
        });
    }
}
