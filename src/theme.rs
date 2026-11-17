//! 视觉系统：可运行期调整的调色板、字体，以及卡片 / 进度条 / 标签构件。

use std::sync::{Mutex, OnceLock};

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, RichText,
    Shadow, Stroke, TextStyle, Vec2, Visuals,
};

pub const RADIUS: u8 = 14;

/// 用户可调的配色。所有取值都存在 `%APPDATA%\glasswing\theme.json`。
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// 玻璃整体浓度：0 完全透明，越大越实。
    pub glass_alpha: u8,
    /// 卡片白色蒙层的不透明度。
    pub card_alpha: u8,
    /// 描边 / 分隔线的不透明度。
    pub line_alpha: u8,
    pub accent: [u8; 3],
    pub text: [u8; 3],
    pub text_dim: [u8; 3],
    pub text_muted: [u8; 3],
    /// 卡片底色（默认近白），透明度由 `card_alpha` 控制。
    pub card_rgb: [u8; 3],
    /// 玻璃底色，透明度由 `glass_alpha` 控制。
    pub glass_rgb: [u8; 3],
    /// 导航放顶部还是左侧。
    pub nav_top: bool,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            glass_alpha: 0,
            card_alpha: 132,
            line_alpha: 70,
            accent: [38, 104, 152],
            text: [22, 33, 45],
            text_dim: [56, 74, 92],
            text_muted: [92, 110, 128],
            card_rgb: [255, 255, 255],
            glass_rgb: [226, 240, 252],
            nav_top: false,
        }
    }
}

static PALETTE: OnceLock<Mutex<Palette>> = OnceLock::new();

fn cell() -> &'static Mutex<Palette> {
    PALETTE.get_or_init(|| Mutex::new(load_palette().unwrap_or_default()))
}

pub fn palette() -> Palette {
    *cell().lock().unwrap()
}

pub fn set_palette(value: Palette) {
    *cell().lock().unwrap() = value;
    save_palette(value);
}

fn theme_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("glasswing")
        .join("theme.json")
}

fn load_palette() -> Option<Palette> {
    let raw = std::fs::read_to_string(theme_path()).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let mut p = Palette::default();
    if let Some(v) = value.get("glass_alpha").and_then(|v| v.as_u64()) {
        p.glass_alpha = v.clamp(0, 220) as u8;
    }
    if let Some(v) = value.get("card_alpha").and_then(|v| v.as_u64()) {
        p.card_alpha = v.clamp(0, 255) as u8;
    }
    if let Some(v) = value.get("line_alpha").and_then(|v| v.as_u64()) {
        p.line_alpha = v.clamp(0, 255) as u8;
    }
    for (key, slot) in [
        ("accent", &mut p.accent),
        ("text", &mut p.text),
        ("text_dim", &mut p.text_dim),
        ("text_muted", &mut p.text_muted),
        ("card_rgb", &mut p.card_rgb),
        ("glass_rgb", &mut p.glass_rgb),
    ] {
        if let Some(array) = value.get(key).and_then(|v| v.as_array()) {
            if array.len() == 3 {
                for (index, item) in array.iter().enumerate() {
                    if let Some(number) = item.as_u64() {
                        slot[index] = number.min(255) as u8;
                    }
                }
            }
        }
    }
    if let Some(top) = value.get("nav_top").and_then(|v| v.as_bool()) {
        p.nav_top = top;
    }
    Some(p)
}

fn save_palette(p: Palette) {
    let path = theme_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let raw = serde_json::json!({
        "glass_alpha": p.glass_alpha,
        "card_alpha": p.card_alpha,
        "line_alpha": p.line_alpha,
        "accent": p.accent,
        "text": p.text,
        "text_dim": p.text_dim,
        "text_muted": p.text_muted,
        "card_rgb": p.card_rgb,
        "glass_rgb": p.glass_rgb,
        "nav_top": p.nav_top,
    })
    .to_string();
    let _ = std::fs::write(path, raw);
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn glass_alpha() -> u8 {
    palette().glass_alpha
}

pub fn glass_rgb() -> Color32 {
    rgb(palette().glass_rgb)
}
pub fn text() -> Color32 {
    rgb(palette().text)
}
pub fn text_dim() -> Color32 {
    rgb(palette().text_dim)
}
pub fn text_muted() -> Color32 {
    rgb(palette().text_muted)
}
pub fn accent() -> Color32 {
    rgb(palette().accent)
}
pub fn accent_soft() -> Color32 {
    let c = palette().accent;
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], 40)
}
pub fn card() -> Color32 {
    let c = palette().card_rgb;
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], palette().card_alpha)
}
pub fn card_hi() -> Color32 {
    let c = palette().card_rgb;
    let alpha = (palette().card_alpha as u16 + 36).min(255) as u8;
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], alpha)
}
pub fn field() -> Color32 {
    let c = palette().card_rgb;
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], palette().card_alpha.saturating_sub(12))
}
pub fn sidebar() -> Color32 {
    Color32::from_rgba_unmultiplied(255, 255, 255, palette().card_alpha / 3)
}
pub fn line() -> Color32 {
    let c = palette().accent;
    Color32::from_rgba_unmultiplied(c[0] / 2 + 30, c[1] / 2 + 30, c[2] / 2 + 30, palette().line_alpha)
}
pub fn line_soft() -> Color32 {
    let c = palette().accent;
    Color32::from_rgba_unmultiplied(
        c[0] / 2 + 30,
        c[1] / 2 + 30,
        c[2] / 2 + 30,
        palette().line_alpha / 2,
    )
}
pub fn bg() -> Color32 {
    Color32::TRANSPARENT
}
pub fn ok() -> Color32 {
    Color32::from_rgb(38, 122, 92)
}
pub fn warn() -> Color32 {
    Color32::from_rgb(160, 108, 28)
}
pub fn danger() -> Color32 {
    Color32::from_rgb(172, 62, 56)
}

