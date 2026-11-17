use eframe::egui;
use crate::app::App;

pub fn draw(_app: &mut App, ui: &mut egui::Ui) {
    ui.centered_and_justified(|ui| {
        ui.label(crate::theme::dim("正在搭建…"));
    });
}
