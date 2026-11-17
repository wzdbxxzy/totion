//! 视觉系统：颜色、字体、间距，以及可复用的卡片 / 进度条 / 标签构件。
//!
//! 参考图为深蓝底色的监控面板：近黑背景、比背景略亮的卡片、
//! 低对比描边、单一青蓝色强调、数字用等宽感字体。

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, RichText,
    Shadow, Stroke, TextStyle, Vec2, Visuals,
};

pub const BG: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 0);
pub const SIDEBAR: Color32 = Color32::from_rgba_premultiplied(3, 2, 3, 70);
pub const CARD: Color32 = Color32::from_rgba_premultiplied(19, 17, 16, 184);
pub const CARD_HI: Color32 = Color32::from_rgba_premultiplied(31, 28, 26, 208);
pub const FIELD: Color32 = Color32::from_rgba_premultiplied(11, 10, 10, 176);

pub const LINE: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 18);
pub const LINE_SOFT: Color32 = Color32::from_rgba_premultiplied(255, 255, 255, 10);

pub const TEXT: Color32 = Color32::from_rgb(246, 242, 236);
pub const TEXT_DIM: Color32 = Color32::from_rgb(206, 198, 190);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(150, 141, 133);

pub const ACCENT: Color32 = Color32::from_rgb(216, 181, 140);
pub const ACCENT_SOFT: Color32 = Color32::from_rgba_premultiplied(31, 26, 20, 44);
pub const OK: Color32 = Color32::from_rgb(111, 194, 154);
pub const WARN: Color32 = Color32::from_rgb(224, 168, 99);
pub const DANGER: Color32 = Color32::from_rgb(217, 122, 112);

pub const RADIUS: u8 = 14;

/// 字体：正文用等线，标题用等线粗体，数字尽量用同一套以免跳字。
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut load = |name: &str, path: &str| -> Option<Vec<u8>> {
        let data = std::fs::read(path).ok().or_else(|| {
            std::fs::read(format!("C:/Windows/Fonts/{name}")).ok()
        });
        data
    };
    if let Some(deng) = load("Deng.ttf", "C:/Windows/Fonts/Deng.ttf") {
        fonts
            .font_data
            .insert("body".to_owned(), FontData::from_owned(deng).into());
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "body".to_owned());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "body".to_owned());
    }
    if let Some(bold) = load("Dengb.ttf", "C:/Windows/Fonts/Dengb.ttf") {
        fonts
            .font_data
            .insert("bold".to_owned(), FontData::from_owned(bold).into());
        let family = FontFamily::Name("bold".into());
        fonts
            .families
            .entry(family)
            .or_insert_with(Vec::new)
            .insert(0, "bold".to_owned());
    }
    ctx.set_fonts(fonts);
}

pub fn bold_family() -> FontFamily {
    FontFamily::Name("bold".into())
}

pub fn title(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(FontId::new(size, bold_family())).color(TEXT)
}

pub fn body(text: impl Into<String>) -> RichText {
    RichText::new(text).size(14.0).color(TEXT)
}

pub fn dim(text: impl Into<String>) -> RichText {
    RichText::new(text).size(13.0).color(TEXT_DIM)
}

pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text).size(12.0).color(TEXT_MUTED)
}

/// 数字读数：等宽感、偏大、亮色。
pub fn metric(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text)
        .font(FontId::new(size, bold_family()))
        .color(TEXT)
}

pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = Visuals::dark();
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = SIDEBAR;
    style.visuals.extreme_bg_color = FIELD;
    style.visuals.faint_bg_color = CARD_HI;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE_SOFT);
    style.visuals.widgets.inactive.bg_fill = CARD;
    style.visuals.widgets.inactive.weak_bg_fill = CARD;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    style.visuals.widgets.hovered.bg_fill = CARD_HI;
    style.visuals.widgets.hovered.weak_bg_fill = CARD_HI;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, LINE);
    style.visuals.widgets.active.bg_fill = ACCENT_SOFT;
    style.visuals.widgets.active.weak_bg_fill = ACCENT_SOFT;
    style.visuals.selection.bg_fill = ACCENT_SOFT;
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    style.visuals.window_corner_radius = CornerRadius::same(RADIUS);
    style.spacing.item_spacing = Vec2::new(10.0, 10.0);
    style.spacing.button_padding = Vec2::new(12.0, 7.0);
    style.spacing.interact_size.y = 30.0;
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = CornerRadius::same(9);
    }
    style.text_styles = [
        (TextStyle::Heading, FontId::new(22.0, bold_family())),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.0, FontFamily::Proportional)),
    ]
    .into();
    ctx.set_style(style);
}

/// 卡片外框：低对比填充 + 1px 边 + 轻微投影。
pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, LINE))
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(Margin::same(16))
        .shadow(Shadow {
            offset: [0, 4],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(70),
        })
}

pub fn flat_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(FIELD)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(10))
}

/// 小标签（V4 / 只读 / 已归档 之类）。
pub fn pill(ui: &mut egui::Ui, text: &str, color: Color32) {
    let bg = Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), 30);
    egui::Frame::new()
        .fill(bg)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).color(color));
        });
}

/// 进度条：圆角细条 + 右侧百分比。`ratio` 为 0..=1。
pub fn progress(ui: &mut egui::Ui, ratio: f32, color: Color32) {
    let ratio = ratio.clamp(0.0, 1.0);
    let height = 6.0;
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let painter = ui.painter();
    let radius = CornerRadius::same((height / 2.0) as u8);
    painter.rect_filled(rect, radius, Color32::from_rgba_premultiplied(255, 255, 255, 16));
    if ratio > 0.0 {
        let filled = egui::Rect::from_min_size(rect.min, Vec2::new(width * ratio, height));
        painter.rect_filled(filled, radius, color);
    }
}

/// 一行「标签 …… 数值」。
pub fn kv(ui: &mut egui::Ui, label: &str, value: impl Into<String>) {
    ui.horizontal(|ui| {
        ui.label(muted(label));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(body(value));
        });
    });
}

/// 分区标题：小号、字距感、低对比，像仪表盘里的分组名。
pub fn section(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .font(FontId::new(11.5, bold_family()))
            .color(TEXT_MUTED),
    );
    ui.add_space(2.0);
}

/// 竖向渐隐分隔线。
pub fn hairline(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), egui::Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, LINE_SOFT));
}
