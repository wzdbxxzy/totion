use std::time::{Duration, Instant};

use chrono::{Datelike, Local, NaiveDate};
use eframe::egui::{self, Align, Color32, Layout, RichText};
use totion_core::{Block, CommentTargetKind, JournalTarget};

use crate::net::{Command, Event, Net};
use crate::store::Store;
use crate::theme::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Today,
    Workspace,
    Calendar,
    Habits,
    Stats,
    Inbox,
    Settings,
}

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Today => "今日",
            View::Workspace => "工作区",
            View::Calendar => "日历",
            View::Habits => "习惯",
            View::Stats => "统计",
            View::Inbox => "留言",
            View::Settings => "设置",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            View::Today => "今天的待办、习惯与总结",
            View::Workspace => "页面与内容块",
            View::Calendar => "按月查看每一天",
            View::Habits => "连续天数与打卡",
            View::Stats => "完成率与趋势",
            View::Inbox => "所有未解决的留言",
            View::Settings => "配色、账号与邀请码",
        }
    }
}

/// 新建 / 编辑弹窗统一用这个描述。
pub enum Editor {
    None,
    Block {
        block: Option<Block>,
        text: String,
        time: String,
        date: String,
    },
    Journal {
        target_kind: JournalTarget,
        target_id: String,
        label: String,
        existing: Option<(String, i64)>,
        text: String,
    },
    Habit {
        title: String,
        icon: String,
        weekly: bool,
    },
    Page {
        page: Option<totion_core::Page>,
        title: String,
    },
}

pub struct InlineEdit {
    /// None 表示新建
    pub id: Option<String>,
    pub version: i64,
    pub date: String,
    pub text: String,
    pub multiline: bool,
    pub focus: bool,
}

pub enum Dialog {
    None,
    Comments {
        kind: CommentTargetKind,
        id: String,
        label: String,
        draft: String,
    },
    Members,
    Invites {
        list: Vec<totion_core::Invite>,
        loading: bool,
    },
}

