//! 弹层：编辑框、评论、成员、邀请码、搜索、设置。

use eframe::egui::{self, Align, Color32, Layout, RichText};
use totion_core::{CommentTargetKind, JournalTarget};

use crate::app::{App, Dialog, Editor};
use crate::net::Command;
use crate::theme::*;
use crate::views::today::rel_time;

pub fn draw(app: &mut App, ctx: &egui::Context) {
    editor_window(app, ctx);
    comments_window(app, ctx);
    members_window(app, ctx);
    invites_window(app, ctx);
    search_window(app, ctx);
    settings_window(app, ctx);
}

fn window_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgb(252, 250, 246))
        .stroke(egui::Stroke::new(1.0, LINE))
        .corner_radius(egui::CornerRadius::same(16))
        .inner_margin(egui::Margin::same(20))
        .shadow(egui::epaint::Shadow {
            offset: [0, 10],
            blur: 40,
            spread: 0,
            color: Color32::from_black_alpha(160),
        })
}

fn editor_window(app: &mut App, ctx: &egui::Context) {
    let mut open = !matches!(app.editor, Editor::None);
    if !open {
        return;
    }
    let mut close = false;
    let mut save: Option<Command> = None;
    let mut title_text = String::new();
    match &app.editor {
        Editor::Block { block, .. } => {
            title_text = if block.is_some() {
                "编辑待办".into()
            } else {
                "新建待办".into()
            }
        }
        Editor::Journal { label, .. } => title_text = format!("总结 · {label}"),
        Editor::Habit { .. } => title_text = "新建习惯".into(),
        Editor::Page { page, .. } => {
            title_text = if page.is_some() { "重命名页面".into() } else { "新建页面".into() }
        }
        Editor::None => {}
    }
    let mut window = egui::Window::new(RichText::new(title_text).size(16.0))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO);
    window = window.frame(window_frame());
    window.show(ctx, |ui| {
        ui.set_width(440.0);
        match &mut app.editor {
            Editor::None => {}
            Editor::Block {
                block,
                text,
                time,
                date,
            } => {
                if let Some(existing) = block {
                    text.clear();
                    ui.label(dim(existing.text.clone()));
                    ui.add_space(6.0);
                }
                section(ui, "内容");
                ui.add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    section(ui, "时间");
                    ui.add(
                        egui::TextEdit::singleline(time)
                            .hint_text("HH:MM")
                            .desired_width(90.0),
                    );
                    ui.label(muted(date.clone()));
                });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui.button("保存").clicked() {
                        let trimmed = text.trim().to_string();
                        if !trimmed.is_empty() {
                            save = Some(match block {
                                Some(existing) => Command::UpdateBlock {
                                    id: existing.id.clone(),
                                    version: existing.version,
                                    text: Some(trimmed),
                                    checked: None,
                                    deleted: false,
                                },
                                None => Command::CreateBlockOnDate {
                                    date: date.clone(),
                                    text: trimmed,
                                    start_time: if time.trim().is_empty() {
                                        None
                                    } else {
                                        Some(time.trim().to_string())
                                    },
                                },
                            });
                            close = true;
                        }
                    }
                    if ui.button("取消").clicked() {
                        close = true;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(existing) = block {
                            if ui
                                .add(egui::Button::new(
                                    RichText::new("删除").color(DANGER),
                                ))
                                .clicked()
                            {
                                save = Some(Command::UpdateBlock {
                                    id: existing.id.clone(),
                                    version: existing.version,
                                    text: None,
                                    checked: None,
                                    deleted: true,
                                });
                                close = true;
                            }
                        }
                    });
                });
            }
            Editor::Journal {
                existing,
                text,
                target_kind,
                target_id,
                ..
            } => {
                ui.add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(6)
                        .hint_text("今天发生了什么？")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("保存").clicked() {
                        let trimmed = text.trim().to_string();
                        if !trimmed.is_empty() {
                            save = Some(match existing {
                                Some((_, version)) => Command::UpdateJournal {
                                    id: app
                                        .store
                                        .daily_summary(&app.store.today())
                                        .map(|j| j.id)
                                        .unwrap_or_default(),
                                    version: *version,
                                    body: Some(trimmed),
                                    deleted: false,
                                },
                                None => Command::CreateJournal {
                                    target_kind: *target_kind,
                                    target_id: target_id.clone(),
                                    body: trimmed,
                                },
                            });
                            close = true;
                        }
                    }
                    if ui.button("取消").clicked() {
                        close = true;
                    }
                });
            }
            Editor::Page { page, title } => {
                section(ui, "页面标题");
                ui.add(
                    egui::TextEdit::singleline(title)
                        .hint_text("例如：灵感收集")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("保存").clicked() {
                        let trimmed = title.trim().to_string();
                        if !trimmed.is_empty() {
                            save = Some(match page {
                                Some(existing) => Command::UpdatePage {
                                    id: existing.id.clone(),
                                    version: existing.version,
                                    title: Some(trimmed),
                                    deleted: false,
                                },
                                None => Command::CreatePage { title: trimmed },
                            });
                            close = true;
                        }
                    }
                    if ui.button("取消").clicked() {
                        close = true;
                    }
                    if let Some(existing) = page {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add(egui::Button::new(RichText::new("删除页面").color(DANGER)))
                                .clicked()
                            {
                                save = Some(Command::UpdatePage {
                                    id: existing.id.clone(),
                                    version: existing.version,
                                    title: None,
                                    deleted: true,
                                });
                                close = true;
                            }
                        });
                    }
                });
            }
            Editor::Habit { title, icon, weekly } => {
                section(ui, "名称");
                ui.add(
                    egui::TextEdit::singleline(title)
                        .hint_text("例如：读书 30 分钟")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    section(ui, "图标");
                    ui.add(egui::TextEdit::singleline(icon).desired_width(60.0));
                });
                ui.add_space(8.0);
                ui.checkbox(weekly, "每周几次（不勾选＝每天）");
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("创建").clicked() {
                        let trimmed = title.trim().to_string();
                        if !trimmed.is_empty() {
                            save = Some(Command::CreateHabit {
                                title: trimmed,
                                icon: icon.clone(),
                                frequency: if *weekly {
                                    totion_core::HabitFrequency::Weekly { times: 3 }
                                } else {
                                    totion_core::HabitFrequency::Daily
                                },
                            });
                            close = true;
                        }
                    }
                    if ui.button("取消").clicked() {
                        close = true;
                    }
                });
            }
        }
    });
    if let Some(command) = save {
        app.send(command);
    }
    if close || !open {
        app.editor = Editor::None;
    }
}

