//! 习惯：卡片网格、热力图、按日打卡与归档。

use chrono::{Duration as ChronoDuration, Local, NaiveDate};
use eframe::egui::{self, Align, Color32, Layout, RichText};
use totion_core::{CommentTargetKind, Habit, HabitCheckin};

use crate::app::{weekday_cn, App, Editor};
use crate::net::Command;
use crate::theme::*;
use crate::views::today::{empty_state, open_comments};

const CELL: f32 = 10.0;
const GAP: f32 = 3.0;
const WEEKS: usize = 12;

/// 卡片里可能触发的动作，统一在渲染结束后发出，避免边画边改状态。
enum CardAction {
    CheckIn {
        habit_id: String,
        existing: Option<(String, i64)>,
    },
    Archive {
        id: String,
        version: i64,
    },
    Rename {
        id: String,
        version: i64,
        title: String,
    },
    Comment,
}

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    header(app, ui);
    ui.add_space(14.0);

    let habits = app.store.active_habits();
    if habits.is_empty() {
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            empty_state(ui, "还没有习惯", "点右上角「＋ 新建习惯」开始第一个");
        });
    } else {
        let width = ui.available_width();
        let columns = if width > 1100.0 { 2 } else { 1 };
        let spacing = ui.spacing().item_spacing.x;
        let card_width =
            ((width - spacing * (columns - 1) as f32) / columns as f32).max(300.0);
        for chunk in habits.chunks(columns) {
            ui.horizontal_top(|ui| {
                for habit in chunk {
                    ui.allocate_ui_with_layout(
                        egui::vec2(card_width, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| habit_card(app, ui, habit),
                    );
                }
            });
            ui.add_space(12.0);
        }
    }

    ui.add_space(2.0);
    archived_section(app, ui);
}

fn header(app: &mut App, ui: &mut egui::Ui) {
    let today = Local::now().date_naive();
    let active = app
        .store
        .habits
        .iter()
        .filter(|habit| !habit.deleted && !habit.archived)
        .count();
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(title("习惯", 26.0));
                pill(ui, &format!("{active} 个进行中"), accent());
                if !app.is_writable() {
                    pill(ui, "只读视角", text_dim());
                }
            });
            ui.label(muted(format!(
                "{} {} · 连续天数与打卡",
                app.date.format("%Y-%m-%d"),
                weekday_cn(app.date)
            )));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let new_habit =
                egui::Button::new(RichText::new("＋ 新建习惯").color(text())).fill(accent_soft());
            if ui.add(new_habit).clicked() {
                app.editor = Editor::Habit {
                    title: String::new(),
                    icon: "◇".to_string(),
                    weekly: false,
                };
            }
            if ui.button("今天").clicked() {
                app.date = today;
            }
            if ui.button("›").clicked() {
                app.date += ChronoDuration::days(1);
            }
            if ui.button("‹").clicked() {
                app.date -= ChronoDuration::days(1);
            }
        });
    });
}

fn habit_card(app: &mut App, ui: &mut egui::Ui, habit: &Habit) {
    let today = Local::now().date_naive();
    let date_key = app.date.format("%Y-%m-%d").to_string();
    let checkin = app.store.checkin(&habit.id, &date_key).cloned();
    let stats = app.store.habit_stats(habit, today);
    let selected = app.selected_habit.as_deref() == Some(habit.id.as_str());
    let rename_id = ui.id().with(("rename_habit", &habit.id));
    let mut action: Option<CardAction> = None;

    let mut frame = card_frame();
    if selected {
        frame = frame.stroke(egui::Stroke::new(1.0, accent()));
    }
    let card = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(title(display_name(habit), 19.0));
            pill(ui, &habit.frequency.label(), accent());
        });
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            reading(ui, stats.current_streak.to_string(), "连续天");
            ui.add_space(22.0);
            reading(ui, stats.longest_streak.to_string(), "最长天");
            ui.add_space(22.0);
            reading(
                ui,
                format!("{}%", (stats.last_30_rate * 100.0).round() as i64),
                "近 30 天",
            );
        });
        ui.add_space(14.0);

        heatmap(ui, &app.store.checkins, &habit.id, today);
        ui.add_space(14.0);
        hairline(ui);
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            let checked = checkin.is_some();
            let button = egui::Button::new(
                RichText::new(if checked { "撤销" } else { "打卡" })
                    .size(12.0)
                    .color(if checked { text_dim() } else { ok() }),
            )
            .fill(Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(1.0, line()));
            if ui.add(button).clicked() {
                action = Some(CardAction::CheckIn {
                    habit_id: habit.id.clone(),
                    existing: checkin.as_ref().map(|item| (item.id.clone(), item.version)),
                });
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(muted("归档")).frame(false))
                    .clicked()
                {
                    action = Some(CardAction::Archive {
                        id: habit.id.clone(),
                        version: habit.version,
                    });
                }
                if ui
                    .add(egui::Button::new(muted("改名")).frame(false))
                    .clicked()
                {
                    if app.is_writable() {
                        ui.data_mut(|data| data.insert_temp(rename_id, habit.title.clone()));
                    } else {
                        app.toast("他人视角只读，可留言", true);
                    }
                }
                if ui
                    .add(egui::Button::new(muted("留言")).frame(false))
                    .clicked()
                {
                    action = Some(CardAction::Comment);
                }
            });
        });

        let draft = ui.data(|data| data.get_temp::<String>(rename_id));
        if let Some(mut draft) = draft {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut draft).desired_width(180.0));
                if ui.add(egui::Button::new(muted("保存")).frame(false)).clicked() {
                    let trimmed = draft.trim().to_string();
                    if !trimmed.is_empty() && trimmed != habit.title {
                        action = Some(CardAction::Rename {
                            id: habit.id.clone(),
                            version: habit.version,
                            title: trimmed,
                        });
                    }
                    ui.data_mut(|data| data.remove_temp::<String>(rename_id));
                } else if ui
                    .add(egui::Button::new(muted("取消")).frame(false))
                    .clicked()
                {
                    ui.data_mut(|data| data.remove_temp::<String>(rename_id));
                } else {
                    ui.data_mut(|data| data.insert_temp(rename_id, draft));
                }
            });
        }
    });

    if card.response.interact(egui::Sense::click()).clicked() {
        app.selected_habit = Some(habit.id.clone());
    }

    match action {
        Some(CardAction::Comment) => {
            open_comments(app, CommentTargetKind::Habit, &habit.id, &habit.title);
        }
        Some(CardAction::CheckIn {
            habit_id,
            existing,
        }) => {
            if !app.is_writable() {
                app.toast("他人视角只读，可留言", true);
            } else {
                match existing {
                    Some((id, version)) => app.send(Command::UndoCheckIn { id, version }),
                    None => app.send(Command::CheckIn {
                        habit_id,
                        date: date_key.clone(),
                    }),
                }
            }
        }
        Some(CardAction::Archive { id, version }) => {
            if app.is_writable() {
                app.send(Command::UpdateHabit {
                    id,
                    version,
                    title: None,
                    archived: Some(true),
                    deleted: false,
                });
            } else {
                app.toast("他人视角只读", true);
            }
        }
        Some(CardAction::Rename { id, version, title }) => {
            if app.is_writable() {
                app.send(Command::UpdateHabit {
                    id,
                    version,
                    title: Some(title),
                    archived: None,
                    deleted: false,
                });
            } else {
                app.toast("他人视角只读", true);
            }
        }
        None => {}
    }
}

