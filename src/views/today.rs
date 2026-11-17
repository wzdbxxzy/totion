//! 今日：一列到底的极简仪表盘 —— 待办、习惯、当日总结，不再套卡片框。

use chrono::{Duration as ChronoDuration, Local};
use eframe::egui::{self, Align, Layout, RichText};
use totion_core::{CommentTargetKind, JournalTarget};

use crate::app::{weekday_cn, App, Editor};
use crate::net::Command;
use crate::theme::*;

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    let date_key = app.date.format("%Y-%m-%d").to_string();
    header(app, ui, &date_key);
    ui.add_space(10.0);
    stats_strip(app, ui, &date_key);
    ui.add_space(6.0);
    hairline(ui);
    todo_section(app, ui, &date_key);
    hairline(ui);
    habit_section(app, ui, &date_key);
    hairline(ui);
    summary_section(app, ui, &date_key);
}

fn header(app: &mut App, ui: &mut egui::Ui, date_key: &str) {
    ui.horizontal(|ui| {
        ui.horizontal(|ui| {
            ui.label(title(
                format!("{} {}", app.date.format("%m-%d"), weekday_cn(app.date)),
                27.0,
            ));
            if app.date == Local::now().date_naive() {
                pill(ui, "今天", accent());
            }
            if !app.is_writable() {
                pill(ui, "只读", warn());
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("＋ 待办").clicked() {
                app.editor = Editor::Block {
                    block: None,
                    text: String::new(),
                    time: String::new(),
                    date: date_key.to_string(),
                };
            }
            if ui.button("‹").clicked() {
                app.date -= ChronoDuration::days(1);
            }
            if ui.button("›").clicked() {
                app.date += ChronoDuration::days(1);
            }
        });
    });
}

fn stats_strip(app: &App, ui: &mut egui::Ui, date_key: &str) {
    let (done, total) = app.store.day_completion(date_key);
    let habits = app.store.active_habits();
    let checked = habits
        .iter()
        .filter(|habit| app.store.checkin(&habit.id, date_key).is_some())
        .count();
    let summary = app.store.daily_summary(date_key).is_some();
    let open_comments = app.store.open_comments().len();
    ui.horizontal(|ui| {
        stat(ui, "待办", &format!("{done}/{total}"));
        stat(ui, "习惯", &format!("{checked}/{}", habits.len()));
        stat(ui, "总结", if summary { "已写" } else { "未写" });
        stat(ui, "留言", &open_comments.to_string());
    });
}

fn stat(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(metric(value, 16.0));
        ui.label(muted(label));
    });
    ui.add_space(20.0);
}

fn todo_section(app: &mut App, ui: &mut egui::Ui, date_key: &str) {
    let items = app.store.day_blocks(date_key);
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        section(ui, "待办");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if items.is_empty() {
                ui.label(muted("用右上角「＋ 待办」加一条"));
            }
        });
    });
    ui.add_space(4.0);
    if items.is_empty() {
        empty_state(ui, "今天还没有待办", "空着也是一种安排");
        return;
    }
    let mut toggle: Option<(String, i64, bool)> = None;
    let mut edit: Option<totion_core::Block> = None;
    for block in &items {
        ui.horizontal(|ui| {
            let (rect, response) =
                ui.allocate_exact_size(egui::Vec2::new(18.0, 18.0), egui::Sense::click());
            let stroke = if block.checked {
                egui::Stroke::new(1.5, ok())
            } else {
                egui::Stroke::new(1.2, text_muted())
            };
            ui.painter().circle_stroke(rect.center(), 7.0, stroke);
            if block.checked {
                ui.painter().circle_filled(rect.center(), 4.0, ok());
            }
            if response.clicked() {
                toggle = Some((block.id.clone(), block.version, !block.checked));
            }

            let text = if block.checked {
                RichText::new(block.text.clone())
                    .color(text_muted())
                    .strikethrough()
            } else {
                RichText::new(block.text.clone()).color(text())
            };
            let label = ui.add(egui::Label::new(text).truncate());
            if label.clicked() {
                edit = Some(block.clone());
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(time) = &block.start_time {
                    ui.label(muted(time.clone()));
                }
                let comments = app
                    .store
                    .comments_for(CommentTargetKind::Block, &block.id)
                    .iter()
                    .filter(|c| !c.resolved)
                    .count();
                if comments > 0 {
                    ui.label(muted(format!("留言 {comments}")));
                }
                if !app.store.journals_for(JournalTarget::Block, &block.id).is_empty() {
                    ui.label(muted("总结"));
                }
            });
        });
        ui.add_space(7.0);
    }
    if let Some((id, version, checked)) = toggle {
        if app.is_writable() {
            app.send(Command::UpdateBlock {
                id,
                version,
                text: None,
                checked: Some(checked),
                deleted: false,
            });
        } else {
            app.toast("他人视角只读，可留言", true);
        }
    }
    if let Some(block) = edit {
        if app.is_writable() {
            app.editor = Editor::Block {
                block: Some(block),
                text: String::new(),
                time: String::new(),
                date: date_key.to_string(),
            };
        } else {
            open_comments(app, CommentTargetKind::Block, &block.id, &block.text);
        }
    }
}