pub struct App {
    // 网络
    net: Option<Net>,
    pub store: Store,
    pub me: String,
    pub me_id: String,
    status: String,
    last_ok: Instant,
    // 登录表单
    server_url: String,
    name: String,
    invite_code: String,
    login_error: Option<String>,
    registering: bool,
    // 界面
    pub view: View,
    pub date: NaiveDate,
    pub selected_page: Option<String>,
    pub selected_habit: Option<String>,
    pub member_filter: Option<String>,
    pub member_name: String,
    pub editor: Editor,
    pub dialog: Dialog,
    /// 就地编辑：待办/总结直接在原位改，不弹窗。
    pub inline: Option<InlineEdit>,
    toast: Option<(String, Instant, bool)>,
    started: Instant,
    logged: bool,
    hwnd: Option<isize>,
    backdrop: Option<crate::wallpaper::Backdrop>,
    desktop_ready: bool,
    last_anchor: Instant,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        apply(&cc.egui_ctx);
        let saved = crate::identity::load_identity().ok().flatten();
        let (server_url, name) = match &saved {
            Some(identity) => (identity.server_url.clone(), identity.name.clone()),
            None => (String::new(), String::new()),
        };
        let mut app = Self {
            net: None,
            store: Store::default(),
            me: name.clone(),
            me_id: String::new(),
            status: "未连接".to_string(),
            last_ok: Instant::now(),
            server_url,
            name,
            invite_code: String::new(),
            login_error: None,
            registering: false,
            view: match std::env::var("GLASSWING_VIEW").as_deref() {
                Ok("workspace") => View::Workspace,
                Ok("calendar") => View::Calendar,
                Ok("habits") => View::Habits,
                Ok("stats") => View::Stats,
                Ok("inbox") => View::Inbox,
                _ => View::Today,
            },
            date: Local::now().date_naive(),
            selected_page: None,
            selected_habit: None,
            member_filter: None,
            member_name: String::new(),
            editor: Editor::None,
            dialog: Dialog::None,
            inline: None,
            toast: None,
            started: Instant::now(),
            logged: false,
            hwnd: None,
            backdrop: None,
            desktop_ready: false,
            last_anchor: Instant::now(),
        };
        if let Some(identity) = saved {
            app.connect(identity, None);
        }
        app
    }

    fn connect(&mut self, identity: crate::identity::Identity, invite: Option<String>) {
        let net = crate::net::spawn();
        let _ = net.start_tx.send(crate::net::Start {
            identity,
            invite_code: invite,
        });
        self.net = Some(net);
        self.status = "连接中…".to_string();
        self.login_error = None;
    }

    fn connect_from_form(&mut self) {
        let server = self.server_url.trim().to_string();
        let name = self.name.trim().to_string();
        if server.is_empty() || name.is_empty() {
            self.login_error = Some("服务器地址和名字不能为空".to_string());
            return;
        }
        let saved = crate::identity::load_identity().ok().flatten();
        match saved {
            Some(mut identity) if identity.server_url == server => {
                identity.name = name;
                self.connect(identity, None);
            }
            _ => {
                let identity = crate::identity::Identity::generate(&server, &name);
                let invite = if self.registering {
                    Some(self.invite_code.trim().to_string())
                } else {
                    self.login_error = Some("还没有身份，请勾选「用邀请码注册」".to_string());
                    return;
                };
                self.connect(identity, invite);
            }
        }
    }

    pub fn send(&mut self, command: Command) {
        if let Some(net) = &self.net {
            let _ = net.tx.send(command);
        }
    }

    pub fn toast(&mut self, message: impl Into<String>, error: bool) {
        self.toast = Some((message.into(), Instant::now(), error));
    }

    fn poll(&mut self) {
        let mut events = Vec::new();
        if let Some(net) = &self.net {
            while let Ok(event) = net.rx.try_recv() {
                events.push(event);
            }
        }
        for event in events {
            match event {
                Event::Ready {
                    me,
                    user_id,
                    store,
                } => {
                    self.store = *store;
                    self.me = me;
                    self.me_id = user_id;
                    self.status = "已连接".to_string();
                    self.last_ok = Instant::now();
                    self.ensure_today();
                }
                Event::Changes { changes, last_seq } => {
                    for change in changes {
                        self.store.apply(change);
                    }
                    self.store.last_seq = self.store.last_seq.max(last_seq);
                    self.status = "已连接".to_string();
                    self.last_ok = Instant::now();
                }
                Event::Ok(message) => {
                    if !message.is_empty() {
                        self.toast(message, false);
                    }
                    self.last_ok = Instant::now();
                }
                Event::Error(message) => {
                    crate::desktop::log_line(&format!("error: {message}"));
                    if self.me_id.is_empty() {
                        // 还没登录成功：回到登录页，把错误留在表单里。
                        self.net = None;
                        self.login_error = Some(message);
                        self.status = "未连接".to_string();
                    } else {
                        self.toast(message.clone(), true);
                        self.status = format!("连接异常 · {message}");
                    }
                }
                Event::Invites(list) => {
                    if let Dialog::Invites {
                        list: items,
                        loading,
                    } = &mut self.dialog
                    {
                        *items = list;
                        *loading = false;
                    }
                }
                Event::Members(users) => {
                    self.store.users = users;
                }
            }
        }
    }

    /// 进入今日时确保当天日报页存在（用它的 id 挂总结）。
    fn ensure_today(&mut self) {
        let today = self.store.today();
        if !self.store.pages.iter().any(|page| page.date.as_deref() == Some(today.as_str())) {
            self.send(Command::EnsureDay { date: today });
        }
    }

    pub fn is_writable(&self) -> bool {
        self.member_filter.is_none() || self.member_filter.as_deref() == Some(self.me_id.as_str())
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(title("Glasswing", 18.0));
            ui.add_space(8.0);
            ui.label(muted(self.store.workspace_name.clone()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            });
        });
    }

    /// 顶部导航模式：一条横排导航，内容区占满整宽。
    fn top_nav(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(title("Glasswing", 17.0));
            ui.add_space(16.0);
            for view in [
                View::Today,
                View::Workspace,
                View::Calendar,
                View::Habits,
                View::Stats,
                View::Inbox,
            ] {
                let active = self.view == view;
                let color = if active { accent() } else { text_dim() };
                let response = ui.add(
                    egui::Button::new(RichText::new(view.label()).size(13.0).color(color))
                        .frame(false),
                );
                if active {
                    let rect = response.rect;
                    ui.painter().hline(
                        rect.x_range(),
                        rect.bottom() + 2.0,
                        egui::Stroke::new(1.6_f32, accent()),
                    );
                }
                if response.clicked() {
                    self.view = view;
                    self.inline = None;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let active = self.view == View::Settings;
                let color = if active { accent() } else { text_dim() };
                if ui
                    .add(egui::Button::new(RichText::new("设置").size(13.0).color(color)).frame(false))
                    .clicked()
                {
                    self.view = View::Settings;
                }
            });
        });
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        for view in [
            View::Today,
            View::Workspace,
            View::Calendar,
            View::Habits,
            View::Stats,
            View::Inbox,
            View::Settings,
        ] {
            let active = self.view == view;
            let width = ui.available_width();
            let (rect, response) =
                ui.allocate_exact_size(egui::Vec2::new(width, 34.0), egui::Sense::click());
            let fill = if response.hovered() && !active {
                Color32::from_rgba_unmultiplied(255, 255, 255, 14)
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, egui::CornerRadius::same(10), fill);
            if active {
                let bar = egui::Rect::from_min_size(
                    egui::pos2(rect.left(), rect.center().y - 9.0),
                    egui::Vec2::new(3.0, 18.0),
                );
                ui.painter().rect_filled(bar, egui::CornerRadius::same(2), accent());
            }
            ui.painter().text(
                egui::pos2(rect.left() + 20.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                view.label(),
                egui::FontId::proportional(13.5),
                if active { accent() } else { text_dim() },
            );
            if response.clicked() {
                self.view = view;
                self.editor = Editor::None;
            }
        }

        ui.add_space(14.0);
        hairline(ui);
        ui.add_space(12.0);

        // 当前时间块（呼应参考图左侧的时间卡）
        let now = Local::now();
        section(ui, "当前时间");
        ui.label(metric(now.format("%H:%M").to_string(), 30.0));
        ui.label(muted(format!(
            "{} · {}",
            now.format("%Y-%m-%d"),
            weekday_cn(now.date_naive())
        )));
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        match self.view {
            View::Today => crate::views::today::draw(self, ui),
            View::Workspace => crate::views::workspace::draw(self, ui),
            View::Calendar => crate::views::calendar::draw(self, ui),
            View::Habits => crate::views::habits::draw(self, ui),
            View::Stats => crate::views::stats::draw(self, ui),
            View::Inbox => crate::views::inbox::draw(self, ui),
            View::Settings => crate::views::settings::draw(self, ui),
        }
    }

    fn login_screen(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(title("Glasswing", 34.0));
            ui.add_space(4.0);
            ui.label(dim("一个人的清晨观测站，也是几个人的共享空间"));
            ui.add_space(28.0);
            egui::Frame::new()
                .fill(card())
                .stroke(egui::Stroke::new(1.0, line()))
                .corner_radius(egui::CornerRadius::same(RADIUS))
                .inner_margin(egui::Margin::same(22))
                .show(ui, |ui| {
                    ui.set_width(360.0);
                    section(ui, "服务器地址");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.server_url)
                            .hint_text("http://your-server:8080")
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(10.0);
                    section(ui, "名字");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.name)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(10.0);
                    ui.checkbox(&mut self.registering, "用邀请码注册（第一次使用）");
                    if self.registering {
                        ui.add_space(6.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.invite_code)
                                .hint_text("AB12-CD34")
                                .desired_width(f32::INFINITY),
                        );
                    }
                    if let Some(error) = &self.login_error {
                        ui.add_space(8.0);
                        ui.label(RichText::new(error).color(danger()).size(13.0));
                    }
                    ui.add_space(16.0);
                    let button = egui::Button::new(RichText::new("连接").size(15.0).color(text()))
                        .fill(accent_soft())
                        .min_size(egui::Vec2::new(ui.available_width(), 36.0));
                    if ui.add(button).clicked() {
                        self.connect_from_form();
                    }
                });
        });
    }

    fn toast_layer(&mut self, ctx: &egui::Context) {
        if let Some((message, at, error)) = &self.toast {
            if at.elapsed() < Duration::from_secs(4) {
                egui::Area::new(egui::Id::new("toast"))
                    .anchor(egui::Align2::CENTER_BOTTOM, egui::Vec2::new(0.0, -24.0))
                    .show(ctx, |ui| {
                        egui::Frame::new()
                            .fill(Color32::from_rgb(20, 27, 39))
                            .stroke(egui::Stroke::new(
                                1.0,
                                if *error { danger() } else { line() },
                            ))
                            .corner_radius(egui::CornerRadius::same(10))
                            .inner_margin(egui::Margin::symmetric(16, 9))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(message.clone())
                                        .size(13.0)
                                        .color(if *error { danger() } else { text() }),
                                );
                            });
                    });
                ctx.request_repaint_after(Duration::from_millis(200));
            } else {
                self.toast = None;
            }
        }
    }
}