fn archived_section(app: &mut App, ui: &mut egui::Ui) {
    let archived: Vec<Habit> = app
        .store
        .habits
        .iter()
        .filter(|habit| !habit.deleted && habit.archived)
        .cloned()
        .collect();
    let open_id = egui::Id::new("habits_archived_open");
    let mut open = ui.data(|data| data.get_temp::<bool>(open_id)).unwrap_or(false);
    let mut restore: Option<(String, i64)> = None;

    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let label = format!("已归档（{}）", archived.len());
            if ui
                .add(egui::Button::new(dim(label)).frame(false))
                .clicked()
            {
                open = !open;
                ui.data_mut(|data| data.insert_temp(open_id, open));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(muted(if open { "收起" } else { "展开" }));
            });
        });
        if !open {
            return;
        }
        ui.add_space(4.0);
        if archived.is_empty() {
            empty_state(ui, "没有已归档的习惯", "归档后会出现在这里，可随时恢复");
            return;
        }
        for habit in &archived {
            ui.horizontal(|ui| {
                ui.label(dim(display_name(habit)));
                pill(ui, "已归档", text_muted());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(egui::Button::new(muted("恢复")).frame(false))
                        .clicked()
                    {
                        restore = Some((habit.id.clone(), habit.version));
                    }
                });
            });
        }
    });

    if let Some((id, version)) = restore {
        if app.is_writable() {
            app.send(Command::UpdateHabit {
                id,
                version,
                title: None,
                archived: Some(false),
                deleted: false,
            });
        } else {
            app.toast("他人视角只读", true);
        }
    }
}

fn reading(ui: &mut egui::Ui, value: String, label: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(metric(value, 20.0));
        ui.label(muted(label));
    });
}

/// 近 12 周热力图：7 行（周一在上）× 12 列，格子 10×10、间距 3。
fn heatmap(ui: &mut egui::Ui, checkins: &[HabitCheckin], habit_id: &str, today: NaiveDate) {
    let grid = totion_core::heatmap_grid(checkins, habit_id, WEEKS as u32, today);
    let (_, week_end) = totion_core::week_range(today);
    let start = week_end - ChronoDuration::days(WEEKS as i64 * 7 - 1);
    let size = egui::vec2(
        WEEKS as f32 * CELL + (WEEKS - 1) as f32 * GAP,
        7.0 * CELL + 6.0 * GAP,
    );
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter();
    let empty = Color32::from_rgba_premultiplied(255, 255, 255, 14);
    for week in 0..WEEKS {
        for day in 0..7 {
            let date = start + ChronoDuration::days((week * 7 + day) as i64);
            if date > today {
                continue;
            }
            let done = grid
                .get(week)
                .and_then(|column| column.get(day))
                .copied()
                .unwrap_or(false);
            let min = egui::pos2(
                rect.left() + week as f32 * (CELL + GAP),
                rect.top() + day as f32 * (CELL + GAP),
            );
            painter.rect_filled(
                egui::Rect::from_min_size(min, egui::vec2(CELL, CELL)),
                egui::CornerRadius::same(2),
                if done { ok() } else { empty },
            );
        }
    }
}

fn display_name(habit: &Habit) -> String {
    let icon = habit.icon.trim();
    if icon.is_empty() {
        habit.title.clone()
    } else {
        format!("{} {}", icon, habit.title)
    }
}
