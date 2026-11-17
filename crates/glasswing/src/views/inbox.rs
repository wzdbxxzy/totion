//! 留言：所有对象（页面 / 内容块 / 习惯）的评论，支持筛选、解决、删除、回复与跳转。

use eframe::egui::{self, Align, Color32, Layout, RichText};
use totion_core::{BlockKind, Comment, CommentTargetKind};

use crate::app::{App, View};
use crate::net::Command;
use crate::theme::*;
use crate::views::today::{empty_state, open_comments, rel_time};

const FILTER_ID: &str = "glasswing_inbox_show_all";

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    let filter_id = egui::Id::new(FILTER_ID);
    let mut show_all = ui.data(|data| data.get_temp::<bool>(filter_id).unwrap_or(false));
    let unresolved = app
        .store
        .comments
        .iter()
        .filter(|comment| !comment.deleted && !comment.resolved)
        .count();

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(title("留言", 26.0));
                if unresolved > 0 {
                    pill(ui, &format!("{unresolved} 条未解决"), text_dim());
                }
            });
            ui.label(muted("按时间倒序 · 点「跳到对象」回到原处"));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.selectable_label(show_all, "全部").clicked() {
                show_all = true;
            }
            if ui.selectable_label(!show_all, "未解决").clicked() {
                show_all = false;
            }
        });
    });
    ui.data_mut(|data| data.insert_temp(filter_id, show_all));
    ui.add_space(14.0);

    let mut comments: Vec<Comment> = app
        .store
        .comments
        .iter()
        .filter(|comment| !comment.deleted && (show_all || !comment.resolved))
        .cloned()
        .collect();
    comments.sort_by_key(|comment| std::cmp::Reverse(comment.created_at));

    if comments.is_empty() {
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            if show_all {
                empty_state(ui, "还没有留言", "在页面、内容块或习惯下留一句");
            } else {
                empty_state(ui, "没有未解决的留言", "干净得像清晨的窗");
            }
        });
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt("inbox_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            for comment in &comments {
                comment_card(app, ui, comment);
                ui.add_space(10.0);
            }
        });
}

fn comment_card(app: &mut App, ui: &mut egui::Ui, comment: &Comment) {
    let info = target_info(app, comment);
    let mut resolve = false;
    let mut delete = false;
    let mut reply = false;
    let mut jump = false;

    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(dim(comment.author_name.clone()));
            ui.label(muted(rel_time(comment.created_at)));
            if comment.resolved {
                pill(ui, "已解决", text_muted());
            } else {
                pill(ui, "未解决", text_dim());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if action_button(ui, "回复", text_dim()) {
                    reply = true;
                }
                if action_button(ui, "删除", text_muted()) {
                    delete = true;
                }
                let label = if comment.resolved { "取消解决" } else { "解决" };
                if action_button(ui, label, text_dim()) {
                    resolve = true;
                }
            });
        });

        ui.add_space(6.0);
        ui.add(egui::Label::new(body(comment.body.clone())).wrap());
        ui.add_space(8.0);

        let jump_button = egui::Button::new(
            RichText::new(format!("跳到对象 · {}", info.label))
                .size(12.0)
                .color(text_dim()),
        )
        .small();
        if ui.add(jump_button).clicked() {
            jump = true;
        }
    });

    if resolve {
        app.send(Command::UpdateComment {
            id: comment.id.clone(),
            version: comment.version,
            body: None,
            resolved: Some(!comment.resolved),
            deleted: false,
        });
    }
    if delete {
        app.send(Command::UpdateComment {
            id: comment.id.clone(),
            version: comment.version,
            body: None,
            resolved: None,
            deleted: true,
        });
    }
    if reply {
        open_comments(app, comment.target_kind, &comment.target_id, &info.label);
    }
    if jump {
        if let Some(page_id) = info.page {
            jump_to_page(app, &page_id);
        } else if let Some(habit_id) = info.habit {
            app.selected_habit = Some(habit_id);
            app.view = View::Habits;
        } else {
            app.toast("对象已删除", true);
        }
    }
}

// ---------------------------------------------------------------------------
// 对象定位
// ---------------------------------------------------------------------------

struct TargetInfo {
    label: String,
    page: Option<String>,
    habit: Option<String>,
}

fn target_info(app: &App, comment: &Comment) -> TargetInfo {
    match comment.target_kind {
        CommentTargetKind::Page => {
            match app
                .store
                .pages
                .iter()
                .find(|page| page.id == comment.target_id && !page.deleted)
            {
                Some(page) => TargetInfo {
                    label: format!("页面「{}」", ellipsize(&page.title, 30)),
                    page: Some(page.id.clone()),
                    habit: None,
                },
                None => TargetInfo {
                    label: "页面 · 已删除".to_string(),
                    page: None,
                    habit: None,
                },
            }
        }
        CommentTargetKind::Block => {
            match app
                .store
                .blocks
                .iter()
                .find(|block| block.id == comment.target_id && !block.deleted)
            {
                Some(block) => {
                    let kind = if block.kind == BlockKind::Todo {
                        "待办"
                    } else {
                        "内容"
                    };
                    TargetInfo {
                        label: format!("{}「{}」", kind, first_line(&block.text, 30)),
                        page: Some(block.page_id.clone()),
                        habit: None,
                    }
                }
                None => TargetInfo {
                    label: "内容块 · 已删除".to_string(),
                    page: None,
                    habit: None,
                },
            }
        }
        CommentTargetKind::Habit => {
            match app
                .store
                .habits
                .iter()
                .find(|habit| habit.id == comment.target_id && !habit.deleted)
            {
                Some(habit) => TargetInfo {
                    label: format!("习惯「{}」", ellipsize(&habit.title, 30)),
                    page: None,
                    habit: Some(habit.id.clone()),
                },
                None => TargetInfo {
                    label: "习惯 · 已删除".to_string(),
                    page: None,
                    habit: None,
                },
            }
        }
    }
}

/// 普通页面回工作区；日报页回到「今日」的对应日期。
fn jump_to_page(app: &mut App, page_id: &str) {
    let daily = app
        .store
        .pages
        .iter()
        .find(|page| page.id == page_id && !page.deleted)
        .and_then(|page| page.date.clone());
    if let Some(date) = daily {
        if let Ok(parsed) = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d") {
            app.date = parsed;
            app.view = View::Today;
            return;
        }
    }
    app.selected_page = Some(page_id.to_string());
    app.view = View::Workspace;
}

// ---------------------------------------------------------------------------
// 小工具
// ---------------------------------------------------------------------------

fn action_button(ui: &mut egui::Ui, text: &str, color: Color32) -> bool {
    ghost(ui, text, color).clicked()
}

fn first_line(text: &str, max: usize) -> String {
    let line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .trim();
    ellipsize(line, max)
}

fn ellipsize(text: &str, max: usize) -> String {
    let mut out = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index >= max {
            out.push('⋯');
            break;
        }
        out.push(ch);
    }
    out
}