impl App {
    /// 第一帧：定尺寸、抓壁纸、挂到桌面层。
    fn setup_desktop(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        if self.desktop_ready {
            return;
        }
        crate::desktop::log_line("setup: begin");
        let Some(hwnd) = crate::desktop::main_window().or_else(|| crate::desktop::hwnd_of(frame)) else {
            crate::desktop::log_line("setup: no hwnd");
            self.desktop_ready = true;
            return;
        };
        self.hwnd = Some(hwnd);
        crate::desktop::log_line(&format!("setup: hwnd={hwnd}"));
        crate::desktop::place_right_half_workarea(hwnd);
        crate::desktop::log_line("setup: placed");
        let rect = crate::desktop::window_rect(hwnd);
        crate::desktop::log_line("setup: loading wallpaper");
        self.backdrop = crate::desktop::wallpaper_path().and_then(|path| {
            let (x, y, w, h) = crate::desktop::window_rect(hwnd)?;
            let scale = ctx.pixels_per_point();
            let margin = (MARGIN * scale) as i32;
            crate::wallpaper::Backdrop::from_wallpaper_rounded(
                ctx,
                &path,
                crate::desktop::screen_size(),
                (x + margin, y + margin, w - margin * 2, h - margin * 2),
                (26.0 * scale).max(14.0),
            )
        });
        crate::desktop::log_line("setup: backdrop done");
        let mode = std::env::var("GLASSWING_ATTACH").unwrap_or_default();
        let window_mode = std::env::var_os("GLASSWING_WINDOW").is_some();
        let attached = if window_mode || mode == "none" {
            false
        } else if mode == "bottom" {
            crate::desktop::anchor_bottom(hwnd);
            false
        } else {
            // 钉在桌面之上、普通窗口之下。
            crate::desktop::clear_appwindow(hwnd);
            crate::desktop::anchor_above_desktop(hwnd);
            true
        };
        crate::desktop::log_line(&format!(
            "hwnd={hwnd} rect={rect:?} backdrop={} attached={attached} class={} {}",
            self.backdrop.is_some(),
            crate::desktop::class_name(hwnd),
            crate::desktop::describe(hwnd)
        ));
        self.desktop_ready = true;
    }

