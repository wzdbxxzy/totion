//! 工作区：左侧页面列表，右侧页面内容块。
//!
//! 交互：点页面切换；「⋯」重命名；右上角「＋ 待办 / ＋ 文本」新建；
//! 待办圆圈勾选；内容行 hover 后出现「编辑 / 留言 / 删除」。
//! 新建与编辑内容都用页面内的轻量弹窗，状态暂存在 egui 的临时数据里。

use chrono::Local;
use eframe::egui::{
    self, Align, Align2, Color32, FontFamily, FontId, Layout, Rect, RichText, Sense, Stroke, Vec2,
};
use totion_core::{Block, BlockKind, CommentTargetKind, JournalEntry, JournalTarget, Page, PageKind};

use crate::app::{App, Editor, View};
use crate::net::Command;
use crate::theme::*;
use crate::views::today::{empty_state, open_comments};

const DRAFT_ID: &str = "glasswing_workspace_draft";
/// 内容行右侧操作区固定预留的宽度，避免 hover 时整行跳动。
const ACTION_W: f32 = 164.0;

/// 新建 / 编辑内容块的临时状态。
#[derive(Clone)]
struct Draft {
    open: bool,
    page_id: String,
    kind: BlockKind,
    block_id: Option<String>,
    version: i64,
    text: String,
    time: String,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            open: false,
            page_id: String::new(),
            kind: BlockKind::Text,
            block_id: None,
            version: 0,
            text: String::new(),
            time: String::new(),
        }
    }
}

pub fn draw(app: &mut App, ui: &mut egui::Ui) {
    let pages = app.store.visible_pages();

    // 选中项为空或已失效时，自动落到第一个页面。
    let valid = app
        .selected_page
        .as_deref()
        .map(|id| pages.iter().any(|page| page.id == id))
        .unwrap_or(false);
    if !valid {
        app.selected_page = pages.first().map(|page| page.id.clone());
    }
    let selected = pages
        .iter()
        .find(|page| Some(page.id.as_str()) == app.selected_page.as_deref())
        .cloned();

    let height = ui.available_height();
    let width = ui.available_width();
    let left_w = 250.0f32.min((width - 320.0).max(180.0));

    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(left_w, height),
            Layout::top_down(Align::Min),
            |ui| page_list(app, ui, &pages),
        );
        ui.add_space(12.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), height),
            Layout::top_down(Align::Min),
            |ui| page_panel(app, ui, selected.as_ref()),
        );
    });

    draft_window(app, ui.ctx());
}

// ---------------------------------------------------------------------------
// 左侧页面列表
// ---------------------------------------------------------------------------

fn page_list(app: &mut App, ui: &mut egui::Ui, pages: &[Page]) {
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            section(ui, "页面");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let color = if app.is_writable() { text() } else { text_muted() };
                let button =
                    egui::Button::new(RichText::new("＋ 新建页面").size(12.0).color(color))
                        .fill(accent_soft());
                if ui.add(button).clicked() {
                    if app.is_writable() {
                        app.editor = Editor::Page {
                            page: None,
                            title: String::new(),
                        };
                    } else {
                        app.toast("他人视角只读，可留言", true);
                    }
                }
            });
        });
        ui.add_space(8.0);

        if pages.is_empty() {
            empty_state(ui, "还没有页面", "点上方「＋ 新建页面」建一个");
            return;
        }

        let max_height = ui.available_height().max(180.0);
        egui::ScrollArea::vertical()
            .id_salt("ws_page_list")
            .auto_shrink([false, false])
            .max_height(max_height)
            .show(ui, |ui| {
                let mut last_kind: Option<PageKind> = None;
                for page in pages {
                    if last_kind != Some(page.kind) {
                        if last_kind.is_some() {
                            ui.add_space(6.0);
                        }
                        section(ui, page.kind.label());
                        last_kind = Some(page.kind);
                    }
                    page_row(app, ui, page);
                }
            });
    });
}