/// 字体：正文用等线，标题用等线粗体，数字尽量用同一套以免跳字。
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut load = |name: &str, path: &str| -> Option<Vec<u8>> {
        std::fs::read(path)
            .ok()
            .or_else(|| std::fs::read(format!("C:/Windows/Fonts/{name}")).ok())
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

pub fn title(text_value: impl Into<String>, size: f32) -> RichText {
    RichText::new(text_value)
        .font(FontId::new(size, bold_family()))
        .color(text())
}

pub fn body(text_value: impl Into<String>) -> RichText {
    RichText::new(text_value).size(13.0).color(text())
}

pub fn dim(text_value: impl Into<String>) -> RichText {
    RichText::new(text_value).size(12.0).color(text_dim())
}

pub fn muted(text_value: impl Into<String>) -> RichText {
    RichText::new(text_value).size(11.0).color(text_muted())
}

/// 数字读数：等宽感、偏大、亮色。
pub fn metric(text_value: impl Into<String>, size: f32) -> RichText {
    RichText::new(text_value)
        .font(FontId::new(size, bold_family()))
        .color(text())
}

pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = Visuals::light();
    style.visuals.override_text_color = Some(text());
    style.visuals.panel_fill = Color32::TRANSPARENT;
    style.visuals.window_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 236);
    style.visuals.extreme_bg_color = Color32::from_rgba_unmultiplied(255, 255, 255, 200);
    style.visuals.faint_bg_color = card_hi();
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, line_soft());
    style.visuals.widgets.inactive.bg_fill = card();
    style.visuals.widgets.inactive.weak_bg_fill = card();
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, line());
    style.visuals.widgets.hovered.bg_fill = card_hi();
    style.visuals.widgets.hovered.weak_bg_fill = card_hi();
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, line());
    style.visuals.widgets.active.bg_fill = accent_soft();
    style.visuals.widgets.active.weak_bg_fill = accent_soft();
    style.visuals.selection.bg_fill = accent_soft();
    style.visuals.selection.stroke = Stroke::new(1.0_f32, accent());
    style.visuals.hyperlink_color = accent();
    style.visuals.window_corner_radius = CornerRadius::same(RADIUS);
    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.fg_stroke = Stroke::new(1.0_f32, text());
    }
    style.spacing.item_spacing = Vec2::new(6.0, 7.0);
    style.spacing.button_padding = Vec2::new(12.0, 7.0);
    style.spacing.interact_size.y = 26.0;
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = CornerRadius::same(9);
    }
    style.text_styles = [
        (TextStyle::Heading, FontId::new(22.0, bold_family())),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(13.0, FontFamily::Proportional)),
    ]
    .into();
    ctx.set_style(style);
}

/// 重新套用调色板（设置面板改色后调用）。
pub fn refresh(ctx: &egui::Context) {
    apply(ctx);
}

/// 卡片外框：低对比填充 + 1px 边 + 轻微投影。
pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(card())
        .stroke(Stroke::new(1.0_f32, line()))
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(Margin::same(13))
        .shadow(Shadow {
            offset: [0, 4],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(40),
        })
}

/// 弹层：和主面板同材质的半透明玻璃，而不是一块实心白板。
pub fn glass_window_frame() -> egui::Frame {
    let fill = 250u16.saturating_sub(0);
    let _ = fill;
    let alpha = palette().card_alpha.saturating_add(40).min(250);
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(250, 253, 255, alpha))
        .stroke(Stroke::new(1.0_f32, line()))
        .corner_radius(CornerRadius::same(18))
        .inner_margin(Margin::same(18))
        .shadow(Shadow {
            offset: [0, 10],
            blur: 40,
            spread: 0,
            color: Color32::from_black_alpha(90),
        })
}

pub fn flat_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(field())
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(10))
}

/// 小标签（V4 / 只读 / 已归档 之类）。
pub fn pill(ui: &mut egui::Ui, label: &str, color: Color32) {
    let bg = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 34);
    egui::Frame::new()
        .fill(bg)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(11.0).color(color));
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
    painter.rect_filled(rect, radius, line_soft());
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
pub fn section(ui: &mut egui::Ui, label: &str) {
    ui.label(
        RichText::new(label)
            .font(FontId::new(11.5, bold_family()))
            .color(text_muted()),
    );
    ui.add_space(2.0);
}

/// 细横线。
pub fn hairline(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), egui::Sense::hover());
    ui.painter()
        .hline(rect.x_range(), rect.center().y, Stroke::new(1.0_f32, line_soft()));
}
