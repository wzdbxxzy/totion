//! 统计：完成率、每日完成数柱状图、全部打卡热力图与习惯概览。

use std::collections::HashMap;

use chrono::{Duration as ChronoDuration, Local, NaiveDate};
use eframe::egui::{self, Align, Color32, Layout, RichText};

use totion_core::{DayStat, HabitCheckin};

use crate::app::{App, View};
use crate::theme::*;
use crate::views::today::empty_state;

const CELL: f32 = 10.0;
const GAP: f32 = 3.0;
const RANGE_ID: &str = "stats_range_days";

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    let days = header(ui);
    ui.add_space(14.0);

    let today = Local::now().date_naive();
    let series = totion_core::completion_series(&app.store.blocks, days, today);
    let done: u32 = series.iter().map(|(_, stat)| stat.done).sum();
    let total: u32 = series.iter().map(|(_, stat)| stat.total).sum();

    let width = ui.available_width();
    let columns = if width > 980.0 { 2 } else { 1 };
    if columns == 2 {
        let spacing = ui.spacing().item_spacing.x;
        let left = (width - spacing) * 0.44;
        let right = width - spacing - left;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(left, 0.0),
                Layout::top_down(Align::Min),
                |ui| rate_card(ui, done, total, days),
            );
            ui.allocate_ui_with_layout(
                egui::vec2(right, 0.0),
                Layout::top_down(Align::Min),
                |ui| chart_card(ui, &series),
            );
        });
    } else {
        rate_card(ui, done, total, days);
        ui.add_space(12.0);
        chart_card(ui, &series);
    }

    ui.add_space(12.0);
    heatmap_card(app, ui, today);
    ui.add_space(12.0);
    overview_card(app, ui, today);
}

fn header(ui: &mut egui::Ui) -> u32 {
    let range_id = egui::Id::new(RANGE_ID);
    let mut days = ui.data(|data| data.get_temp::<u32>(range_id)).unwrap_or(30);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(title("统计", 26.0));
                pill(ui, &format!("近 {days} 天"), accent());
            });
            ui.label(muted("完成率、每日完成数与打卡热力图"));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            for option in [7u32, 14, 30] {
                let selected = days == option;
                let text = RichText::new(format!("{option} 天"))
                    .size(13.0)
                    .color(if selected { text() } else { text_dim() });
                let button = egui::Button::new(text)
                    .fill(if selected {
                        accent_soft()
                    } else {
                        Color32::TRANSPARENT
                    })
                    .stroke(egui::Stroke::new(
                        1.0,
                        if selected { accent() } else { line() },
                    ));
                if ui.add(button).clicked() {
                    days = option;
                }
            }
        });
    });
    ui.data_mut(|data| data.insert_temp(range_id, days));
    days
}

fn rate_card(ui: &mut egui::Ui, done: u32, total: u32, days: u32) {
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section(ui, "完成率");
        if total == 0 {
            empty_state(ui, "还没有待办数据", "记录并完成一些待办后，这里会出现完成率");
            return;
        }
        let ratio = done as f32 / total as f32;
        ui.horizontal(|ui| {
            ui.label(metric(
                format!("{}%", (ratio * 100.0).round() as i64),
                38.0,
            ));
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.add_space(6.0);
                ui.label(muted(format!("近 {days} 天")));
                ui.label(muted(format!("完成 {done} / {total}")));
            });
        });
        ui.add_space(10.0);
        progress(ui, ratio, accent());
        ui.add_space(6.0);
        ui.label(muted(format!(
            "平均每天 {:.1} 条",
            done as f32 / days.max(1) as f32
        )));
    });
}

