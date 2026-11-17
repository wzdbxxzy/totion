//! 日历：月视图网格 + 选中日议程 + 当日总结。
//!
//! 一屏内先看整月待办的疏密，再在右侧处理某一天的具体安排。

use chrono::{Datelike, Duration as ChronoDuration, Local, Months, NaiveDate};
use eframe::egui::{self, Align, Align2, Color32, CornerRadius, FontId, Layout, RichText, Sense, Stroke, Vec2};
use totion_core::JournalTarget;

use crate::app::{weekday_cn, App};
use crate::net::Command;
use crate::theme::*;
use crate::views::today::empty_state;

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    header(app, ui);
    ui.add_space(12.0);

    let width = ui.available_width();
    if width >= 1100.0 {
        let avail_h = ui.available_height();
        let sidebar_w = 260.0;
        let grid_w = (width - sidebar_w - 12.0).max(320.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(grid_w, avail_h),
                Layout::top_down(Align::Min),
                |ui| month_grid(app, ui, None),
            );
            ui.add_space(4.0);
            ui.allocate_ui_with_layout(
                Vec2::new(sidebar_w, avail_h),
                Layout::top_down(Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| day_panel(app, ui));
                },
            );
        });
    } else {
        let gap = 5.0;
        let cell_w = ((width - gap * 6.0) / 7.0).max(20.0);
        let cell_h = (cell_w * 0.72).clamp(54.0, 92.0);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                month_grid(app, ui, Some(cell_h));
                ui.add_space(12.0);
                day_panel(app, ui);
            });
    }
}

// ---------------------------------------------------------------------------
// 顶部：换月、今天、月份标题、本月统计
// ---------------------------------------------------------------------------

fn header(app: &mut App, ui: &mut egui::Ui) {
    let today = Local::now().date_naive();
    ui.horizontal(|ui| {
        if ui.button("‹").clicked() {
            app.date = shift_month(app.date, -1);
        }
        if ui.button("›").clicked() {
            app.date = shift_month(app.date, 1);
        }
        ui.add_space(4.0);
        if ui.button("今天").clicked() {
            app.date = today;
        }
        ui.add_space(10.0);
        ui.label(title(
            format!("{} 年 {} 月", app.date.year(), app.date.month()),
            24.0,
        ));
        if !app.is_writable() {
            pill(ui, "只读视角", warn());
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let (done, total) = month_completion(app, app.date.year(), app.date.month());
            ui.label(muted(format!("待办 {} · 完成 {}", total, done)));
            if total > 0 {
                ui.label(metric(format!("{}%", done * 100 / total), 16.0));
            }
        });
    });
}

fn month_completion(app: &App, year: i32, month: u32) -> (usize, usize) {
    let days = days_in_month(year, month);
    let mut done = 0;
    let mut total = 0;
    for day in 1..=days {
        if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
            let key = date.format("%Y-%m-%d").to_string();
            let (d, t) = app.store.day_completion(&key);
            done += d;
            total += t;
        }
    }
    (done, total)
}

// ---------------------------------------------------------------------------
// 月历网格
// ---------------------------------------------------------------------------

fn month_grid(app: &mut App, ui: &mut egui::Ui, fixed_cell_h: Option<f32>) {
    let gap = 5.0;
    let cell_w = ((ui.available_width() - gap * 6.0) / 7.0).max(20.0);
    let header_h = 18.0;
    let cell_h = fixed_cell_h.unwrap_or_else(|| {
        ((ui.available_height() - header_h - gap * 6.0) / 6.0).max(64.0)
    });

    ui.spacing_mut().item_spacing = Vec2::new(gap, gap);

    let first = NaiveDate::from_ymd_opt(app.date.year(), app.date.month(), 1)
        .unwrap_or_else(|| Local::now().date_naive());
    let offset = first.weekday().num_days_from_monday() as i64;
    let start = first - ChronoDuration::days(offset);
    let today = Local::now().date_naive();
    let shown_month = (app.date.year(), app.date.month());

    // 周一到周日表头
    const WEEKDAYS: [&str; 7] = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
    ui.horizontal(|ui| {
        for name in WEEKDAYS {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(cell_w, header_h), Sense::hover());
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                name,
                FontId::proportional(11.0),
                text_muted(),
            );
        }
    });

    for row in 0..6 {
        ui.horizontal(|ui| {
            for col in 0..7 {
                let date = start + ChronoDuration::days(row * 7 + col);
                let in_month = (date.year(), date.month()) == shown_month;
                day_cell(app, ui, date, today, in_month, cell_w, cell_h);
            }
        });
    }
}