fn comments_window(app: &mut App, ctx: &egui::Context) {
    let Dialog::Comments {
        kind,
        id,
        label,
        draft,
    } = &mut app.dialog
    else {
        return;
    };
    let kind = *kind;
    let target = id.clone();
    let title = label.clone();
    let mut close = false;
    let mut send: Option<Command> = None;
    egui::Window::new(RichText::new(format!("留言 · {title}")).size(15.0))
        .default_width(420.0)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::RIGHT_CENTER, egui::Vec2::new(-40.0, 0.0))
        .frame(window_frame())
        .show(ctx, |ui| {
            let comments = app.store.comments_for(kind, &target);
            if comments.is_empty() {
                ui.label(muted("还没有留言"));
            }
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    for comment in &comments {
                        egui::Frame::new()
                            .fill(if comment.resolved { FIELD } else { CARD })
                            .corner_radius(egui::CornerRadius::same(10))
                            .inner_margin(egui::Margin::same(10))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(dim(comment.author_name.clone()));
                                    ui.label(muted(rel_time(comment.created_at)));
                                    ui.with_layout(
                                        Layout::right_to_left(Align::Center),
                                        |ui| {
                                            if ui
                                                .add(egui::Button::new(muted(if comment
                                                    .resolved
                                                {
                                                    "取消解决"
                                                } else {
                                                    "解决"
                                                }))
                                                .frame(false))
                                                .clicked()
                                            {
                                                send = Some(Command::UpdateComment {
                                                    id: comment.id.clone(),
                                                    version: comment.version,
                                                    body: None,
                                                    resolved: Some(!comment.resolved),
                                                    deleted: false,
                                                });
                                            }
                                            if ui
                                                .add(egui::Button::new(muted("删除")).frame(false))
                                                .clicked()
                                            {
                                                send = Some(Command::UpdateComment {
                                                    id: comment.id.clone(),
                                                    version: comment.version,
                                                    body: None,
                                                    resolved: None,
                                                    deleted: true,
                                                });
                                            }
                                        },
                                    );
                                });
                                ui.label(body(comment.body.clone()));
                            });
                        ui.add_space(6.0);
                    }
                });
            ui.add_space(8.0);
            ui.add(
                egui::TextEdit::multiline(draft)
                    .desired_rows(2)
                    .hint_text("写一句留言…")
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("发送").clicked() {
                    let trimmed = draft.trim().to_string();
                    if !trimmed.is_empty() {
                        send = Some(Command::CreateComment {
                            target_kind: kind,
                            target_id: target.clone(),
                            body: trimmed,
                        });
                        draft.clear();
                    }
                }
                if ui.button("关闭").clicked() {
                    close = true;
                }
            });
        });
    if let Some(command) = send {
        app.send(command);
    }
    if close {
        app.dialog = Dialog::None;
    }
}

