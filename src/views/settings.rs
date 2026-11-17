//! 设置：一整个页面，不再是弹窗。配色、账号、邀请码都在这里。

use eframe::egui::{self, Align, Color32, CornerRadius, Layout, Sense, Stroke, StrokeKind, Vec2};
use totion_core::User;

use crate::app::{App, Dialog, View};
use crate::net::Command;
use crate::theme::*;

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    ui.label(title("设置", 24.0));
    ui.add_space(2.0);
    ui.label(muted("配色、账号与空间管理"));
    ui.add_space(14.0);

    let mut palette = palette();
    let mut changed = false;

    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section(ui, "玻璃与卡片");
        changed |= ui
            .add(egui::Slider::new(&mut palette.glass_alpha, 0..=200).text("玻璃浓度"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut palette.card_alpha, 0..=255).text("卡片不透明度"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut palette.line_alpha, 0..=255).text("描边不透明度"))
            .changed();
        changed |= color_row(ui, "玻璃底色", &mut palette.glass_rgb);
        changed |= color_row(ui, "卡片底色", &mut palette.card_rgb);
    });

    ui.add_space(12.0);
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section(ui, "颜色");
        changed |= color_row(ui, "强调色", &mut palette.accent);
        changed |= color_row(ui, "正文", &mut palette.text);
        changed |= color_row(ui, "次级文字", &mut palette.text_dim);
        changed |= color_row(ui, "弱化文字", &mut palette.text_muted);
        if ui.button("恢复默认配色").clicked() {
            palette = Palette::default();
            changed = true;
        }
    });

    ui.add_space(12.0);
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section(ui, "布局");
        let mut side = !palette.nav_top;
        if ui.radio_value(&mut side, true, "左侧导航").clicked() {
            palette.nav_top = false;
            changed = true;
        }
        if ui.radio_value(&mut side, false, "顶部导航（内容更宽）").clicked() {
            palette.nav_top = true;
            changed = true;
        }
    });

    ui.add_space(12.0);
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section(ui, "空间");
        kv(ui, "账号", app.me.clone());
        kv(ui, "空间名", app.store.workspace_name.clone());
        kv(ui, "成员", format!("{} 人", app.store.users.len()));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("立即同步").clicked() {
                app.send(Command::SyncNow);
            }
            if ui.button("成员").clicked() {
                app.send(Command::Members);
                app.dialog = Dialog::Members;
            }
            if ui.button("邀请码").clicked() {
                app.send(Command::ListInvites);
                app.dialog = Dialog::Invites {
                    list: Vec::new(),
                    loading: true,
                };
            }
        });
        ui.add_space(10.0);
        section(ui, "成员列表");
        for User { name, id, .. } in app.store.users.clone() {
            ui.horizontal(|ui| {
                if app.member_filter.as_deref() == Some(id.as_str()) {
                    ui.label(accent_text("●"));
                } else {
                    ui.label(muted("○"));
                }
                ui.label(body(name));
                if id == app.me_id {
                    ui.label(muted("（我）"));
                }
            });
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("看共享空间").clicked() {
                app.member_filter = None;
                app.member_name.clear();
                app.view = View::Today;
            }
        });
    });

    if changed {
        set_palette(palette);
        refresh(ui.ctx());
    }
}

fn accent_text(value: &str) -> egui::RichText {
    egui::RichText::new(value).size(12.0).color(accent())
}

fn color_row(ui: &mut egui::Ui, label: &str, slot: &mut [u8; 3]) -> bool {
    let id = ui.make_persistent_id(("palette-row", label));
    let mut open = ui.data(|data| data.get_temp::<bool>(id).unwrap_or(false));
    let mut changed = false;
    ui.horizontal(|ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(26.0, 15.0), Sense::click());
        let color = Color32::from_rgb(slot[0], slot[1], slot[2]);
        ui.painter().rect_filled(rect, CornerRadius::same(4), color);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(1.0_f32, line()),
            StrokeKind::Inside,
        );
        ui.label(muted(label));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(muted(format!("#{:02X}{:02X}{:02X}", slot[0], slot[1], slot[2])));
        });
        if response.clicked() {
            open = !open;
        }
    });
    if open {
        let mut rgb = *slot;
        ui.horizontal(|ui| {
            changed |= ui.add(egui::Slider::new(&mut rgb[0], 0..=255).text("R")).changed();
            changed |= ui.add(egui::Slider::new(&mut rgb[1], 0..=255).text("G")).changed();
            changed |= ui.add(egui::Slider::new(&mut rgb[2], 0..=255).text("B")).changed();
        });
        *slot = rgb;
    }
    ui.data_mut(|data| data.insert_temp(id, open));
    changed
}