fn day_cell(
    app: &mut App,
    ui: &mut egui::Ui,
    date: NaiveDate,
    today: NaiveDate,
    in_month: bool,
    w: f32,
    h: f32,
) {
    let date_key = date.format("%Y-%m-%d").to_string();
    let is_today = date == today;
    let selected = date == app.date;
    let items = app.store.day_blocks(&date_key);
    let has_summary = app.store.daily_summary(&date_key).is_some();

    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let hovered = response.hovered();
    let radius = CornerRadius::same(9);

    let base = if !in_month {
        field()
    } else if hovered || selected {
        card_hi()
    } else {
        card()
    };
    let fill = if is_today { tint(base, accent(), 0.18) } else { base };

    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, radius, accent());
        painter.rect_filled(rect.shrink(1.6), radius, fill);
    } else {
        painter.rect_filled(rect, radius, fill);
    }

    // 日期数字
    let num_color = if !in_month {
        text_muted()
    } else if is_today {
        accent()
    } else if selected {
        text()
    } else {
        text_dim()
    };
    painter.text(
        rect.left_top() + Vec2::new(7.0, 5.0),
        Align2::LEFT_TOP,
        date.day().to_string(),
        FontId::new(12.0, bold_family()),
        num_color,
    );

    // 当日事项用小圆点表示：实心＝已完成，空心＝未完成。
    let total = items.len();
    let done = items.iter().filter(|block| block.checked).count();
    if total > 0 {
        let dot_r = 2.4;
        let step = 7.5;
        let left = rect.left() + 8.0;
        let max_per_row = (((rect.width() - 16.0) / step).floor() as usize).max(1);
        let rows = ((total + max_per_row - 1) / max_per_row).min(2);
        for index in 0..total.min(max_per_row * rows) {
            let row = index / max_per_row;
            let col = index % max_per_row;
            let center = egui::pos2(
                left + col as f32 * step,
                rect.top() + 26.0 + row as f32 * 9.0,
            );
            let filled = index < done;
            let color = if filled { ok() } else { text_muted() };
            if filled {
                painter.circle_filled(center, dot_r, color);
            } else {
                painter.circle_stroke(center, dot_r, Stroke::new(1.0_f32, color));
            }
        }
        if total > max_per_row * rows {
            painter.text(
                egui::pos2(left + max_per_row as f32 * step, rect.top() + 24.0),
                Align2::LEFT_TOP,
                format!("+{}", total - max_per_row * rows),
                FontId::proportional(9.0),
                text_muted(),
            );
        }
    }

    // 右下角：当天有总结的小圆点
    if has_summary {
        painter.circle_filled(
            egui::pos2(rect.right() - 8.0, rect.bottom() - 8.0),
            2.5,
            accent(),
        );
    }

    if response.clicked() {
        app.date = date;
    }
}

// ---------------------------------------------------------------------------
// 右侧（或底部）：选中日议程 + 新建待办 + 当日总结
// ---------------------------------------------------------------------------