fn habit_section(app: &mut App, ui: &mut egui::Ui, date_key: &str) {
    let habits = app.store.active_habits();
    let today = Local::now().date_naive();
    let mut pending: Option<(String, Option<String>)> = None;
    ui.add_space(10.0);
    section(ui, "习惯");
    ui.add_space(4.0);
    if habits.is_empty() {
        empty_state(ui, "还没有习惯", "到「习惯」里新建一个");
        return;
    }
    for habit in &habits {
        let checkin = app.store.checkin(&habit.id, date_key);
        ui.horizontal(|ui| {
            let label = if checkin.is_some() {
                RichText::new(habit.title.clone()).color(text())
            } else {
                RichText::new(habit.title.clone()).color(text_dim())
            };
            ui.add(egui::Label::new(label).truncate());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let text = if checkin.is_some() { "撤销" } else { "打卡" };
                let color = if checkin.is_some() { text_muted() } else { ok() };
                if ui
                    .add(egui::Button::new(RichText::new(text).size(12.0).color(color)).frame(false))
                    .clicked()
                {
                    pending = Some((habit.id.clone(), checkin.map(|c| c.id.clone())));
                }
                ui.label(muted(format!(
                    "{} 天",
                    app.store.habit_stats(habit, today).current_streak
                )));
            });
        });
        ui.add_space(6.0);
    }
    if let Some((habit_id, checkin_id)) = pending {
        if !app.is_writable() {
            app.toast("他人视角只读", true);
            return;
        }
        match checkin_id {
            Some(id) => {
                let version = app
                    .store
                    .checkins
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.version)
                    .unwrap_or(0);
                app.send(Command::UndoCheckIn { id, version });
            }
            None => app.send(Command::CheckIn {
                habit_id,
                date: date_key.to_string(),
            }),
        }
    }
}

fn summary_section(app: &mut App, ui: &mut egui::Ui, date_key: &str) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        section(ui, "当日总结");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add(egui::Button::new(muted("编辑")).frame(false))
                .clicked()
            {
                open_daily_summary(app, date_key);
            }
        });
    });
    ui.add_space(4.0);
    match app.store.daily_summary(date_key) {
        Some(journal) => {
            ui.label(body(journal.body.clone()));
            ui.add_space(4.0);
            ui.label(muted(format!(
                "{} {} · {}",
                journal.date, journal.time, journal.author_name
            )));
        }
        None => empty_state(ui, "还没写今天的总结", "一句话也算"),
    }
}

fn open_daily_summary(app: &mut App, date_key: &str) {
    let target = date_key.to_string();
    let existing = app
        .store
        .daily_summary(date_key)
        .map(|journal| (journal.body.clone(), journal.version));
    app.editor = Editor::Journal {
        target_kind: JournalTarget::Daily,
        target_id: target,
        label: format!("{} 的总结", date_key),
        text: existing
            .as_ref()
            .map(|(body, _)| body.clone())
            .unwrap_or_default(),
        existing,
    };
}

pub fn open_comments(app: &mut App, kind: CommentTargetKind, id: &str, label: &str) {
    app.dialog = crate::app::Dialog::Comments {
        kind,
        id: id.to_string(),
        label: label.to_string(),
        draft: String::new(),
    };
}

pub fn empty_state(ui: &mut egui::Ui, title: &str, hint: &str) {
    ui.label(dim(title));
    ui.label(muted(hint));
    ui.add_space(6.0);
}

pub fn rel_time(at: chrono::DateTime<chrono::Utc>) -> String {
    let seconds = (chrono::Utc::now() - at).num_seconds();
    if seconds < 60 {
        "刚刚".to_string()
    } else if seconds < 3600 {
        format!("{} 分钟前", seconds / 60)
    } else if seconds < 86_400 {
        format!("{} 小时前", seconds / 3600)
    } else {
        format!("{} 天前", seconds / 86_400)
    }
}
