//! 今日：卡片式仪表盘 —— 待办清单 + 习惯打卡 + 当日总结 + 未解决留言。

use chrono::{Duration as ChronoDuration, Local};
use eframe::egui::{self, Align, Color32, Layout, RichText};
use totion_core::{CommentTargetKind, JournalTarget};

use crate::app::{weekday_cn, App, Editor};
use crate::net::Command;
use crate::theme::*;

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    let date = app.date;
    let date_key = date.format("%Y-%m-%d").to_string();
    header(app, ui, &date_key);
    ui.add_space(14.0);

    let width = ui.available_width();
    let columns = if width > 980.0 { 2 } else { 1 };
    if columns == 2 {
        ui.horizontal_top(|ui| {
            let left = width * 0.58 - 10.0;
            ui.allocate_ui_with_layout(
                egui::Vec2::new(left, ui.available_height()),
                Layout::top_down(Align::Min),
                |ui| todo_card(app, ui, &date_key),
            );
            ui.add_space(6.0);
            ui.vertical(|ui| {
                summary_card(app, ui, &date_key);
                ui.add_space(12.0);
                habit_card(app, ui, &date_key);
                ui.add_space(12.0);
                inbox_card(app, ui);
            });
        });
    } else {
        todo_card(app, ui, &date_key);
        ui.add_space(12.0);
        summary_card(app, ui, &date_key);
        ui.add_space(12.0);
        habit_card(app, ui, &date_key);
    }
}

fn header(app: &mut App, ui: &mut egui::Ui, date_key: &str) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(title(
                    format!("{} {}", app.date.format("%m-%d"), weekday_cn(app.date)),
                    26.0,
                ));
                if app.date == Local::now().date_naive() {
                    pill(ui, "今天", ACCENT);
                }
                if !app.is_writable() {
                    pill(ui, "只读视角", WARN);
                }
            });
            let (done, total) = app.store.day_completion(date_key);
            ui.label(muted(format!(
                "{} 条待办 · 已完成 {} · {}%",
                total,
                done,
                if total == 0 { 0 } else { done * 100 / total }
            )));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("写总结").clicked() {
                open_daily_summary(app, date_key);
            }
            if ui.button("新建习惯").clicked() {
                app.editor = Editor::Habit {
                    title: String::new(),
                    icon: "◇".to_string(),
                    weekly: false,
                };
            }
            let new_todo = egui::Button::new(RichText::new("+ 新建待办").color(TEXT))
                .fill(ACCENT_SOFT);
            if ui.add(new_todo).clicked() {
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

fn open_daily_summary(app: &mut App, date_key: &str) {
    let page_id = app
        .store
        .pages
        .iter()
        .find(|page| page.date.as_deref() == Some(date_key))
        .map(|page| page.id.clone())
        .unwrap_or_else(|| date_key.to_string());
    let existing = app
        .store
        .daily_summary(date_key)
        .map(|journal| (journal.body.clone(), journal.version));
    app.editor = Editor::Journal {
        target_kind: JournalTarget::Daily,
        target_id: page_id,
        label: format!("{} 的总结", date_key),
        text: existing.as_ref().map(|(body, _)| body.clone()).unwrap_or_default(),
        existing,
    };
}

fn todo_card(app: &mut App, ui: &mut egui::Ui, date_key: &str) {
    let items = app.store.day_blocks(date_key);
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            section(ui, "待办");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (done, total) = app.store.day_completion(date_key);
                ui.label(muted(format!("{done}/{total}")));
            });
        });
        ui.add_space(6.0);
        if items.is_empty() {
            empty_state(ui, "今天还没有待办", "用右上角「新建待办」加一条");
            return;
        }
        let mut toggle: Option<(String, i64, bool)> = None;
        let mut edit: Option<totion_core::Block> = None;
        for block in &items {
            let row = egui::Frame::new()
                .fill(Color32::TRANSPARENT)
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(8, 7));
            let response = row
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let size = egui::Vec2::new(18.0, 18.0);
                        let (rect, response) =
                            ui.allocate_exact_size(size, egui::Sense::click());
                        let stroke = if block.checked {
                            egui::Stroke::new(1.5, OK)
                        } else {
                            egui::Stroke::new(1.2, TEXT_MUTED)
                        };
                        ui.painter().circle_stroke(rect.center(), 7.5, stroke);
                        if block.checked {
                            ui.painter().circle_filled(rect.center(), 4.5, OK);
                        }
                        if response.clicked() {
                            toggle = Some((block.id.clone(), block.version, !block.checked));
                        }

                        ui.vertical(|ui| {
                            let text = if block.checked {
                                RichText::new(block.text.clone())
                                    .color(TEXT_MUTED)
                                    .strikethrough()
                            } else {
                                RichText::new(block.text.clone()).color(TEXT)
                            };
                            ui.label(text);
                            let mut meta = Vec::new();
                            if let Some(time) = &block.start_time {
                                meta.push(time.clone());
                            }
                            if block.checked {
                                if let Some(at) = block.completed_at {
                                    meta.push(format!(
                                        "完成于 {}",
                                        at.with_timezone(&Local).format("%H:%M")
                                    ));
                                }
                            } else {
                                meta.push(format!("{} 建", block.updated_by));
                            }
                            ui.label(muted(meta.join(" · ")));
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let summary = app
                                .store
                                .journals_for(JournalTarget::Block, &block.id)
                                .len();
                            if summary > 0 {
                                pill(ui, &format!("总结 {summary}"), ACCENT);
                            }
                            let comments = app
                                .store
                                .comments_for(CommentTargetKind::Block, &block.id)
                                .iter()
                                .filter(|c| !c.resolved)
                                .count();
                            if comments > 0 {
                                pill(ui, &format!("留言 {comments}"), WARN);
                            }
                            if ui
                                .add(egui::Button::new(muted("编辑")).frame(false))
                                .clicked()
                            {
                                edit = Some(block.clone());
                            }
                        });
                    });
                })
                .response;
            if response.interact(egui::Sense::click()).clicked() && edit.is_none() {
                // 点击空白处也进入编辑
            }
            ui.add_space(2.0);
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
    });
}