fn chart_card(ui: &mut egui::Ui, series: &[(String, DayStat)]) {
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            section(ui, "完成数");
            if series.iter().any(|(_, stat)| stat.total > 0) {
                let max = series
                    .iter()
                    .map(|(_, stat)| stat.done)
                    .max()
                    .unwrap_or(0)
                    .max(1);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(muted(format!("峰值 {max} 条")));
                });
            }
        });
        if !series.iter().any(|(_, stat)| stat.total > 0) {
            empty_state(ui, "还没有完成记录", "每天完成的待办会在这里排成柱状图");
            return;
        }
        let max = series
            .iter()
            .map(|(_, stat)| stat.done)
            .max()
            .unwrap_or(0)
            .max(1);
        let width = ui.available_width();
        let chart_h = 104.0;
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(width, chart_h + 22.0), egui::Sense::hover());
        let painter = ui.painter();
        let base_y = rect.top() + chart_h;
        painter.hline(
            rect.x_range(),
            base_y,
            egui::Stroke::new(1.0, line_soft()),
        );

        let count = series.len().max(1);
        let slot = rect.width() / count as f32;
        let bar_w = (slot * 0.52).clamp(3.0, 16.0);
        for (index, (date, stat)) in series.iter().enumerate() {
            let center_x = rect.left() + slot * (index as f32 + 0.5);
            let height = chart_h * (stat.done as f32 / max as f32);
            if height > 0.0 {
                let filled = egui::Rect::from_min_size(
                    egui::pos2(center_x - bar_w / 2.0, base_y - height),
                    egui::vec2(bar_w, height),
                );
                let color = if stat.total > 0 && stat.done == stat.total {
                    ok()
                } else {
                    accent()
                };
                painter.rect_filled(filled, egui::CornerRadius::same(3), color);
            } else {
                let nub = egui::Rect::from_min_size(
                    egui::pos2(center_x - bar_w / 2.0, base_y - 2.0),
                    egui::vec2(bar_w, 2.0),
                );
                painter.rect_filled(
                    nub,
                    egui::CornerRadius::same(1),
                    Color32::from_rgba_premultiplied(255, 255, 255, 18),
                );
            }
            if index % 3 == 0 {
                let label = date.get(5..).unwrap_or(date.as_str());
                painter.text(
                    egui::pos2(center_x, base_y + 11.0),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(9.5),
                    text_muted(),
                );
            }
        }
    });
}

fn heatmap_card(app: &App, ui: &mut egui::Ui, today: NaiveDate) {
    let checkins = &app.store.checkins;
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        let total = checkins.iter().filter(|item| !item.deleted).count();
        ui.horizontal(|ui| {
            section(ui, "全部打卡热力图");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(muted(format!("累计 {total} 次打卡")));
            });
        });
        if total == 0 {
            empty_state(ui, "还没有打卡记录", "给习惯打卡后，这里会积累成一张热力图");
            return;
        }
        ui.add_space(6.0);
        let width = ui.available_width();
        let weeks = (((width + GAP) / (CELL + GAP)).floor() as usize).clamp(4, 26);
        draw_heatmap(ui, checkins, today, weeks);
        ui.add_space(8.0);
        legend(ui);
    });
}

fn draw_heatmap(ui: &mut egui::Ui, checkins: &[HabitCheckin], today: NaiveDate, weeks: usize) {
    let mut counts: HashMap<NaiveDate, u32> = HashMap::new();
    for item in checkins.iter().filter(|item| !item.deleted) {
        if let Ok(date) = NaiveDate::parse_from_str(&item.date, "%Y-%m-%d") {
            *counts.entry(date).or_insert(0) += 1;
        }
    }

    let (_, week_end) = totion_core::week_range(today);
    let start = week_end - ChronoDuration::days(weeks as i64 * 7 - 1);
    let size = egui::vec2(
        weeks as f32 * CELL + (weeks.saturating_sub(1)) as f32 * GAP,
        7.0 * CELL + 6.0 * GAP,
    );
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter();
    for week in 0..weeks {
        for day in 0..7 {
            let date = start + ChronoDuration::days((week * 7 + day) as i64);
            if date > today {
                continue;
            }
            let count = counts.get(&date).copied().unwrap_or(0);
            let color = match count {
                0 => Color32::from_rgba_premultiplied(255, 255, 255, 14),
                1..=2 => Color32::from_rgba_unmultiplied(94, 170, 198, 110),
                _ => accent(),
            };
            let min = egui::pos2(
                rect.left() + week as f32 * (CELL + GAP),
                rect.top() + day as f32 * (CELL + GAP),
            );
            painter.rect_filled(
                egui::Rect::from_min_size(min, egui::vec2(CELL, CELL)),
                egui::CornerRadius::same(2),
                color,
            );
        }
    }
}

