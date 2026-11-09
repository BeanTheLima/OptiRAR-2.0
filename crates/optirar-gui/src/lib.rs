pub mod app;
pub mod batch;
pub mod shell;
pub mod taskbar;
pub mod theme;
pub mod views;
pub mod worker;

pub use app::{LaunchArgs, OptiRarApp, QuickAction};

pub fn run_gui(args: LaunchArgs) -> eframe::Result<()> {
    let config = optirar_core::config::AppConfig::load();
    // Context-menu actions get a small WinRAR-style progress window.
    let (size, min) = if args.quick.is_some() {
        ([520.0, 250.0], [420.0, 200.0])
    } else {
        ([config.window_width.max(820.0), config.window_height.max(520.0)], [760.0, 480.0])
    };

    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_min_inner_size(min)
            .with_title("OptiRAR")
            .with_icon(std::sync::Arc::new(eframe::egui::IconData {
                rgba: include_bytes!("../../../assets/icon_64.rgba").to_vec(),
                width: 64,
                height: 64,
            }))
            .with_drag_and_drop(true),
        centered: true,
        ..Default::default()
    };

    eframe::run_native("OptiRAR", native_options, Box::new(|cc| Ok(Box::new(OptiRarApp::new(cc, args)))))
}