fn day_panel(app: &mut App, ui: &mut egui::Ui) {
    let date = app.date;
    let date_key = date.format("%Y-%m-%d").to_string();
    let items = app.store.day_blocks(&date_key);
    let total = items.len();
    let today = Local::now().date_naive();
    let writable = app.is_writable();

    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(title(format!("{} 月 {} 日", date.month(), date.day()), 20.0));
            pill(ui, weekday_cn(date), accent());
            if date == today {
                pill(ui, "今天", ok());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(muted(format!("{} 条", total)));
            });
        });
        ui.add_space(6.0);
        hairline(ui);
        ui.add_space(6.0);

        if items.is_empty() {
            empty_state(ui, "这一天还没有安排", "在下面加一条待办");
        } else {
            let mut toggle: Option<(String, i64, bool)> = None;
            for block in &items {
                ui.horizontal(|ui| {
                    let time = block.start_time.clone().unwrap_or_else(|| "—".to_string());
                    ui.add_sized([38.0, 18.0], egui::Label::new(muted(time)));

                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::new(18.0, 18.0), Sense::click());
                    let stroke = if block.checked {
                        Stroke::new(1.5_f32, ok())
                    } else {
                        Stroke::new(1.2_f32, text_muted())
                    };
                    ui.painter().circle_stroke(rect.center(), 7.5, stroke);
                    if block.checked {
                        ui.painter().circle_filled(rect.center(), 4.5, ok());
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
                    ui.add(egui::Label::new(text).truncate());
                });
                ui.add_space(2.0);
            }
            if let Some((id, version, checked)) = toggle {
                if writable {
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
        }

        ui.add_space(8.0);
        hairline(ui);
        ui.add_space(8.0);
        new_todo_input(app, ui, &date_key, writable);
    });

    ui.add_space(12.0);
    summary_card(app, ui, &date_key, writable);
}

fn new_todo_input(app: &mut App, ui: &mut egui::Ui, date_key: &str, writable: bool) {
    let open_id = egui::Id::new(("cal_new_open", date_key));
    let text_id = egui::Id::new(("cal_new_text", date_key));
    let focus_id = egui::Id::new(("cal_new_focus", date_key));

    let mut open = ui.memory(|m| m.data.get_temp::<bool>(open_id).unwrap_or(false));

    if !open {
        let button = egui::Button::new(RichText::new("＋ 新建待办").color(text()))
            .fill(accent_soft())
            .min_size(Vec2::new(ui.available_width(), 32.0));
        if ui.add(button).clicked() {
            if writable {
                ui.memory_mut(|m| {
                    m.data.insert_temp(open_id, true);
                    m.data.insert_temp(focus_id, true);
                });
            } else {
                app.toast("他人视角只读，可留言", true);
            }
        }
        return;
    }

    let mut draft = ui
        .memory(|m| m.data.get_temp::<String>(text_id).unwrap_or_default());
    let response = ui.add(
        egui::TextEdit::singleline(&mut draft)
            .id(text_id)
            .hint_text("新的待办，回车添加")
            .desired_width(f32::INFINITY),
    );
    if ui
        .memory(|m| m.data.get_temp::<bool>(focus_id).unwrap_or(false))
    {
        response.request_focus();
        ui.memory_mut(|m| m.data.insert_temp(focus_id, false));
    }

    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
    let mut submit = enter && (response.has_focus() || response.lost_focus());
    let mut cancel = false;
    ui.horizontal(|ui| {
        if ui.button("添加").clicked() {
            submit = true;
        }
        if ui.button("取消").clicked() {
            cancel = true;
        }
    });

    if cancel {
        open = false;
        draft.clear();
    } else if submit {
        let text = draft.trim().to_string();
        if text.is_empty() {
            // 空内容不提交，保留输入框
        } else if writable {
            app.send(Command::CreateBlockOnDate {
                date: date_key.to_string(),
                text,
                start_time: None,
            });
            draft.clear();
            open = false;
        } else {
            app.toast("他人视角只读，可留言", true);
        }
    }

    ui.memory_mut(|m| {
        m.data.insert_temp(text_id, draft.clone());
        m.data.insert_temp(open_id, open);
    });
}