fn legend(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(muted("少"));
        for (color, label) in [
            (Color32::from_rgba_premultiplied(255, 255, 255, 14), "0"),
            (Color32::from_rgba_unmultiplied(94, 170, 198, 110), "1 到 2"),
            (accent(), "3 以上"),
        ] {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(2), color);
            ui.label(muted(label));
        }
        ui.label(muted("多"));
    });
}

fn overview_card(app: &mut App, ui: &mut egui::Ui, today: NaiveDate) {
    let habits = app.store.active_habits();
    let mut jump: Option<String> = None;

    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            section(ui, "习惯概览");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(muted("点击查看习惯"));
            });
        });
        ui.add_space(4.0);
        if habits.is_empty() {
            empty_state(ui, "还没有习惯", "先在「习惯」里新建一个");
            return;
        }

        let width = ui.available_width();
        let (header_rect, _) =
            ui.allocate_exact_size(egui::vec2(width, 20.0), egui::Sense::hover());
        paint_row(
            ui.painter(),
            header_rect,
            "名称",
            "连续",
            "最长",
            "30 天",
            true,
        );
        hairline(ui);

        for habit in &habits {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, 32.0), egui::Sense::click());
            if response.hovered() {
                ui.painter()
                    .rect_filled(rect, egui::CornerRadius::same(6), card_hi());
            } else if app.selected_habit.as_deref() == Some(habit.id.as_str()) {
                ui.painter()
                    .rect_filled(rect, egui::CornerRadius::same(6), accent_soft());
            }
            let stats = app.store.habit_stats(habit, today);
            paint_row(
                ui.painter(),
                rect,
                &display_name(habit),
                &stats.current_streak.to_string(),
                &stats.longest_streak.to_string(),
                &format!("{}%", (stats.last_30_rate * 100.0).round() as i64),
                false,
            );
            if response.clicked() {
                jump = Some(habit.id.clone());
            }
        }
    });

    if let Some(id) = jump {
        app.selected_habit = Some(id);
        app.view = View::Habits;
    }
}

fn paint_row(
    painter: &egui::Painter,
    rect: egui::Rect,
    name: &str,
    current: &str,
    longest: &str,
    rate: &str,
    header: bool,
) {
    let pad = 10.0;
    let font = egui::FontId::proportional(if header { 10.5 } else { 13.0 });
    let color = if header { text_muted() } else { text() };
    let dim = if header { text_muted() } else { text_dim() };
    let y = rect.center().y;
    painter.text(
        egui::pos2(rect.left() + pad, y),
        egui::Align2::LEFT_CENTER,
        name,
        font.clone(),
        color,
    );
    painter.text(
        egui::pos2(rect.left() + rect.width() * 0.62, y),
        egui::Align2::RIGHT_CENTER,
        current,
        font.clone(),
        dim,
    );
    painter.text(
        egui::pos2(rect.left() + rect.width() * 0.80, y),
        egui::Align2::RIGHT_CENTER,
        longest,
        font.clone(),
        dim,
    );
    painter.text(
        egui::pos2(rect.right() - pad, y),
        egui::Align2::RIGHT_CENTER,
        rate,
        font,
        dim,
    );
}

fn display_name(habit: &totion_core::Habit) -> String {
    let icon = habit.icon.trim();
    if icon.is_empty() {
        habit.title.clone()
    } else {
        format!("{} {}", icon, habit.title)
    }
}