fn page_row(app: &mut App, ui: &mut egui::Ui, page: &Page) {
    let selected = app.selected_page.as_deref() == Some(page.id.as_str());
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::click());

    let fill = if selected {
        Color32::from_rgba_premultiplied(accent().r(), accent().g(), accent().b(), 26)
    } else if resp.hovered() {
        card_hi()
    } else {
        Color32::TRANSPARENT
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(8), fill);
    if selected {
        let bar = Rect::from_min_size(
            egui::pos2(rect.left() + 4.0, rect.center().y - 8.0),
            Vec2::new(3.0, 16.0),
        );
        ui.painter()
            .rect_filled(bar, egui::CornerRadius::same(2), accent());
    }

    let more_rect = Rect::from_center_size(
        egui::pos2(rect.right() - 16.0, rect.center().y),
        Vec2::new(24.0, 24.0),
    );
    let more = ui.interact(
        more_rect,
        ui.id().with(("ws_page_more", &page.id)),
        Sense::click(),
    );
    if more.hovered() {
        ui.painter()
            .rect_filled(more_rect, egui::CornerRadius::same(6), card_hi());
    }

    ui.painter().text(
        egui::pos2(rect.left() + 14.0, rect.center().y),
        Align2::LEFT_CENTER,
        ellipsize(&page.title, 12),
        FontId::new(13.5, FontFamily::Proportional),
        if selected { text() } else { text_dim() },
    );
    ui.painter().text(
        more_rect.center(),
        Align2::CENTER_CENTER,
        "⋯",
        FontId::new(15.0, FontFamily::Proportional),
        if more.hovered() { text() } else { text_muted() },
    );

    if more.clicked() {
        if app.is_writable() {
            app.editor = Editor::Page {
                page: Some(page.clone()),
                title: page.title.clone(),
            };
        } else {
            app.toast("他人视角只读", true);
        }
    } else if resp.clicked() {
        app.selected_page = Some(page.id.clone());
    }
}

// ---------------------------------------------------------------------------
// 右侧页面内容
// ---------------------------------------------------------------------------

fn page_panel(app: &mut App, ui: &mut egui::Ui, page: Option<&Page>) {
    let Some(page) = page.cloned() else {
        card_frame().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(title("工作区", 24.0));
            ui.add_space(2.0);
            empty_state(ui, "还没有选中页面", "从左侧选一个，或点「＋ 新建」创建一个");
        });
        return;
    };

    egui::ScrollArea::vertical()
        .id_salt("ws_page_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            page_header(app, ui, &page);
            ui.add_space(12.0);
            block_list(app, ui, &page);
            ui.add_space(12.0);
            recent_card(app, ui);
        });
}

fn page_header(app: &mut App, ui: &mut egui::Ui, page: &Page) {
    let width = ui.available_width();
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width((width - 230.0).max(180.0));
            ui.add(egui::Label::new(title(page.title.clone(), 24.0)).wrap());
            ui.add_space(2.0);
            let blocks = app.store.page_blocks(&page.id).len();
            ui.label(muted(format!(
                "{} · {} 个内容块",
                page.kind.label(),
                blocks
            )));
        });

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let writable = app.is_writable();
            let todo = egui::Button::new(
                RichText::new("＋ 待办").color(if writable { text() } else { text_muted() }),
            )
            .fill(if writable {
                accent_soft()
            } else {
                Color32::TRANSPARENT
            });
            if ui.add(todo).clicked() {
                if writable {
                    open_draft(ui, &page.id, BlockKind::Todo, None);
                } else {
                    app.toast("他人视角只读，可留言", true);
                }
            }
            let text = egui::Button::new(RichText::new("＋ 文本").color(text_dim()))
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::new(1.0_f32, line()));
            if ui.add(text).clicked() {
                if writable {
                    open_draft(ui, &page.id, BlockKind::Text, None);
                } else {
                    app.toast("他人视角只读，可留言", true);
                }
            }
        });
    });
}

fn block_list(app: &mut App, ui: &mut egui::Ui, page: &Page) {
    let blocks = app.store.page_blocks(&page.id);
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            section(ui, "内容");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(muted(format!("{} 条", blocks.len())));
            });
        });
        ui.add_space(6.0);

        if blocks.is_empty() {
            empty_state(ui, "这个页面还没有内容", "用右上角「＋ 待办」或「＋ 文本」添加");
            return;
        }

        for block in &blocks {
            block_row(app, ui, block);
            ui.add_space(2.0);
        }
    });
}

