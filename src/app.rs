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
    Search {
        query: String,
    },
    Settings,
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
    toast: Option<(String, Instant, bool)>,
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
            view: View::Today,
            date: Local::now().date_naive(),
            selected_page: None,
            selected_habit: None,
            member_filter: None,
            member_name: String::new(),
            editor: Editor::None,
            dialog: Dialog::None,
            toast: None,
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
                    self.toast(message.clone(), true);
                    self.status = format!("连接异常 · {message}");
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
            ui.add_space(4.0);
            ui.label(title("Glasswing", 19.0));
            ui.add_space(6.0);
            ui.label(muted("·"));
            ui.label(dim(self.store.workspace_name.clone()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("设置").clicked() {
                    self.dialog = Dialog::Settings;
                }
                ui.add_space(4.0);
                let (dot, text) = if self.status.starts_with("已连接") {
                    (OK, self.status.clone())
                } else {
                    (WARN, self.status.clone())
                };
                // 状态胶囊
                egui::Frame::new()
                    .fill(Color32::from_rgba_premultiplied(dot.r(), dot.g(), dot.b(), 24))
                    .corner_radius(egui::CornerRadius::same(9))
                    .inner_margin(egui::Margin::symmetric(10, 5))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(
                                egui::Vec2::new(6.0, 6.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().circle_filled(rect.center(), 3.0, dot);
                            ui.label(RichText::new(text).size(12.0).color(dot));
                        });
                    });
                ui.add_space(6.0);
                if ui.button("同步").clicked() {
                    self.send(Command::SyncNow);
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
        ] {
            let active = self.view == view;
            let width = ui.available_width();
            let (rect, response) =
                ui.allocate_exact_size(egui::Vec2::new(width, 38.0), egui::Sense::click());
            let fill = if active {
                Color32::from_rgba_premultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), 26)
            } else if response.hovered() {
                CARD_HI
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, egui::CornerRadius::same(10), fill);
            if active {
                let bar = egui::Rect::from_min_size(
                    egui::pos2(rect.left(), rect.center().y - 9.0),
                    egui::Vec2::new(3.0, 18.0),
                );
                ui.painter().rect_filled(bar, egui::CornerRadius::same(2), ACCENT);
            }
            ui.painter().text(
                egui::pos2(rect.left() + 20.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                view.label(),
                egui::FontId::proportional(14.5),
                if active { TEXT } else { TEXT_DIM },
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
        ui.add_space(12.0);

        let (done, total) = self.store.day_completion(&self.date.format("%Y-%m-%d").to_string());
        section(ui, "今日进度");
        ui.horizontal(|ui| {
            ui.label(metric(format!("{done}/{total}"), 18.0));
            ui.label(muted(if total == 0 {
                "还没有待办".to_string()
            } else {
                format!("{}%", done * 100 / total.max(1))
            }));
        });
        progress(
            ui,
            if total == 0 { 0.0 } else { done as f32 / total as f32 },
            ACCENT,
        );

        ui.add_space(14.0);
        section(ui, "成员视角");
        let mut changed = None;
        egui::ComboBox::from_id_salt("member")
            .width(ui.available_width() - 8.0)
            .selected_text(if self.member_filter.is_some() {
                format!("{}（只读）", self.member_name)
            } else {
                "共享空间".to_string()
            })
            .show_ui(ui, |ui| {
                if ui.selectable_label(self.member_filter.is_none(), "共享空间").clicked() {
                    changed = Some(None);
                }
                for user in self.store.users.clone() {
                    let selected = self.member_filter.as_deref() == Some(user.id.as_str());
                    let label = if user.id == self.me_id {
                        format!("{}（我）", user.name)
                    } else {
                        user.name.clone()
                    };
                    if ui.selectable_label(selected, label).clicked() {
                        changed = Some(Some((user.id.clone(), user.name.clone())));
                    }
                }
            });
        if let Some(change) = changed {
            match change {
                Some((id, name)) => {
                    self.member_filter = Some(id);
                    self.member_name = name;
                }
                None => {
                    self.member_filter = None;
                    self.member_name.clear();
                }
            }
        }
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        match self.view {
            View::Today => crate::views::today::draw(self, ui),
            View::Workspace => crate::views::workspace::draw(self, ui),
            View::Calendar => crate::views::calendar::draw(self, ui),
            View::Habits => crate::views::habits::draw(self, ui),
            View::Stats => crate::views::stats::draw(self, ui),
            View::Inbox => crate::views::inbox::draw(self, ui),
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
                .fill(CARD)
                .stroke(egui::Stroke::new(1.0, LINE))
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
                        ui.label(RichText::new(error).color(DANGER).size(13.0));
                    }
                    ui.add_space(16.0);
                    let button = egui::Button::new(RichText::new("连接").size(15.0).color(TEXT))
                        .fill(ACCENT_SOFT)
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
                                if *error { DANGER } else { LINE },
                            ))
                            .corner_radius(egui::CornerRadius::same(10))
                            .inner_margin(egui::Margin::symmetric(16, 9))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(message.clone())
                                        .size(13.0)
                                        .color(if *error { DANGER } else { TEXT }),
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

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        // 每秒刷新时钟与同步状态
        ctx.request_repaint_after(Duration::from_secs(1));
        if self.last_ok.elapsed() > Duration::from_secs(20) && self.status.starts_with("已连接") {
            self.status = "等待同步…".to_string();
        }

        if self.net.is_none() && self.store.users.is_empty() && self.login_error.is_some() {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::same(0)))
                .show(ctx, |ui| self.login_screen(ui));
            return;
        }
        if self.me.is_empty() && self.store.workspace_name.is_empty() {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::same(0)))
                .show(ctx, |ui| self.login_screen(ui));
            return;
        }

        let top = egui::TopBottomPanel::top("top")
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(18, 10)),
            )
            .show(ctx, |ui| self.top_bar(ui));
        let _ = top;

        let side = egui::SidePanel::left("side")
            .resizable(false)
            .exact_width(232.0)
            .frame(
                egui::Frame::new()
                    .fill(SIDEBAR)
                    .inner_margin(egui::Margin::symmetric(14, 8)),
            )
            .show(ctx, |ui| self.sidebar(ui));
        let _ = side;

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(20, 14)),
            )
            .show(ctx, |ui| {
                self.content(ui);
            });

        crate::views::overlays::draw(self, ctx);
        self.toast_layer(ctx);
    }
}

pub fn weekday_cn(date: NaiveDate) -> &'static str {
    const NAMES: [&str; 7] = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
    NAMES[date.weekday().num_days_from_monday() as usize]
}