fn members_window(app: &mut App, ctx: &egui::Context) {
    if !matches!(app.dialog, Dialog::Members) {
        return;
    }
    let mut close = false;
    let mut pick: Option<Option<(String, String)>> = None;
    egui::Window::new("成员")
        .default_width(320.0)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .frame(window_frame())
        .show(ctx, |ui| {
            if ui
                .selectable_label(app.member_filter.is_none(), "共享空间（所有人）")
                .clicked()
            {
                pick = Some(None);
            }
            for user in app.store.users.clone() {
                let label = if user.id == app.me_id {
                    format!("{}（我）", user.name)
                } else {
                    user.name.clone()
                };
                let selected = app.member_filter.as_deref() == Some(user.id.as_str());
                if ui.selectable_label(selected, label).clicked() {
                    pick = Some(Some((user.id.clone(), user.name.clone())));
                }
            }
            ui.add_space(10.0);
            if ui.button("关闭").clicked() {
                close = true;
            }
        });
    if let Some(choice) = pick {
        match choice {
            Some((id, name)) => {
                app.member_filter = Some(id);
                app.member_name = name;
            }
            None => {
                app.member_filter = None;
                app.member_name.clear();
            }
        }
        app.dialog = Dialog::None;
    }
    if close {
        app.dialog = Dialog::None;
    }
}

fn invites_window(app: &mut App, ctx: &egui::Context) {
    let Dialog::Invites { list, loading } = &app.dialog else {
        return;
    };
    let list = list.clone();
    let loading = *loading;
    let mut close = false;
    let mut create = false;
    egui::Window::new("邀请码")
        .default_width(420.0)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .frame(window_frame())
        .show(ctx, |ui| {
            if loading {
                ui.label(muted("加载中…"));
            }
            for invite in &list {
                ui.horizontal(|ui| {
                    ui.label(metric(invite.code.clone(), 15.0));
                    if invite.revoked {
                        pill(ui, "已撤销", DANGER);
                    } else {
                        pill(ui, "可用", OK);
                    }
                    ui.label(muted(format!(
                        "已用 {}/{}",
                        invite.used_count,
                        invite
                            .max_uses
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "∞".into())
                    )));
                });
                if !invite.note.is_empty() {
                    ui.label(muted(invite.note.clone()));
                }
                ui.add_space(4.0);
            }
            if list.is_empty() && !loading {
                ui.label(muted("还没有邀请码"));
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("生成新邀请码").clicked() {
                    create = true;
                }
                if ui.button("关闭").clicked() {
                    close = true;
                }
            });
        });
    if create {
        app.send(Command::CreateInvite {
            note: "Glasswing".to_string(),
        });
        if let Dialog::Invites { loading, .. } = &mut app.dialog {
            *loading = true;
        }
    }
    if close {
        app.dialog = Dialog::None;
    }
}