fn summary_card(app: &mut App, ui: &mut egui::Ui, date_key: &str, writable: bool) {
    let open_id = egui::Id::new(("cal_sum_open", date_key));
    let text_id = egui::Id::new(("cal_sum_text", date_key));

    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section(ui, "当天总结");
        ui.add_space(2.0);

        if let Some(journal) = app.store.daily_summary(date_key) {
            ui.label(body(journal.body.clone()));
            ui.add_space(6.0);
            ui.label(muted(format!(
                "{} {} · {}",
                journal.date, journal.time, journal.author_name
            )));
            return;
        }

        let mut open = ui.memory(|m| m.data.get_temp::<bool>(open_id).unwrap_or(false));
        if !open {
            let button = egui::Button::new(RichText::new("＋ 写总结").color(text_dim()))
                .frame(false);
            if ui.add(button).clicked() {
                if writable {
                    ui.memory_mut(|m| {
                        m.data.insert_temp(open_id, true);
                        m.data.insert_temp(
                            egui::Id::new(("cal_sum_focus", date_key)),
                            true,
                        );
                    });
                } else {
                    app.toast("他人视角只读，可留言", true);
                }
            }
            return;
        }

        let mut draft = ui
            .memory(|m| m.data.get_temp::<String>(text_id).unwrap_or_default());
        let response = ui.add(
            egui::TextEdit::multiline(&mut draft)
                .id(text_id)
                .hint_text("今天怎么样？一句话也算")
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        if ui
            .memory(|m| m.data.get_temp::<bool>(egui::Id::new(("cal_sum_focus", date_key))).unwrap_or(false))
        {
            response.request_focus();
            ui.memory_mut(|m| {
                m.data
                    .insert_temp(egui::Id::new(("cal_sum_focus", date_key)), false)
            });
        }

        ui.horizontal(|ui| {
            if ui.button("记录").clicked() {
                let text = draft.trim().to_string();
                if text.is_empty() {
                    // 空内容不提交
                } else if writable {
                    app.send(Command::CreateJournal {
                        target_kind: JournalTarget::Daily,
                        target_id: date_key.to_string(),
                        body: text,
                    });
                    draft.clear();
                    open = false;
                } else {
                    app.toast("他人视角只读，可留言", true);
                }
            }
            if ui.button("取消").clicked() {
                draft.clear();
                open = false;
            }
        });

        ui.memory_mut(|m| {
            m.data.insert_temp(text_id, draft.clone());
            m.data.insert_temp(open_id, open);
        });
    });
}

// ---------------------------------------------------------------------------
// 小工具
// ---------------------------------------------------------------------------

fn days_in_month(year: i32, month: u32) -> u32 {
    let first = match NaiveDate::from_ymd_opt(year, month, 1) {
        Some(date) => date,
        None => return 30,
    };
    let next = first.checked_add_months(Months::new(1)).unwrap_or(first);
    (next - first).num_days().max(28) as u32
}

fn shift_month(date: NaiveDate, delta: i32) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap_or(date);
    let shifted = if delta >= 0 {
        first.checked_add_months(Months::new(delta.max(0) as u32))
    } else {
        first.checked_sub_months(Months::new((-delta) as u32))
    }
    .unwrap_or(first);
    let day = date.day().min(days_in_month(shifted.year(), shifted.month()));
    NaiveDate::from_ymd_opt(shifted.year(), shifted.month(), day).unwrap_or(shifted)
}

/// 按目标宽度估算截断（中文按整字宽，西文约 0.55 字宽），超出加省略号。
#[allow(dead_code)]
fn fit_text(text: &str, max_width: f32, size: f32) -> String {
    if max_width <= size {
        return String::new();
    }
    let max_width = max_width.max(0.0);
    let mut out = String::new();
    let mut width = 0.0;
    for ch in text.chars() {
        let char_w = if ch.is_ascii() { size * 0.55 } else { size };
        if width + char_w > max_width {
            out.push('…');
            return out;
        }
        width += char_w;
        out.push(ch);
    }
    out
}

fn tint(base: Color32, top: Color32, alpha: f32) -> Color32 {
    let a = alpha.clamp(0.0, 1.0);
    let mix = |b: u8, t: u8| ((b as f32) * (1.0 - a) + (t as f32) * a).round() as u8;
    Color32::from_rgb(
        mix(base.r(), top.r()),
        mix(base.g(), top.g()),
        mix(base.b(), top.b()),
    )
}