    /// 底色：默认完全透明，只有用户主动调高「玻璃浓度」时才铺一层底。
    fn paint_backdrop(&self, ctx: &egui::Context) {
        let alpha = crate::theme::glass_alpha();
        if alpha == 0 {
            return;
        }
        let panel = ctx.screen_rect().shrink(MARGIN);
        let painter = ctx.layer_painter(egui::LayerId::background());
        if let Some(backdrop) = &self.backdrop {
            painter.image(
                backdrop.texture.id(),
                panel,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
        painter.rect_filled(
            panel,
            egui::CornerRadius::same(26),
            egui::Color32::from_rgba_unmultiplied(226, 240, 252, alpha),
        );
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // 窗口底色必须全透明，否则页边距会露出一层底色。
        egui::Rgba::TRANSPARENT.to_array()
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.setup_desktop(ctx, frame);
        if !self.logged && self.started.elapsed().as_secs() >= 3 {
            self.logged = true;
            if let Some(hwnd) = self.hwnd {
                crate::desktop::log_line(&format!("later: {}", crate::desktop::describe(hwnd)));
                crate::desktop::log_line(&format!("windows: {}", crate::desktop::list_own_windows()));
            }
        }
        if self.desktop_ready && self.last_anchor.elapsed().as_secs() >= 2 {
            self.last_anchor = Instant::now();
            if let Some(hwnd) = self.hwnd {
                crate::desktop::anchor_above_desktop(hwnd);
            }
        }
        self.poll();
        self.paint_backdrop(ctx);
        // 每秒刷新时钟与同步状态
        ctx.request_repaint_after(Duration::from_secs(1));
        if self.last_ok.elapsed() > Duration::from_secs(20) && self.status.starts_with("已连接") {
            self.status = "等待同步…".to_string();
        }

        if self.net.is_none() && self.store.users.is_empty() && self.login_error.is_some() {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(Color32::TRANSPARENT).inner_margin(egui::Margin::same(0)))
                .show(ctx, |ui| self.login_screen(ui));
            return;
        }
        if self.me.is_empty() && self.store.workspace_name.is_empty() {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(Color32::TRANSPARENT).inner_margin(egui::Margin::same(0)))
                .show(ctx, |ui| self.login_screen(ui));
            return;
        }

        let nav_top = crate::theme::palette().nav_top;
        let top = egui::TopBottomPanel::top("top")
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(Color32::TRANSPARENT)
                    .inner_margin(egui::Margin {
                        left: 28,
                        right: 28,
                        top: 18,
                        bottom: 6,
                    }),
            )
            .show(ctx, |ui| {
                if nav_top {
                    self.top_nav(ui);
                } else {
                    self.top_bar(ui);
                }
            });
        let _ = top;

        if !nav_top {
            let side = egui::SidePanel::left("side")
                .show_separator_line(false)
                .resizable(false)
                .exact_width(172.0)
                .frame(
                    egui::Frame::new()
                        .fill(Color32::TRANSPARENT)
                        .inner_margin(egui::Margin {
                            left: 24,
                            right: 12,
                            top: 10,
                            bottom: 16,
                        }),
                )
                .show(ctx, |ui| self.sidebar(ui));
            let _ = side;
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::TRANSPARENT)
                    .inner_margin(egui::Margin {
                        left: 24,
                        right: 34,
                        top: 10,
                        bottom: 26,
                    }),
            )
            .show(ctx, |ui| {
                self.content(ui);
            });

        crate::views::overlays::draw(self, ctx);
        self.toast_layer(ctx);
    }
}

pub const MARGIN: f32 = 12.0;

pub fn weekday_cn(date: NaiveDate) -> &'static str {
    const NAMES: [&str; 7] = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
    NAMES[date.weekday().num_days_from_monday() as usize]
}
