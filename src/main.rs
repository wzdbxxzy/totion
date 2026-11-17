#![windows_subsystem = "windows"]

mod api;
mod app;
mod demo;
mod desktop;
mod identity;
mod net;
mod store;
mod theme;
mod views;
mod wallpaper;

fn main() -> eframe::Result<()> {
    std::panic::set_hook(Box::new(|info| {
        desktop::log_line(&format!("panic: {info}"));
    }));
    desktop::log_line("start");
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_transparent(true)
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_window_level(eframe::egui::WindowLevel::AlwaysOnBottom)
            .with_title("Glasswing"),
        ..Default::default()
    };
    eframe::run_native(
        "Glasswing",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