fn search_window(app: &mut App, ctx: &egui::Context) {
    let Dialog::Search { query } = &mut app.dialog else {
        return;
    };
    let mut close = false;
    let mut goto_page: Option<String> = None;
    let mut goto_habit: Option<String> = None;
    egui::Window::new("搜索")
        .default_width(460.0)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, egui::Vec2::new(0.0, 80.0))
        .frame(window_frame())
        .show(ctx, |ui| {
            let response = ui.add(
                egui::TextEdit::singleline(query)
                    .hint_text("页面、待办、习惯…")
                    .desired_width(f32::INFINITY),
            );
            response.request_focus();
            let needle = query.to_lowercase();
            ui.add_space(8.0);
            if needle.is_empty() {
                ui.label(muted("输入关键词开始搜索"));
            } else {
                for page in app.store.visible_pages() {
                    if page.title.to_lowercase().contains(&needle)
                        && ui.selectable_label(false, format!("页面 · {}", page.title)).clicked()
                    {
                        goto_page = Some(page.id.clone());
                    }
                }
                for block in app.store.blocks.iter().filter(|b| !b.deleted) {
                    if block.text.to_lowercase().contains(&needle)
                        && ui
                            .selectable_label(false, format!("内容 · {}", block.text))
                            .clicked()
                    {
                        goto_page = Some(block.page_id.clone());
                    }
                }
                for habit in app.store.active_habits() {
                    if habit.title.to_lowercase().contains(&needle)
                        && ui
                            .selectable_label(false, format!("习惯 · {}", habit.title))
                            .clicked()
                    {
                        goto_habit = Some(habit.id.clone());
                    }
                }
            }
            ui.add_space(8.0);
            if ui.button("关闭").clicked() {
                close = true;
            }
        });
    if let Some(page) = goto_page {
        app.selected_page = Some(page);
        app.view = crate::app::View::Workspace;
        app.dialog = Dialog::None;
    }
    if let Some(habit) = goto_habit {
        app.selected_habit = Some(habit);
        app.view = crate::app::View::Habits;
        app.dialog = Dialog::None;
    }
    if close {
        app.dialog = Dialog::None;
    }
}

fn settings_window(app: &mut App, ctx: &egui::Context) {
    if !matches!(app.dialog, Dialog::Settings) {
        return;
    }
    let mut close = false;
    egui::Window::new("设置")
        .default_width(380.0)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .frame(window_frame())
        .show(ctx, |ui| {
            kv(ui, "账号", app.me.clone());
            kv(ui, "空间", app.store.workspace_name.clone());
            kv(ui, "成员", format!("{} 人", app.store.users.len()));
            kv(ui, "协议", "v5");
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("立即同步").clicked() {
                    app.send(Command::SyncNow);
                }
                if ui.button("成员列表").clicked() {
                    app.send(Command::Members);
                    app.dialog = Dialog::Members;
                }
                if ui.button("邀请码").clicked() {
                    app.send(Command::ListInvites);
                    app.dialog = Dialog::Invites {
                        list: Vec::new(),
                        loading: true,
                    };
                }
            });
            ui.add_space(12.0);
            ui.label(muted("所有数据与 totion 服务端共用，可在手机或终端里继续看。"));
            ui.add_space(8.0);
            if ui.button("关闭").clicked() {
                close = true;
            }
        });
    if close {
        app.dialog = Dialog::None;
    }
}