fn summary_card(app: &mut App, ui: &mut egui::Ui, date_key: &str) {
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            section(ui, "当日总结");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(egui::Button::new(muted("编辑")).frame(false)).clicked() {
                    open_daily_summary(app, date_key);
                }
            });
        });
        ui.add_space(4.0);
        match app.store.daily_summary(date_key) {
            Some(journal) => {
                ui.label(body(journal.body.clone()));
                ui.add_space(6.0);
                ui.label(muted(format!(
                    "{} {} · {}",
                    journal.date, journal.time, journal.author_name
                )));
            }
            None => empty_state(ui, "还没写今天的总结", "一句话也算，按「写总结」"),
        }
    });
}

fn habit_card(app: &mut App, ui: &mut egui::Ui, date_key: &str) {
    let habits = app.store.active_habits();
    let today = Local::now().date_naive();
    let mut pending: Option<(String, Option<String>)> = None;
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section(ui, "习惯打卡");
        ui.add_space(4.0);
        if habits.is_empty() {
            empty_state(ui, "还没有习惯", "右上角「新建习惯」开始第一个");
            return;
        }
        for habit in &habits {
            let checkin = app.store.checkin(&habit.id, date_key);
            ui.horizontal(|ui| {
                let label = if checkin.is_some() {
                    RichText::new(format!("{} {}", habit.icon, habit.title)).color(TEXT)
                } else {
                    RichText::new(format!("{} {}", habit.icon, habit.title)).color(TEXT_DIM)
                };
                ui.label(label);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let stats = app.store.habit_stats(habit, today);
                    ui.label(muted(format!("{} 天", stats.current_streak)));
                    let text = if checkin.is_some() { "撤销" } else { "打卡" };
                    let button = egui::Button::new(
                        RichText::new(text)
                            .size(12.0)
                            .color(if checkin.is_some() { TEXT_DIM } else { OK }),
                    )
                    .fill(Color32::TRANSPARENT)
                    .stroke(egui::Stroke::new(1.0, LINE));
                    if ui.add(button).clicked() {
                        pending = Some((
                            habit.id.clone(),
                            checkin.map(|c| c.id.clone()),
                        ));
                    }
                });
            });
            ui.add_space(2.0);
        }
    });
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

fn inbox_card(app: &mut App, ui: &mut egui::Ui) {
    let comments = app.store.open_comments();
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            section(ui, "未解决留言");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(egui::Button::new(muted("全部")).frame(false)).clicked() {
                    app.view = crate::app::View::Inbox;
                }
            });
        });
        ui.add_space(4.0);
        if comments.is_empty() {
            empty_state(ui, "没有待处理留言", "干净得像清晨的窗");
            return;
        }
        for comment in comments.iter().take(3) {
            ui.label(dim(comment.body.lines().next().unwrap_or("").to_string()));
            ui.label(muted(format!(
                "{} · {}",
                comment.author_name,
                rel_time(comment.created_at)
            )));
            ui.add_space(4.0);
        }
    });
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
    ui.add_space(8.0);
    ui.label(dim(title));
    ui.label(muted(hint));
    ui.add_space(8.0);
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
