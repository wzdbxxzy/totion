#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod app;
mod identity;
mod net;
mod store;
mod theme;
mod views;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([1020.0, 680.0])
            .with_title("Glasswing"),
        ..Default::default()
    };
    eframe::run_native(
        "Glasswing",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