fn block_row(app: &mut App, ui: &mut egui::Ui, block: &Block) {
    // 按钮只在 hover 后出现；用上一帧的 hover 状态决定是否绘制，避免布局跳动。
    let hover_key = egui::Id::new(("ws_block_hover", &block.id));
    let was_hovered = ui
        .data(|data| data.get_temp::<bool>(hover_key).unwrap_or(false));

    let mut toggle = false;
    let mut edit = false;
    let mut comment = false;
    let mut delete = false;

    let row = egui::Frame::new()
        .fill(if was_hovered {
            card_hi()
        } else {
            Color32::TRANSPARENT
        })
        .corner_radius(egui::CornerRadius::same(9))
        .inner_margin(egui::Margin::symmetric(10, 8));

    let resp = row
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let lead_sense = if block.kind == BlockKind::Todo {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                let (lead_rect, lead_resp) =
                    ui.allocate_exact_size(Vec2::new(18.0, 18.0), lead_sense);
                if block.kind == BlockKind::Todo {
                    let stroke = if block.checked {
                        Stroke::new(1.5_f32, ok())
                    } else {
                        Stroke::new(
                            1.2_f32,
                            if lead_resp.hovered() {
                                accent()
                            } else {
                                text_muted()
                            },
                        )
                    };
                    ui.painter().circle_stroke(lead_rect.center(), 7.5, stroke);
                    if block.checked {
                        ui.painter().circle_filled(lead_rect.center(), 4.5, ok());
                    }
                    if lead_resp.clicked() {
                        toggle = true;
                    }
                } else {
                    let (radius, color) = if block.kind == BlockKind::Heading {
                        (2.5, accent())
                    } else {
                        (1.6, text_muted())
                    };
                    ui.painter().circle_filled(lead_rect.center(), radius, color);
                }

                let text_w = (ui.available_width() - ACTION_W - 6.0).max(60.0);
                ui.vertical(|ui| {
                    ui.set_width(text_w);
                    if block.kind == BlockKind::Heading {
                        ui.add(egui::Label::new(title(block.text.clone(), 16.5)).wrap());
                    } else if block.kind == BlockKind::Todo && block.checked {
                        ui.add(
                            egui::Label::new(
                                RichText::new(block.text.clone())
                                    .color(text_muted())
                                    .strikethrough(),
                            )
                            .wrap(),
                        );
                    } else {
                        ui.add(egui::Label::new(body(block.text.clone())).wrap());
                    }

                    let mut meta = Vec::new();
                    if let Some(time) = &block.start_time {
                        meta.push(time.clone());
                    }
                    if let Some(date) = &block.due_date {
                        if block.kind != BlockKind::CalendarEvent {
                            meta.push(format!("截止 {date}"));
                        }
                    }
                    if block.kind == BlockKind::Todo && block.checked {
                        if let Some(at) = block.completed_at {
                            meta.push(format!(
                                "完成于 {}",
                                at.with_timezone(&Local).format("%m-%d %H:%M")
                            ));
                        }
                    }
                    if !meta.is_empty() {
                        ui.label(muted(meta.join(" · ")));
                    }

                    let open = app
                        .store
                        .comments_for(CommentTargetKind::Block, &block.id)
                        .iter()
                        .filter(|item| !item.resolved)
                        .count();
                    let journals = app
                        .store
                        .journals_for(JournalTarget::Block, &block.id)
                        .len();
                    if open > 0 || journals > 0 {
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            if open > 0 {
                                pill(ui, &format!("留言 {open}"), warn());
                            }
                            if journals > 0 {
                                pill(ui, &format!("总结 {journals}"), accent());
                            }
                        });
                    }
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    if was_hovered {
                        if action_button(ui, "删除", danger()) {
                            delete = true;
                        }
                        if action_button(ui, "留言", text_muted()) {
                            comment = true;
                        }
                        if action_button(ui, "编辑", text_muted()) {
                            edit = true;
                        }
                    } else {
                        ui.add_space(ACTION_W);
                    }
                });
            });
        })
        .response;

    ui.data_mut(|data| data.insert_temp(hover_key, resp.hovered()));

    if toggle {
        if app.is_writable() {
            app.send(Command::UpdateBlock {
                id: block.id.clone(),
                version: block.version,
                text: None,
                checked: Some(!block.checked),
                deleted: false,
            });
        } else {
            app.toast("他人视角只读，可留言", true);
        }
    }
    if edit {
        if app.is_writable() {
            open_draft(ui, &block.page_id, block.kind, Some(block));
        } else {
            app.toast("他人视角只读，可留言", true);
        }
    }
    if comment {
        open_comments(
            app,
            CommentTargetKind::Block,
            &block.id,
            &first_line(&block.text, 24),
        );
    }
    if delete {
        if app.is_writable() {
            app.send(Command::UpdateBlock {
                id: block.id.clone(),
                version: block.version,
                text: None,
                checked: None,
                deleted: true,
            });
        } else {
            app.toast("他人视角只读", true);
        }
    }
}

// ---------------------------------------------------------------------------
// 最近总结
// ---------------------------------------------------------------------------

fn recent_card(app: &mut App, ui: &mut egui::Ui) {
    let journals = app.store.all_journals();
    card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        section(ui, "最近总结");
        ui.add_space(4.0);
        if journals.is_empty() {
            empty_state(ui, "还没有总结", "在今日或内容块下记一句");
            return;
        }

        let mut jump: Option<JournalEntry> = None;
        for journal in journals.iter().take(5) {
            let width = ui.available_width();
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 44.0), Sense::click());
            let fill = if resp.hovered() {
                card_hi()
            } else {
                Color32::TRANSPARENT
            };
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(8), fill);

            let inner = rect.shrink2(Vec2::new(10.0, 5.0));
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(Layout::top_down(Align::Min)),
            );
            child.spacing_mut().item_spacing.y = 2.0;
            child.set_width(inner.width());
            child.add(
                egui::Label::new(muted(format!(
                    "{} {} · {}",
                    journal.date, journal.time, journal.author_name
                )))
                .truncate(),
            );
            child.add(egui::Label::new(dim(first_line(&journal.body, 80))).truncate());

            if resp.clicked() {
                jump = Some(journal.clone());
            }
            ui.add_space(2.0);
        }

        if let Some(journal) = jump {
            jump_to_journal(app, &journal);
        }
    });
}

fn jump_to_journal(app: &mut App, journal: &JournalEntry) {
    match journal.target_kind {
        JournalTarget::Block => {
            let page_id = app
                .store
                .blocks
                .iter()
                .find(|block| block.id == journal.target_id && !block.deleted)
                .map(|block| block.page_id.clone());
            match page_id {
                Some(page_id) => jump_to_page(app, &page_id),
                None => app.toast("找不到对应页面", true),
            }
        }
        JournalTarget::Daily => {
            let page_id = if app
                .store
                .pages
                .iter()
                .any(|page| page.id == journal.target_id && !page.deleted)
            {
                Some(journal.target_id.clone())
            } else {
                app.store
                    .pages
                    .iter()
                    .find(|page| {
                        !page.deleted && page.date.as_deref() == Some(journal.date.as_str())
                    })
                    .map(|page| page.id.clone())
            };
            match page_id {
                Some(page_id) => jump_to_page(app, &page_id),
                None => app.toast("找不到对应页面", true),
            }
        }
        JournalTarget::Habit => {
            app.selected_habit = Some(journal.target_id.clone());
            app.view = View::Habits;
        }
    }
}

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
// 新建 / 编辑弹窗
// ---------------------------------------------------------------------------

fn open_draft(ui: &mut egui::Ui, page_id: &str, kind: BlockKind, block: Option<&Block>) {
    let draft = match block {
        Some(existing) => Draft {
            open: true,
            page_id: existing.page_id.clone(),
            kind: existing.kind,
            block_id: Some(existing.id.clone()),
            version: existing.version,
            text: existing.text.clone(),
            time: existing.start_time.clone().unwrap_or_default(),
        },
        None => Draft {
            open: true,
            page_id: page_id.to_string(),
            kind,
            block_id: None,
            version: 0,
            text: String::new(),
            time: String::new(),
        },
    };
    ui.data_mut(|data| data.insert_temp(egui::Id::new(DRAFT_ID), draft));
}

fn draft_window(app: &mut App, ctx: &egui::Context) {
    let id = egui::Id::new(DRAFT_ID);
    let mut draft: Draft = ctx.data(|data| data.get_temp(id).unwrap_or_default());
    if !draft.open {
        return;
    }

    let editing = draft.block_id.is_some();
    let is_todo = draft.kind == BlockKind::Todo;
    let heading = if editing {
        "编辑内容"
    } else if is_todo {
        "新建待办"
    } else {
        "新建文本"
    };

    let mut open = true;
    let mut save = false;
    let mut cancel = false;

    egui::Window::new(RichText::new(heading).size(16.0))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .frame(card_frame())
        .show(ctx, |ui| {
            ui.set_width(440.0);
            section(ui, "内容");
            ui.add(
                egui::TextEdit::multiline(&mut draft.text)
                    .desired_rows(3)
                    .hint_text("写点什么…")
                    .desired_width(f32::INFINITY),
            );
            if !editing && is_todo {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    section(ui, "时间");
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.time)
                            .hint_text("HH:MM")
                            .desired_width(90.0),
                    );
                });
            }
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui.button("保存").clicked() {
                    save = true;
                }
                if ui.button("取消").clicked() {
                    cancel = true;
                }
            });
        });

    if save {
        let text = draft.text.trim().to_string();
        if text.is_empty() {
            app.toast("内容不能为空", true);
        } else {
            match draft.block_id.clone() {
                Some(block_id) => {
                    app.send(Command::UpdateBlock {
                        id: block_id,
                        version: draft.version,
                        text: Some(text),
                        checked: None,
                        deleted: false,
                    });
                }
                None => {
                    let time = draft.time.trim();
                    app.send(Command::CreateBlock {
                        page_id: draft.page_id.clone(),
                        kind: draft.kind,
                        text,
                        due_date: None,
                        start_time: if time.is_empty() {
                            None
                        } else {
                            Some(time.to_string())
                        },
                    });
                }
            }
            draft.open = false;
        }
    }
    if cancel || !open {
        draft.open = false;
    }

    ctx.data_mut(|data| data.insert_temp(id, draft));
}

// ---------------------------------------------------------------------------
// 小工具
// ---------------------------------------------------------------------------

fn action_button(ui: &mut egui::Ui, text: &str, color: Color32) -> bool {
    ui.add(egui::Button::new(RichText::new(text).size(12.0).color(color)).small())
        .clicked()
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
