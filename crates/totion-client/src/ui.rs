use chrono::{Datelike, Local, NaiveDate, Utc};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Gauge, Paragraph, Sparkline, Wrap},
    Frame,
};
use totion_core::HabitStats;

use crate::app::{
    AgendaPanel, App, CommentPanel, Editing, EventForm, Focus, HabitForm, HelpPanel, InvitePanel,
    MainState, MemberPanel, Modal, MouseAction, Screen, SearchState, SetupForm, TodayFocus,
    TodayRow, View,
};
use crate::charts;

/// 选中行统一背景（lazygit/k9s 风格的深色高亮）。
pub(crate) const SELECTION_BG: Color = Color::Rgb(38, 45, 56);

pub(crate) fn selection_style() -> Style {
    Style::default()
        .bg(SELECTION_BG)
        .add_modifier(Modifier::BOLD)
}

pub fn draw(frame: &mut Frame, app: &App) {
    match &app.screen {
        Screen::Setup(form) => draw_setup(frame, form),
        Screen::Main(main) => draw_main(frame, main),
    }
}

fn draw_setup(frame: &mut Frame, form: &SetupForm) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(12),
            Constraint::Min(1),
        ])
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" totion · 连接共享空间 ");
    let inner = block.inner(chunks[1]);
    frame.render_widget(block, chunks[1]);

    let mut lines = vec![
        Line::from(Span::styled(
            "连接服务器；第一次使用需要邀请码注册，之后自动免密登录。",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
    ];

    for (index, (label, value)) in form.fields().into_iter().enumerate() {
        let active = index == form.field;
        let marker = if active { "▶ " } else { "  " };
        let mut text = format!("{marker}{label}: {value}");
        if active {
            text.push('█');
        }
        let style = if active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(text, style)));
    }

    lines.push(Line::from(""));
    if let Some(error) = &form.error {
        lines.push(Line::from(Span::styled(
            error.clone(),
            Style::default().fg(Color::Red),
        )));
    }
    lines.push(Line::from(Span::styled(
        "Tab 切换字段 · Enter 连接 · Ctrl+R 重置密钥 · Ctrl+C 退出",
        Style::default().fg(Color::DarkGray),
    )));

    frame.render_widget(Paragraph::new(lines).block(Block::default()), inner);
}

fn draw_main(frame: &mut Frame, main: &MainState) {
    let area = frame.area();
    main.clear_mouse_hits();
    let editing_height = if main.editing().is_some() { 4 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(editing_height),
            Constraint::Length(2),
        ])
        .split(area);

    draw_header(frame, chunks[0], main);
    draw_nav(frame, chunks[1], main);

    match main.view() {
        View::Today => draw_today(frame, chunks[2], main),
        View::Workspace => draw_workspace(frame, chunks[2], main),
        View::Calendar => draw_calendar(frame, chunks[2], main),
        View::Habits => draw_habits(frame, chunks[2], main),
        View::Stats => draw_stats(frame, chunks[2], main),
        View::Inbox => draw_inbox(frame, chunks[2], main),
    }

    if let Some(editing) = main.editing() {
        draw_editing(frame, chunks[3], editing);
    }

    draw_footer(frame, chunks[4], main);

    if let Some(modal) = main.modal() {
        draw_modal(frame, main, modal);
    }
}

/// 顶端导航栏：6 个界面 + 常用操作，都可以鼠标点击。
fn draw_nav(frame: &mut Frame, area: Rect, main: &MainState) {
    let mut line = HitLine::new(main, area.x, area.y);
    let tabs = [
        (View::Today, "1 今日"),
        (View::Workspace, "2 工作区"),
        (View::Calendar, "3 日历"),
        (View::Habits, "4 习惯"),
        (View::Stats, "5 统计"),
        (View::Inbox, "6 留言"),
    ];
    for (view, label) in tabs {
        let style = if main.view() == view {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        line.push(
            format!(" {label} "),
            style,
            Some(MouseAction::SwitchView(view)),
        );
    }
    line.push("  ", Style::default(), None);
    let actions = [
        (" m 成员 ", MouseAction::OpenMembers),
        (" i 邀请 ", MouseAction::OpenInvites),
        (" / 搜索 ", MouseAction::OpenSearch),
        (" ? 帮助 ", MouseAction::OpenHelp),
        (" r 同步 ", MouseAction::SyncNow),
    ];
    for (label, action) in actions {
        line.push(label, Style::default().fg(Color::Yellow), Some(action));
    }
    frame.render_widget(Paragraph::new(line.into_line()), area);
}

fn draw_header(frame: &mut Frame, area: Rect, main: &MainState) {
    let mut spans = vec![Span::styled(
        format!(
            " totion · {} · {} · {} 位成员 · {} [{}/6]",
            main.workspace_name(),
            main.me(),
            main.users().len(),
            main.view().label(),
            view_number(main.view())
        ),
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];
    if let Some(user) = main.viewed_user() {
        let label = if main.read_only() {
            format!(" 视角：{}（只读，可评论） ", user.name)
        } else {
            format!(" 视角：{}（我） ", user.name)
        };
        spans.push(Span::styled(
            label,
            Style::default()
                .fg(Color::Black)
                .bg(if main.read_only() {
                    Color::Yellow
                } else {
                    Color::Green
                })
                .add_modifier(Modifier::BOLD),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn view_number(view: View) -> u8 {
    match view {
        View::Today => 1,
        View::Workspace => 2,
        View::Calendar => 3,
        View::Habits => 4,
        View::Stats => 5,
        View::Inbox => 6,
    }
}

/// 可视窗口：让选中项始终留在高度为 `height` 的窗口内。
pub(crate) fn window_range(total: usize, selected: usize, height: usize) -> (usize, usize) {
    if height == 0 || total == 0 {
        return (0, 0);
    }
    if total <= height {
        return (0, total);
    }
    let selected = selected.min(total - 1);
    let half = height / 2;
    let start = selected.saturating_sub(half).min(total - height);
    (start, start + height)
}

/// 变高行列表（例如留言：每条占多行）取可视起点，保证选中块落在窗口内。
pub(crate) fn scroll_start(heights: &[usize], selected: usize, budget: usize) -> usize {
    if heights.is_empty() || budget == 0 {
        return 0;
    }
    let selected = selected.min(heights.len() - 1);
    let mut start = selected;
    let mut used = heights[selected].min(budget);
    while start > 0 {
        let next = heights[start - 1];
        if used + next > budget {
            break;
        }
        start -= 1;
        used += next;
    }
    start
}

/// 逐段拼接一行文本，同时按显示宽度登记鼠标点击区域。
struct HitLine<'a> {
    spans: Vec<Span<'static>>,
    x: u16,
    y: u16,
    main: &'a MainState,
}

impl<'a> HitLine<'a> {
    fn new(main: &'a MainState, x: u16, y: u16) -> Self {
        Self {
            spans: Vec::new(),
            x,
            y,
            main,
        }
    }

    fn push(&mut self, text: impl Into<String>, style: Style, action: Option<MouseAction>) {
        let text: String = text.into();
        let width = unicode_width::UnicodeWidthStr::width(text.as_str()) as u16;
        if let Some(action) = action {
            self.main.push_mouse_hit(
                Rect {
                    x: self.x,
                    y: self.y,
                    width,
                    height: 1,
                },
                action,
            );
        }
        self.spans.push(Span::styled(text, style));
        self.x = self.x.saturating_add(width);
    }

    fn into_line(self) -> Line<'static> {
        Line::from(self.spans)
    }
}

/// 列表/内容区里第 `row` 行的可点击区域（相对内边距，跳过圆角边框）。
fn list_row_rect(area: Rect, row: usize) -> Option<Rect> {
    let height = area.height.saturating_sub(2);
    if row as u16 >= height {
        return None;
    }
    Some(Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1 + row as u16),
        width: area.width.saturating_sub(2),
        height: 1,
    })
}

fn block_title(title: impl Into<String>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(58, 68, 80)))
        .title(Span::styled(
            title.into(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
}

fn block_title_focused(title: impl Into<String>, focused: bool) -> Block<'static> {
    let border = if focused {
        Color::Cyan
    } else {
        Color::Rgb(58, 68, 80)
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border))
        .title(Span::styled(
            title.into(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
}

fn comment_suffix(count: usize) -> String {
    if count == 0 {
        String::new()
    } else {
        format!("  [{count}]")
    }
}

fn draw_editing(frame: &mut Frame, area: Rect, editing: &Editing) {
    let lines = editing_preview(editing);
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            " 编辑（Enter 保存 · Esc 取消） ",
            Style::default().fg(Color::Cyan),
        ));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn editing_preview(editing: &Editing) -> Vec<Line<'static>> {
    let cursor = editing.cursor.min(editing.buffer.len());
    let before = &editing.buffer[..cursor];
    let line_index = before.matches('\n').count();
    let lines: Vec<&str> = editing.buffer.split('\n').collect();
    let start = line_index.saturating_sub(1);
    let end = (line_index + 2).min(lines.len());
    let mut output = Vec::new();
    for (index, line) in lines.iter().enumerate().take(end).skip(start) {
        let text = if index == line_index {
            let offset = before.rfind('\n').map(|position| position + 1).unwrap_or(0);
            let column = (cursor - offset).min(line.len());
            let (left, right) = (*line).split_at(column);
            format!(" {left}█{right}")
        } else {
            format!(" {line}")
        };
        output.push(Line::from(text));
    }
    output
}

fn draw_footer(frame: &mut Frame, area: Rect, main: &MainState) {
    let status_line = if let Some(notice) = main.notice() {
        Line::from(Span::styled(
            format!(" {notice}"),
            Style::default().fg(Color::Yellow),
        ))
    } else {
        Line::from(Span::styled(
            format!(" {}  ·  m 成员视角", main.status()),
            Style::default().fg(Color::Gray),
        ))
    };

    let help = if main.editing().is_some() {
        " Enter 保存 · Esc 取消 · 总结/评论等多行内容用 Ctrl+S 保存"
    } else if let Some(modal) = main.modal() {
        match modal {
            Modal::EventForm(_) => " Tab 切换字段 · Enter 提交 · Esc 取消",
            Modal::HabitForm(_) => " Tab 切换字段 · Ctrl+F 切换频率 · Enter 提交 · Esc 取消",
            Modal::Comments(_) => " a 添加 · e 编辑 · r 解决/取消 · d 删除 · j/k 选择 · Esc 关闭",
            Modal::Search(_) => " 输入搜索 · j/k 选择 · Enter 跳转 · Esc 关闭",
            Modal::Agenda(_) => {
                " j/k 选择 · Space 勾选 · Enter 编辑 · d 删除 · t/c 新建 · Esc 关闭"
            }
            Modal::Invites(_) => " g 生成新码 · x 撤销/恢复 · j/k 选择 · Esc 关闭",
            Modal::Members(_) => " j/k 选择 · Enter 切换视角 · Esc 关闭",
            Modal::Confirm(_) => " 输入 delete 并回车确认 · Esc 取消",
            Modal::Help(_) => " j/k 滚动 · Esc 关闭",
        }
    } else {
        match main.view() {
            View::Today => match main.today_focus() {
                TodayFocus::Items => {
                    " j/k 移动 · Space 勾选 · Enter 编辑 · d 删除 · a 总结 · t 新建 · h/l 切天 · w 周 · Tab 面板"
                }
                TodayFocus::Habits => {
                    " j/k 移动 · Space 打卡 · u 撤销 · a 总结 · d 删除 · h/l 切天 · w 周 · Tab 面板"
                }
                TodayFocus::Journal => {
                    " Enter/a 编辑总结 · d 删除 · h/l 切天 · w 周 · Tab 面板"
                }
            },
            View::Workspace => {
                " Tab 页面/内容 · j/k 移动 · Enter 编辑 · n 文本 · t 待办 · N 新页面 · a 每日页 · d 删除 · J 全部总结 · C 评论"
            }
            View::Calendar => {
                " h/l 选日 · j/k 前后周 · [ ] 换月 · Enter/双击 议程 · v 框选 · t 新建/批量待办"
            }
            View::Habits => {
                " n 新建习惯 · j/k 选择 · Space 打卡 · u 撤销 · e 改名称 · d 删除 · C 评论 · q 退出"
            }
            View::Stats => " h/l 切换 7/14/30/90 天窗口 · q 退出",
            View::Inbox => {
                " j/k 选择 · Enter 跳转 · a 回复 · r 解决/取消 · e 编辑 · d 删除 · q 退出"
            }
        }
    };

    let lines = vec![
        status_line,
        Line::from(Span::styled(help, Style::default().fg(Color::DarkGray))),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_workspace(frame: &mut Frame, area: Rect, main: &MainState) {
    if main.workspace_journal_mode() {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
            .split(area);
        draw_pages(frame, columns[0], main);
        draw_journal_list(frame, columns[1], main);
        return;
    }
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(area);
    draw_pages(frame, columns[0], main);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(columns[1]);
    draw_blocks(frame, right[0], main);
    draw_summary_preview(frame, right[1], main);
}

/// 工作区右下常驻的「全部总结」预览，无需按 J 就能看到今日写过的总结。
fn draw_summary_preview(frame: &mut Frame, area: Rect, main: &MainState) {
    let journals = main.all_journals();
    let limit = area.height.saturating_sub(2) as usize;
    let mut lines: Vec<Line> = Vec::new();
    for (index, journal) in journals.iter().take(limit).enumerate() {
        let tag = match journal.target_kind {
            totion_core::JournalTarget::Daily => "当日",
            totion_core::JournalTarget::Block => "待办",
            totion_core::JournalTarget::Habit => "习惯",
        };
        let label = main.journal_target_label(journal.target_kind, &journal.target_id);
        let preview: String = journal
            .body
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(30)
            .collect();
        let row_index = lines.len();
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", journal.date),
                Style::default().fg(Color::Cyan),
            ),
            Span::styled(format!("[{tag}] "), Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{label} "),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(preview, Style::default().fg(Color::Gray)),
        ]));
        if let Some(rect) = list_row_rect(area, row_index) {
            main.push_mouse_hit(rect, MouseAction::OpenJournal(index));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " 还没有总结。在今日选中待办/习惯后按 a 写总结。",
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(block_title(" 全部总结 · J 展开 "))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_journal_list(frame: &mut Frame, area: Rect, main: &MainState) {
    let journals = main.all_journals();
    let mut lines: Vec<Line> = Vec::new();
    for (index, journal) in journals.iter().enumerate() {
        let selected = index == main.selected_journal_index();
        let mut style = Style::default();
        if selected {
            style = selection_style();
        }
        let target = match journal.target_kind {
            totion_core::JournalTarget::Daily => "当日".to_string(),
            totion_core::JournalTarget::Block => "待办".to_string(),
            totion_core::JournalTarget::Habit => "习惯".to_string(),
        };
        let row_index = lines.len();
        lines.push(
            Line::from(vec![
                Span::styled(
                    if selected { "▶ " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("{} {} ", journal.date, journal.time),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("[{target}] {} · {}", journal.target_id, journal.author_name),
                    Style::default().fg(Color::Yellow),
                ),
            ])
            .style(style),
        );
        if let Some(rect) = list_row_rect(area, row_index) {
            main.push_mouse_hit(rect, MouseAction::OpenJournal(index));
        }
        let body = journal
            .body
            .lines()
            .take(3)
            .map(|line| format!("    {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        lines.push(Line::from(Span::styled(body, style)));
        lines.push(Line::from(""));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " 还没有总结。回到今日，选中待办/习惯后按 a 写总结。",
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " J 返回 · j/k 选择 · e 编辑 · d 删除",
        Style::default().fg(Color::DarkGray),
    )));
    frame.render_widget(
        Paragraph::new(lines)
            .block(block_title(" 全部总结 "))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_pages(frame: &mut Frame, area: Rect, main: &MainState) {
    let block = block_title(" 页面 ");
    let pages = main.visible_pages();
    let mut lines: Vec<Line> = Vec::new();
    let mut last_kind = None;
    for (index, page) in pages.iter().enumerate() {
        if last_kind != Some(page.kind) {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                format!("{} {}", page.kind.icon(), page.kind.label()),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )));
            last_kind = Some(page.kind);
        }
        let marker = if index == main.selected_page_index() {
            "▶"
        } else {
            " "
        };
        let mut style = Style::default();
        if index == main.selected_page_index() && main.focus() == Focus::Pages {
            style = selection_style().fg(Color::White);
        }
        let comments =
            main.unresolved_comment_count(totion_core::CommentTargetKind::Page, &page.id);
        let row_index = lines.len();
        lines.push(Line::from(vec![
            Span::styled(
                format!(
                    "{marker} {} {}{}",
                    page.icon,
                    page.title,
                    comment_suffix(comments)
                ),
                style,
            ),
            Span::styled(
                if page.is_daily() { " [每日]" } else { "" },
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        if let Some(rect) = list_row_rect(area, row_index) {
            main.push_mouse_hit(rect, MouseAction::SelectPage(index));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "还没有页面，按 N 新建",
            Style::default().fg(Color::DarkGray),
        )));
    }
    if main.show_all_pages() {
        lines.push(Line::from(Span::styled(
            "（a 隐藏每日页面 · J 查看总结列表）",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "（J 查看总结列表）",
            Style::default().fg(Color::DarkGray),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_blocks(frame: &mut Frame, area: Rect, main: &MainState) {
    let page_title = main
        .visible_pages()
        .get(main.selected_page_index())
        .map(|page| page.title.clone())
        .unwrap_or_else(|| "内容".to_string());
    let blocks = main.visible_blocks();
    let list_height = area.height.saturating_sub(2) as usize;
    let (block_start, block_end) =
        window_range(blocks.len(), main.selected_block_index(), list_height);
    let block = block_title(if list_height > 0 && blocks.len() > list_height {
        format!(
            " {page_title} {}-{}/{} ",
            block_start + 1,
            block_end,
            blocks.len()
        )
    } else {
        format!(" {page_title} ")
    });

    let mut lines: Vec<Line> = Vec::new();
    for (index, block_item) in blocks.iter().enumerate().take(block_end).skip(block_start) {
        let is_selected = index == main.selected_block_index();
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::raw(if is_selected { "▶ " } else { "  " }));

        match block_item.kind {
            totion_core::BlockKind::Todo | totion_core::BlockKind::CalendarEvent => {
                spans.push(Span::styled(
                    if block_item.checked { "✓ " } else { "□ " },
                    Style::default().fg(if block_item.checked {
                        Color::Green
                    } else {
                        Color::Gray
                    }),
                ));
            }
            totion_core::BlockKind::Habit => {
                spans.push(Span::styled("🌱 ", Style::default().fg(Color::Green)));
            }
            totion_core::BlockKind::Heading => {
                spans.push(Span::styled(
                    "# ",
                    Style::default().add_modifier(Modifier::BOLD),
                ));
            }
            totion_core::BlockKind::Divider => {
                spans.push(Span::styled(
                    "────────",
                    Style::default().fg(Color::DarkGray),
                ));
            }
            totion_core::BlockKind::Text => {}
        }

        if let Some(time) = &block_item.start_time {
            spans.push(Span::styled(
                format!("{time} "),
                Style::default().fg(Color::Cyan),
            ));
        }
        if let Some(section) = block_item.section {
            spans.push(Span::styled(
                format!("{}: ", section.label()),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        let text_style = if block_item.checked && block_item.kind == totion_core::BlockKind::Todo {
            Style::default().fg(Color::DarkGray)
        } else if block_item.kind == totion_core::BlockKind::Heading {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        spans.push(Span::styled(block_item.text.clone(), text_style));

        if block_item.kind == totion_core::BlockKind::Habit && block_item.streak > 0 {
            spans.push(Span::styled(
                format!("  🔥 {} 天", block_item.streak),
                Style::default().fg(Color::Yellow),
            ));
        }
        let comments =
            main.unresolved_comment_count(totion_core::CommentTargetKind::Block, &block_item.id);
        if comments > 0 {
            spans.push(Span::styled(
                comment_suffix(comments),
                Style::default().fg(Color::Magenta),
            ));
        }
        if block_item.updated_by != main.me() {
            spans.push(Span::styled(
                format!("  · {}", block_item.updated_by),
                Style::default().fg(Color::DarkGray),
            ));
        }

        let style = if is_selected && main.focus() == Focus::Blocks {
            selection_style()
        } else {
            Style::default()
        };
        let row_index = lines.len();
        lines.push(Line::from(spans).style(style));
        if let Some(rect) = list_row_rect(area, row_index) {
            main.push_mouse_hit(rect, MouseAction::SelectBlock(index));
        }
    }

    if let Some(block_item) = blocks.get(main.selected_block_index()) {
        let summaries = main.journals_for_target(totion_core::JournalTarget::Block, &block_item.id);
        if !summaries.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " 总结",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            for summary in summaries.iter().take(2) {
                lines.push(Line::from(Span::styled(
                    format!(" {} {}", summary.date, summary.time),
                    Style::default().fg(Color::DarkGray),
                )));
                for line in summary.body.lines().take(1) {
                    lines.push(Line::from(format!("   {line}")));
                }
            }
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "空页面。n 文本 · t 待办 · Enter 编辑",
            Style::default().fg(Color::DarkGray),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_today(frame: &mut Frame, area: Rect, main: &MainState) {
    if main.week_mode() {
        draw_week(frame, area, main);
        return;
    }

    let items = main.today_items();
    let habits = main.today_habits();
    let summaries = main.today_summary_entries();
    let focus = main.today_focus();
    let stat = main.today_completion();
    let view_date = main.view_date();
    let date_label = view_date.format("%Y-%m-%d").to_string();
    let relation = date_relation(view_date, main.today());
    let bar = charts::progress_bar(stat.done, stat.total, 20);
    let rate = if stat.total == 0 {
        0.0
    } else {
        stat.done as f64 / stat.total as f64 * 100.0
    };

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(6),
            Constraint::Min(5),
        ])
        .split(area);

    let mut header = HitLine::new(main, outer[0].x, outer[0].y);
    header.push(
        " ‹ ",
        Style::default().fg(Color::Cyan),
        Some(MouseAction::PrevDay),
    );
    header.push(
        format!("{date_label} {relation} "),
        Style::default().add_modifier(Modifier::BOLD),
        None,
    );
    header.push(
        " › ",
        Style::default().fg(Color::Cyan),
        Some(MouseAction::NextDay),
    );
    header.push(bar, Style::default().fg(Color::Green), None);
    header.push(
        format!("  {}/{}  {rate:.0}%", stat.done, stat.total),
        Style::default().fg(Color::Gray),
        None,
    );
    header.push(
        "   [周视图] ",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        Some(MouseAction::OpenWeek),
    );
    header.push(
        "  h/l 或 [ ] 切日期",
        Style::default().fg(Color::DarkGray),
        None,
    );
    if !main.supports_v2() {
        header.push(
            "   （服务器版本过旧）",
            Style::default().fg(Color::Yellow),
            None,
        );
    }
    frame.render_widget(Paragraph::new(header.into_line()), outer[0]);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(outer[1]);
    main.push_mouse_hit(columns[0], MouseAction::FocusToday(TodayFocus::Items));
    main.push_mouse_hit(columns[1], MouseAction::FocusToday(TodayFocus::Habits));

    let item_height = columns[0].height.saturating_sub(2) as usize;
    let (item_start, item_end) =
        window_range(items.len(), main.selected_today_index(), item_height);
    let mut item_lines: Vec<Line> = Vec::new();
    for (index, row) in items.iter().enumerate().take(item_end).skip(item_start) {
        if let TodayRow::Todo(block) | TodayRow::Event(block) = row {
            let time = block.start_time.as_deref().unwrap_or("--:--");
            let selected = focus == TodayFocus::Items && index == main.selected_today_index();
            let mut style = Style::default();
            if selected {
                style = selection_style();
            }
            let comments =
                main.unresolved_comment_count(totion_core::CommentTargetKind::Block, &block.id);
            let row_index = item_lines.len();
            item_lines.push(
                Line::from(vec![
                    Span::styled(
                        if selected { "▶ " } else { "  " },
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(format!("{time} "), Style::default().fg(Color::Cyan)),
                    Span::styled(
                        if block.checked { "✓ " } else { "○ " },
                        Style::default().fg(if block.checked {
                            Color::Green
                        } else {
                            Color::Gray
                        }),
                    ),
                    Span::styled(format!("{}{}", block.text, comment_suffix(comments)), style),
                ])
                .style(style),
            );
            if let Some(rect) = list_row_rect(columns[0], row_index) {
                main.push_mouse_hit(rect, MouseAction::SelectTodayItem(index));
                // 点击 ✓/○ 直接勾选：▶ + 时间 占 8 格。
                let checkbox = Rect {
                    x: rect.x.saturating_add(8),
                    y: rect.y,
                    width: 2,
                    height: 1,
                };
                main.push_mouse_hit(checkbox, MouseAction::ToggleTodayItem(index));
            }
        }
    }
    if item_lines.is_empty() {
        item_lines.push(Line::from(Span::styled(
            " 这一天还没有待办，按 t 新建",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let item_title = if item_height > 0 && items.len() > item_height {
        format!(
            " 今日待办 [Tab] {}-{}/{} ",
            item_start + 1,
            item_end,
            items.len()
        )
    } else {
        " 今日待办 [Tab] ".to_string()
    };
    frame.render_widget(
        Paragraph::new(item_lines)
            .block(block_title_focused(item_title, focus == TodayFocus::Items))
            .wrap(Wrap { trim: false }),
        columns[0],
    );

    let habit_height = columns[1].height.saturating_sub(2) as usize;
    let (habit_start, habit_end) = window_range(
        habits.len(),
        main.selected_today_habit_index(),
        habit_height,
    );
    let mut habit_lines: Vec<Line> = Vec::new();
    for (index, (habit, stats)) in habits.iter().enumerate().take(habit_end).skip(habit_start) {
        let selected = focus == TodayFocus::Habits && index == main.selected_today_habit_index();
        let mut style = Style::default();
        if selected {
            style = selection_style();
        }
        let row_index = habit_lines.len();
        habit_lines.push(
            Line::from(vec![
                Span::styled(
                    if selected { "▶ " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    if stats.checked_today { "✓ " } else { "○ " },
                    Style::default().fg(if stats.checked_today {
                        Color::Green
                    } else {
                        Color::Gray
                    }),
                ),
                Span::styled(
                    format!(
                        "{} {}  🔥{} · {}",
                        habit.icon,
                        habit.title,
                        stats.current_streak,
                        weekly_label(*stats)
                    ),
                    style,
                ),
            ])
            .style(style),
        );
        if let Some(rect) = list_row_rect(columns[1], row_index) {
            main.push_mouse_hit(rect, MouseAction::SelectTodayHabit(index));
            let checkbox = Rect {
                x: rect.x.saturating_add(2),
                y: rect.y,
                width: 2,
                height: 1,
            };
            main.push_mouse_hit(checkbox, MouseAction::ToggleTodayHabit(index));
        }
    }
    if habit_lines.is_empty() {
        habit_lines.push(Line::from(Span::styled(
            " 去习惯界面（4）新建习惯",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let habit_title = if habit_height > 0 && habits.len() > habit_height {
        format!(
            " 今日习惯 [Tab] {}-{}/{} ",
            habit_start + 1,
            habit_end,
            habits.len()
        )
    } else {
        " 今日习惯 [Tab] ".to_string()
    };
    frame.render_widget(
        Paragraph::new(habit_lines)
            .block(block_title_focused(
                habit_title,
                focus == TodayFocus::Habits,
            ))
            .wrap(Wrap { trim: false }),
        columns[1],
    );

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(outer[2]);

    if !summaries.is_empty() {
        main.push_mouse_hit(bottom[0], MouseAction::FocusToday(TodayFocus::Journal));
        let mut summary_lines: Vec<Line> = Vec::new();
        for summary in summaries.iter().take(3) {
            summary_lines.push(Line::from(vec![
                Span::styled(
                    format!(" {} ", summary.time),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("· {}", summary.author_name),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            for line in summary.body.lines().take(2) {
                summary_lines.push(Line::from(format!(" {line}")));
            }
        }
        frame.render_widget(
            Paragraph::new(summary_lines)
                .block(block_title_focused(
                    " 总结 [Tab] ",
                    focus == TodayFocus::Journal,
                ))
                .wrap(Wrap { trim: false }),
            bottom[0],
        );
    }

    let comments = selected_comments(main);
    if !comments.is_empty() {
        let mut comment_lines: Vec<Line> = Vec::new();
        for comment in comments.iter().take(3) {
            comment_lines.push(Line::from(vec![
                Span::styled(
                    format!(" {}：", comment.author_name),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    comment.body.lines().next().unwrap_or("").to_string(),
                    Style::default().fg(Color::Gray),
                ),
            ]));
        }
        frame.render_widget(
            Paragraph::new(comment_lines)
                .block(block_title(" 评论 "))
                .wrap(Wrap { trim: false }),
            bottom[1],
        );
    }
}

fn selected_comments(main: &MainState) -> Vec<totion_core::Comment> {
    let Some((kind, id)) = main.current_summary_target() else {
        return Vec::new();
    };
    let comments = match kind {
        totion_core::JournalTarget::Block => {
            main.comments_for(totion_core::CommentTargetKind::Block, &id)
        }
        totion_core::JournalTarget::Habit => {
            main.comments_for(totion_core::CommentTargetKind::Habit, &id)
        }
        totion_core::JournalTarget::Daily => Vec::new(),
    };
    comments.into_iter().filter(|c| !c.resolved).collect()
}

fn date_relation(view_date: NaiveDate, today: NaiveDate) -> String {
    let diff = (today - view_date).num_days();
    match diff {
        0 => "今天".to_string(),
        1 => "昨天".to_string(),
        2 => "前天".to_string(),
        n if n > 0 => format!("{n} 天前"),
        _ => "未来".to_string(),
    }
}

fn draw_week(frame: &mut Frame, area: Rect, main: &MainState) {
    let overview = main.week_overview();
    let selected = main.week_selected();
    let budget = area.height.saturating_sub(4) as usize;
    let heights: Vec<usize> = overview
        .iter()
        .map(|(_, _, blocks)| 1 + blocks.len().max(1) + 1)
        .collect();
    let start_day = scroll_start(&heights, selected, budget);
    let mut lines: Vec<Line> = Vec::new();
    let mut rendered_end = start_day;
    for (index, (date, stat, blocks)) in overview.iter().enumerate().skip(start_day) {
        if lines.len() >= budget && index != selected {
            break;
        }
        let is_selected = index == selected;
        let mut style = Style::default();
        if is_selected {
            style = selection_style();
        }
        let rate = if stat.total == 0 {
            0.0
        } else {
            stat.done as f64 / stat.total as f64 * 100.0
        };
        let header_row = lines.len();
        lines.push(
            Line::from(vec![
                Span::styled(
                    if is_selected { "▶ " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("{} {}  ", date.format("%m-%d"), weekday_label(*date)),
                    style,
                ),
                Span::styled(
                    format!("完成 {}/{}  {rate:.0}%", stat.done, stat.total),
                    Style::default().fg(Color::Green),
                ),
            ])
            .style(style),
        );
        if let Some(rect) = list_row_rect(area, header_row) {
            main.push_mouse_hit(rect, MouseAction::WeekSelect(index));
        }
        for block in blocks {
            let time = match block.start_time.as_deref() {
                Some(value) if !value.is_empty() => format!("{value} "),
                _ => "      ".to_string(),
            };
            let item_row = lines.len();
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(time, Style::default().fg(Color::Cyan)),
                Span::styled(
                    if block.checked { "✓ " } else { "○ " },
                    Style::default().fg(if block.checked {
                        Color::Green
                    } else {
                        Color::Gray
                    }),
                ),
                Span::styled(
                    block.text.clone(),
                    if block.checked {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default()
                    },
                ),
            ]));
            if let Some(rect) = list_row_rect(area, item_row) {
                main.push_mouse_hit(rect, MouseAction::WeekSelect(index));
            }
        }
        if blocks.is_empty() {
            lines.push(Line::from(Span::styled(
                "      （这一天没有待办）",
                Style::default().fg(Color::DarkGray),
            )));
        }
        lines.push(Line::from(""));
        rendered_end = index + 1;
    }
    lines.push(Line::from(Span::styled(
        " j/k 或点击 选日 · h/l 前后一天 · Enter 查看该天 · w/Esc 返回",
        Style::default().fg(Color::DarkGray),
    )));
    let start = main.week_end() - chrono::Duration::days(6);
    let title = if start_day > 0 || rendered_end < overview.len() {
        format!(
            " 周视图 {} ~ {} · {}-{}/7 ",
            start.format("%m-%d"),
            main.week_end().format("%m-%d"),
            start_day + 1,
            rendered_end.max(start_day + 1)
        )
    } else {
        format!(
            " 周视图 {} ~ {} ",
            start.format("%m-%d"),
            main.week_end().format("%m-%d")
        )
    };
    frame.render_widget(
        Paragraph::new(lines)
            .block(block_title(title))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn weekly_label(stats: HabitStats) -> String {
    match stats.weekly_target {
        Some(target) => format!("本周 {}/{}", stats.weekly_done, target),
        None => format!("本周 {} 次", stats.weekly_done),
    }
}

fn weekday_label(date: NaiveDate) -> &'static str {
    match date.weekday() {
        chrono::Weekday::Mon => "周一",
        chrono::Weekday::Tue => "周二",
        chrono::Weekday::Wed => "周三",
        chrono::Weekday::Thu => "周四",
        chrono::Weekday::Fri => "周五",
        chrono::Weekday::Sat => "周六",
        chrono::Weekday::Sun => "周日",
    }
}

fn draw_calendar(frame: &mut Frame, area: Rect, main: &MainState) {
    let month = main.calendar_month();
    let range = main.selected_range();
    let block = block_title(" 日历 ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 14 || inner.height < 9 {
        return;
    }

    let mark = month_mark(month.month());
    let month_stat = main.month_block_stats(month);
    let banner = if month_stat.total == 0 {
        format!(
            "{mark}    {} 年 {} 月    {mark}",
            month.year(),
            month.month()
        )
    } else {
        format!(
            "{mark}   {} 年 {} 月   ·   待办 {} · 完成 {}",
            month.year(),
            month.month(),
            month_stat.total,
            month_stat.done
        )
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            banner,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center),
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        },
    );

    let arrow_left = Rect {
        x: inner.x + 1,
        y: inner.y,
        width: 3,
        height: 1,
    };
    let arrow_right = Rect {
        x: inner.x + inner.width.saturating_sub(4),
        y: inner.y,
        width: 4,
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(" ‹ ").style(Style::default().fg(Color::Cyan)),
        arrow_left,
    );
    frame.render_widget(
        Paragraph::new(" › ")
            .alignment(Alignment::Right)
            .style(Style::default().fg(Color::Cyan)),
        arrow_right,
    );
    main.push_mouse_hit(arrow_left, MouseAction::PrevMonth);
    main.push_mouse_hit(arrow_right, MouseAction::NextMonth);

    let weekday_labels = ["一", "二", "三", "四", "五", "六", "日"];
    let cell_w = inner.width / 7;
    for (index, label) in weekday_labels.iter().enumerate() {
        frame.render_widget(
            Paragraph::new(*label)
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray)),
            Rect {
                x: inner.x + cell_w * index as u16,
                y: inner.y + 1,
                width: cell_w,
                height: 1,
            },
        );
    }

    let grid_y = inner.y + 2;
    let grid_h = inner.height.saturating_sub(3);
    let cell_h = grid_h / 6;
    if cell_w == 0 || cell_h == 0 {
        return;
    }

    let offset = month.weekday().num_days_from_monday() as usize;
    let days = days_in_month(month);
    let today = Local::now().date_naive();
    let selected = main.selected_day();

    let mut cells = main.calendar_cells.borrow_mut();
    cells.clear();

    for week in 0..6u16 {
        for weekday in 0..7u16 {
            let index = week as usize * 7 + weekday as usize;
            if index < offset || index >= offset + days as usize {
                continue;
            }
            let day = (index - offset + 1) as u32;
            let Some(date) = NaiveDate::from_ymd_opt(month.year(), month.month(), day) else {
                continue;
            };
            let rect = Rect {
                x: inner.x + weekday * cell_w,
                y: grid_y + week * cell_h,
                width: cell_w,
                height: cell_h,
            };
            cells.push((rect, date));

            let in_range = range
                .map(|(start, end)| date >= start && date <= end)
                .unwrap_or(false);
            let (background, border) = if date == selected {
                (Color::Rgb(0, 70, 80), Color::Cyan)
            } else if date == today {
                (Color::Rgb(16, 60, 36), Color::Green)
            } else if in_range {
                (Color::Rgb(30, 40, 48), Color::Rgb(70, 105, 130))
            } else if date < today {
                (Color::Rgb(18, 22, 27), Color::Rgb(40, 46, 54))
            } else {
                (Color::Rgb(26, 32, 40), Color::Rgb(55, 68, 82))
            };

            let cell_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border))
                .style(Style::default().bg(background));
            let cell_inner = cell_block.inner(rect);
            frame.render_widget(cell_block, rect);

            let day_color = if date == selected {
                Color::White
            } else if date == today {
                Color::Green
            } else if date < today {
                Color::DarkGray
            } else {
                Color::Gray
            };
            let mut lines: Vec<Line> = Vec::new();
            lines.push(Line::from(Span::styled(
                format!("{day}"),
                Style::default().fg(day_color).add_modifier(Modifier::BOLD),
            )));

            if cell_inner.height > 1 {
                let max_items = (cell_inner.height as usize).saturating_sub(1);
                for block in main.calendar_blocks(date).iter().take(max_items) {
                    let text =
                        truncate_chars(&block.text, cell_inner.width.saturating_sub(3) as usize);
                    lines.push(Line::from(vec![
                        Span::styled(
                            if block.checked { "✓" } else { "○" },
                            Style::default().fg(if block.checked {
                                Color::Green
                            } else {
                                Color::Gray
                            }),
                        ),
                        Span::raw(format!(" {text}")),
                    ]));
                }
            }

            frame.render_widget(
                Paragraph::new(lines).style(Style::default().bg(background)),
                cell_inner,
            );
        }
    }

    let range_hint = match range {
        Some((start, end)) if start != end => {
            format!(
                " · 已选 {} ~ {}",
                start.format("%m-%d"),
                end.format("%m-%d")
            )
        }
        Some((start, _)) if main.range_is_active() => {
            format!(" · 起点 {}", start.format("%m-%d"))
        }
        _ => String::new(),
    };
    let hint = format!(
        " {} · Enter 议程 · t 新建待办 · v 框选{}",
        selected.format("%Y-%m-%d"),
        range_hint
    );
    frame.render_widget(
        Paragraph::new(hint).style(Style::default().fg(Color::DarkGray)),
        Rect {
            x: inner.x,
            y: inner.y + inner.height - 1,
            width: inner.width,
            height: 1,
        },
    );
}

fn month_mark(month: u32) -> &'static str {
    match month {
        1 => "❄️",
        2 => "🌨️",
        3 => "🌱",
        4 => "🌸",
        5 => "🌿",
        6 => "☀️",
        7 => "🏖️",
        8 => "🌻",
        9 => "🍂",
        10 => "🎃",
        11 => "🍁",
        12 => "🎄",
        _ => "·",
    }
}

fn truncate_chars(value: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let mut output: String = value.chars().take(max).collect();
    if value.chars().count() > max {
        output.pop();
        output.push('…');
    }
    output
}

fn draw_habits(frame: &mut Frame, area: Rect, main: &MainState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(36), Constraint::Percentage(64)])
        .split(area);

    let habits = main.visible_habits();
    let list_height = columns[0].height.saturating_sub(2) as usize;
    let (habit_start, habit_end) =
        window_range(habits.len(), main.selected_habit_index(), list_height);
    let mut lines: Vec<Line> = Vec::new();
    for (index, habit) in habits.iter().enumerate().take(habit_end).skip(habit_start) {
        let stats = main.habit_stats(habit);
        let style = if index == main.selected_habit_index() {
            selection_style()
        } else {
            Style::default()
        };
        let mark = if stats.checked_today { "[x]" } else { "[ ]" };
        let comments =
            main.unresolved_comment_count(totion_core::CommentTargetKind::Habit, &habit.id);
        let row_index = lines.len();
        lines.push(Line::from(Span::styled(
            format!(
                " {mark} {} {}  🔥{} {}{}",
                habit.icon,
                habit.title,
                stats.current_streak,
                weekly_label(stats),
                comment_suffix(comments)
            ),
            style,
        )));
        if let Some(rect) = list_row_rect(columns[0], row_index) {
            main.push_mouse_hit(rect, MouseAction::SelectHabit(index));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " 还没有习惯，按 n 创建第一个",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let list_title = if list_height > 0 && habits.len() > list_height {
        format!(" 习惯 {}-{}/{} ", habit_start + 1, habit_end, habits.len())
    } else {
        " 习惯 ".to_string()
    };
    frame.render_widget(
        Paragraph::new(lines)
            .block(block_title(list_title))
            .wrap(Wrap { trim: false }),
        columns[0],
    );

    let detail = habits.get(main.selected_habit_index());
    let mut detail_lines: Vec<Line> = Vec::new();
    if let Some(habit) = detail {
        let stats = main.habit_stats(habit);
        detail_lines.push(Line::from(Span::styled(
            format!(" {} {}", habit.icon, habit.title),
            Style::default().add_modifier(Modifier::BOLD),
        )));
        detail_lines.push(Line::from(vec![
            Span::styled(" 频率: ", Style::default().fg(Color::DarkGray)),
            Span::raw(habit.frequency.label()),
            Span::styled("   连续: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{} 天", stats.current_streak)),
            Span::styled("   最长: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{} 天", stats.longest_streak)),
        ]));
        detail_lines.push(Line::from(vec![
            Span::styled(" 30 天完成率: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                charts::progress_bar((stats.last_30_rate * 100.0) as u32, 100, 16),
                Style::default().fg(Color::Green),
            ),
            Span::styled(
                format!(" {:.0}%", stats.last_30_rate * 100.0),
                Style::default().fg(Color::Gray),
            ),
        ]));
        detail_lines.push(Line::from(""));
        detail_lines.push(Line::from(Span::styled(
            " 最近 12 周",
            Style::default().fg(Color::DarkGray),
        )));
        let weeks = ((columns[1].width.saturating_sub(10)) / 2).clamp(8, 52) as u32;
        let grid = main.heatmap_grid(habit, weeks);
        detail_lines.extend(charts::heatmap_lines(&grid));
        if !habit.note.trim().is_empty() {
            detail_lines.push(Line::from(""));
            detail_lines.push(Line::from(format!(" 备注: {}", habit.note)));
        }
        let summaries = main.journals_for_target(totion_core::JournalTarget::Habit, &habit.id);
        if !summaries.is_empty() {
            detail_lines.push(Line::from(""));
            detail_lines.push(Line::from(Span::styled(
                " 最近的总结",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            for summary in summaries.iter().take(3) {
                detail_lines.push(Line::from(Span::styled(
                    format!(" {} {}", summary.date, summary.time),
                    Style::default().fg(Color::DarkGray),
                )));
                for line in summary.body.lines().take(1) {
                    detail_lines.push(Line::from(format!("  {line}")));
                }
            }
        }
    } else {
        detail_lines.push(Line::from(Span::styled(
            " 选择一个习惯查看详情",
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(detail_lines)
            .block(block_title(" 详情 "))
            .wrap(Wrap { trim: false }),
        columns[1],
    );
}

fn draw_stats(frame: &mut Frame, area: Rect, main: &MainState) {
    let series = main.completion_series(main.stats_days());
    let done_values = charts::done_values(&series);
    let rate_values = charts::rate_values(&series);
    let stat = main.today_completion();
    let rate = if stat.total == 0 {
        0.0
    } else {
        stat.done as f64 / stat.total as f64
    };

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(10),
            Constraint::Min(4),
        ])
        .split(area);

    frame.render_widget(
        Gauge::default()
            .block(block_title(" 今日完成率 "))
            .gauge_style(Style::default().fg(Color::Green).bg(Color::Black))
            .ratio(rate.clamp(0.0, 1.0))
            .label(format!(
                "{}/{}  {:.0}%",
                stat.done,
                stat.total,
                rate * 100.0
            )),
        sections[0],
    );

    frame.render_widget(
        Sparkline::default()
            .block(block_title(format!(
                " 最近 {} 天完成数 ",
                main.stats_days()
            )))
            .data(done_values)
            .style(Style::default().fg(Color::Cyan)),
        sections[1],
    );

    frame.render_widget(
        Sparkline::default()
            .block(block_title(format!(
                " 最近 {} 天完成率（%） ",
                main.stats_days()
            )))
            .data(rate_values)
            .style(Style::default().fg(Color::Green)),
        sections[2],
    );

    let weeks = ((sections[3].width.saturating_sub(10)) / 2).clamp(8, 52) as u32;
    let grid = main.aggregate_heatmap(weeks);
    let has_checkins = main.checkins.iter().any(|checkin| !checkin.deleted);
    let mut heatmap_lines: Vec<Line> = Vec::new();
    if !has_checkins {
        heatmap_lines.push(Line::from(Span::styled(
            " 还没有打卡记录。去习惯界面（4）按 Space 打卡，这里会显示热力图。",
            Style::default().fg(Color::DarkGray),
        )));
    }
    heatmap_lines.extend(charts::heatmap_lines(&grid));
    frame.render_widget(
        Paragraph::new(heatmap_lines)
            .block(block_title(format!(" 最近 {weeks} 周打卡热力图 ")))
            .wrap(Wrap { trim: false }),
        sections[3],
    );

    let mut habit_lines: Vec<Line> = Vec::new();
    for habit in main.visible_habits() {
        let stats = main.habit_stats(&habit);
        habit_lines.push(Line::from(format!(
            " {} {:<14} 连续 {:>3} 天  30天 {:>3.0}%  {}",
            habit.icon,
            habit.title,
            stats.current_streak,
            stats.last_30_rate * 100.0,
            weekly_label(stats)
        )));
    }
    if habit_lines.is_empty() {
        habit_lines.push(Line::from(Span::styled(
            " 还没有习惯数据",
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(habit_lines)
            .block(block_title(" 习惯统计 "))
            .wrap(Wrap { trim: false }),
        sections[4],
    );
}

fn draw_modal(frame: &mut Frame, main: &MainState, modal: &Modal) {
    match modal {
        Modal::EventForm(form) => draw_event_form(frame, form),
        Modal::HabitForm(form) => draw_habit_form(frame, form),
        Modal::Comments(panel) => draw_comments(frame, main, panel),
        Modal::Search(state) => draw_search(frame, main, state),
        Modal::Agenda(panel) => draw_agenda(frame, main, panel),
        Modal::Invites(panel) => draw_invites(frame, panel),
        Modal::Members(panel) => draw_members(frame, main, panel),
        Modal::Confirm(confirm) => draw_confirm(frame, confirm),
        Modal::Help(panel) => draw_help(frame, panel),
    }
}

fn draw_members(frame: &mut Frame, main: &MainState, panel: &MemberPanel) {
    let area = centered_rect(56, 64, frame.area());
    frame.render_widget(Clear, area);
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        " 选择一个成员，切换到对方的 1-6 界面",
        Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    let mut entries: Vec<(String, bool, &'static str)> =
        vec![("共享空间（全部数据）".to_string(), true, "")];
    for user in main.users() {
        let is_me = user.name == main.me();
        entries.push((user.name.clone(), false, if is_me { "（我）" } else { "" }));
    }
    for (index, (label, shared, suffix)) in entries.iter().enumerate() {
        let selected = index == panel.selected;
        let style = if selected {
            selection_style()
        } else {
            Style::default()
        };
        let current = if selected { "▶ " } else { "  " };
        let tag = if *shared { " [共享] " } else { " [成员] " };
        lines.push(Line::from(vec![
            Span::styled(current, Style::default().fg(Color::Cyan)),
            Span::styled(tag, Style::default().fg(Color::Yellow)),
            Span::styled(format!("{label}{suffix}"), style),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " 他人视角只读，可以评论；Enter 切换，Esc 关闭",
        Style::default().fg(Color::DarkGray),
    )));
    frame.render_widget(
        Paragraph::new(lines)
            .block(block_title(" 成员视角 · m "))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_confirm(frame: &mut Frame, confirm: &crate::app::ConfirmState) {
    let area = centered_rect(62, 28, frame.area());
    frame.render_widget(Clear, area);
    let lines = vec![
        Line::from(Span::styled(
            format!(" 确定要删除 {} 吗？", confirm.label),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            " 删除后所有成员都会看不到它，且无法撤销。",
            Style::default().fg(Color::Gray),
        )),
        Line::from(""),
        Line::from(" 请输入 delete 并回车确认："),
        Line::from(Span::styled(
            format!(" > {}█", confirm.input),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " Esc 取消",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).block(block_title(" 删除确认 ")), area);
}

fn draw_invites(frame: &mut Frame, panel: &InvitePanel) {
    let area = centered_rect(70, 64, frame.area());
    frame.render_widget(Clear, area);
    let now = Utc::now();
    let mut lines: Vec<Line> = Vec::new();
    for (index, invite) in panel.invites.iter().enumerate() {
        let selected = index == panel.selected;
        let style = if selected {
            selection_style()
        } else {
            Style::default()
        };
        let status = invite.status_label(now);
        let uses = match invite.max_uses {
            Some(max) => format!("{}/{}", invite.used_count, max),
            None => format!("{} 次", invite.used_count),
        };
        let expires = match invite.expires_at {
            Some(value) => value
                .with_timezone(&Local)
                .format("%m-%d %H:%M")
                .to_string(),
            None => "不过期".to_string(),
        };
        lines.push(
            Line::from(vec![
                Span::styled(
                    if selected { "▶ " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("{} ", invite.code),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{status} "),
                    Style::default().fg(if status == "有效" {
                        Color::Green
                    } else {
                        Color::Gray
                    }),
                ),
                Span::styled(
                    format!("已用 {uses} · {expires}"),
                    Style::default().fg(Color::DarkGray),
                ),
            ])
            .style(style),
        );
        lines.push(Line::from(Span::styled(
            format!("    {} · {}", invite.note, invite.created_by),
            style.fg(Color::Gray),
        )));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " 还没有邀请码，按 g 生成一个",
            Style::default().fg(Color::DarkGray),
        )));
    }
    if let Some(message) = &panel.message {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            message.clone(),
            Style::default().fg(Color::Yellow),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " g 生成新码 · x 撤销/恢复 · j/k 选择 · Esc 关闭",
        Style::default().fg(Color::DarkGray),
    )));
    frame.render_widget(Paragraph::new(lines).block(block_title(" 邀请码 ")), area);
}

fn draw_agenda(frame: &mut Frame, main: &MainState, panel: &AgendaPanel) {
    let area = centered_rect(66, 62, frame.area());
    frame.render_widget(Clear, area);
    let blocks = main.calendar_blocks(panel.date);
    let budget = area.height.saturating_sub(4) as usize;
    let (start, end) = window_range(blocks.len(), panel.selected, budget);
    let mut lines: Vec<Line> = Vec::new();
    for (index, block) in blocks.iter().enumerate().take(end).skip(start) {
        let style = if index == panel.selected {
            selection_style()
        } else {
            Style::default()
        };
        let time = block.start_time.as_deref().unwrap_or("--:--");
        let comments =
            main.unresolved_comment_count(totion_core::CommentTargetKind::Block, &block.id);
        lines.push(
            Line::from(vec![
                Span::styled(format!(" {time} "), Style::default().fg(Color::Cyan)),
                Span::styled(
                    if block.checked { "✓ " } else { "○ " },
                    Style::default().fg(if block.checked {
                        Color::Green
                    } else {
                        Color::Gray
                    }),
                ),
                Span::styled(format!("{}{}", block.text, comment_suffix(comments)), style),
            ])
            .style(style),
        );
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " 这天没有安排，按 t 新建待办",
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " j/k 选择 · Space 勾选 · Enter 编辑 · d 删除 · t 新建 · Esc 关闭",
        Style::default().fg(Color::DarkGray),
    )));
    let title = if start > 0 || end < blocks.len() {
        format!(
            " {} 议程 {}-{}/{} ",
            panel.date.format("%Y-%m-%d"),
            start + 1,
            end,
            blocks.len()
        )
    } else {
        format!(" {} 议程 ", panel.date.format("%Y-%m-%d"))
    };
    frame.render_widget(
        Paragraph::new(lines)
            .block(block_title(title))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_inbox(frame: &mut Frame, area: Rect, main: &MainState) {
    let comments = main.inbox_comments();
    let unresolved = comments.iter().filter(|comment| !comment.resolved).count();
    let budget = area.height.saturating_sub(4) as usize;
    let heights: Vec<usize> = comments
        .iter()
        .map(|comment| 1 + comment.body.lines().count() + 1)
        .collect();
    let start = scroll_start(&heights, main.selected_inbox_index(), budget);

    let mut blocks: Vec<Vec<Line>> = Vec::new();
    for (index, comment) in comments.iter().enumerate() {
        let selected = index == main.selected_inbox_index();
        let style = if selected {
            selection_style()
        } else {
            Style::default()
        };
        let target = main.comment_target_label(comment.target_kind, &comment.target_id);
        let mut block: Vec<Line> = Vec::new();
        block.push(
            Line::from(vec![
                Span::styled(
                    if selected { "▶ " } else { "  " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    if comment.resolved {
                        "[已解决] "
                    } else {
                        "[未解决] "
                    },
                    Style::default().fg(if comment.resolved {
                        Color::Green
                    } else {
                        Color::Yellow
                    }),
                ),
                Span::styled(
                    format!("{} ", comment.author_name),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(
                        "{} ",
                        comment
                            .updated_at
                            .with_timezone(&Local)
                            .format("%m-%d %H:%M")
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(target, Style::default().fg(Color::Cyan)),
            ])
            .style(style),
        );
        for line in comment.body.lines() {
            block.push(Line::from(Span::styled(
                format!("    {line}"),
                style.fg(Color::Gray),
            )));
        }
        block.push(Line::from(""));
        blocks.push(block);
    }

    let mut lines: Vec<Line> = Vec::new();
    let mut rendered_end = start;
    for (index, block) in blocks.iter().enumerate().skip(start) {
        if lines.len() >= budget {
            break;
        }
        let row_index = lines.len();
        lines.extend(block.iter().cloned());
        if let Some(rect) = list_row_rect(area, row_index) {
            main.push_mouse_hit(rect, MouseAction::SelectInbox(index));
        }
        rendered_end = index + 1;
    }
    if comments.is_empty() {
        lines.push(Line::from(Span::styled(
            " 还没有留言。选中一个页面/内容块/习惯后按 C 发表评论。",
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines.push(Line::from(Span::styled(
        " j/k 选择 · Enter 跳转 · a 回复 · r 解决/取消 · e 编辑 · d 删除",
        Style::default().fg(Color::DarkGray),
    )));
    let title = if start > 0 || rendered_end < comments.len() {
        format!(
            " 留言 · {}-{}/{} · {unresolved} 条未解决 ",
            start + 1,
            rendered_end.max(start + 1),
            comments.len()
        )
    } else {
        format!(" 留言 · {unresolved} 条未解决 ")
    };
    frame.render_widget(
        Paragraph::new(lines)
            .block(block_title(title))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_event_form(frame: &mut Frame, form: &EventForm) {
    let area = centered_rect(64, 44, frame.area());
    frame.render_widget(Clear, area);
    let labels = ["标题", "日期 (YYYY-MM-DD)", "开始 (HH:MM)", "结束 (HH:MM)"];
    let values = [&form.title, &form.date, &form.start_time, &form.end_time];
    let mut lines = Vec::new();
    for (index, (label, value)) in labels.iter().zip(values.iter()).enumerate() {
        let active = index == form.field;
        let mut text = format!("{}{}: {}", if active { "▶ " } else { "  " }, label, value);
        if active {
            text.push('█');
        }
        lines.push(Line::from(Span::styled(
            text,
            if active {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        )));
    }
    if let Some(end) = &form.range_end {
        lines.push(Line::from(Span::styled(
            format!("  批量范围: {} ~ {end}", form.date),
            Style::default().fg(Color::Yellow),
        )));
    }
    lines.push(Line::from(""));
    if let Some(error) = &form.error {
        lines.push(Line::from(Span::styled(
            error.clone(),
            Style::default().fg(Color::Red),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines).block(block_title(format!(
            " 新建{} ",
            if form.kind == totion_core::BlockKind::Todo {
                "待办"
            } else {
                "日程"
            }
        ))),
        area,
    );
}

fn draw_habit_form(frame: &mut Frame, form: &HabitForm) {
    let area = centered_rect(64, 40, frame.area());
    frame.render_widget(Clear, area);
    let mut lines = Vec::new();
    for (index, label, value) in [
        (0, "名称", form.title.as_str()),
        (1, "备注", form.note.as_str()),
    ] {
        let active = index == form.field;
        let mut text = format!("{}{}: {}", if active { "▶ " } else { "  " }, label, value);
        if active {
            text.push('█');
        }
        lines.push(Line::from(Span::styled(
            text,
            if active {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        )));
    }
    lines.push(Line::from(format!(
        "  频率: {}（Ctrl+F 切换）",
        form.frequency.label()
    )));
    if let Some(end) = &form.range_end {
        lines.push(Line::from(Span::styled(
            format!("  批量范围: {} ~ {end}（创建后逐日打卡）", form.date),
            Style::default().fg(Color::Yellow),
        )));
    }
    lines.push(Line::from(""));
    if let Some(error) = &form.error {
        lines.push(Line::from(Span::styled(
            error.clone(),
            Style::default().fg(Color::Red),
        )));
    }
    frame.render_widget(Paragraph::new(lines).block(block_title(" 新建习惯 ")), area);
}

fn draw_comments(frame: &mut Frame, main: &MainState, panel: &CommentPanel) {
    let area = Rect {
        x: frame.area().width * 55 / 100,
        y: 0,
        width: frame.area().width * 45 / 100,
        height: frame.area().height,
    };
    frame.render_widget(Clear, area);

    let comments = main.comments_for(panel.target_kind, &panel.target_id);
    let mut lines: Vec<Line> = Vec::new();
    for (index, comment) in comments.iter().enumerate() {
        let style = if index == panel.selected {
            selection_style()
        } else {
            Style::default()
        };
        let mark = if comment.resolved { "[✓]" } else { "[ ]" };
        lines.push(Line::from(Span::styled(
            format!(
                " {mark} {} {}",
                comment.author_name,
                comment
                    .created_at
                    .with_timezone(&Local)
                    .format("%m-%d %H:%M")
            ),
            style.fg(Color::Cyan),
        )));
        let body = comment
            .body
            .lines()
            .map(|line| format!("    {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        lines.push(Line::from(Span::styled(
            body,
            if index == panel.selected {
                selection_style()
            } else {
                Style::default()
            },
        )));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " 还没有评论，按 a 添加",
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(block_title(format!(" 评论 · {} ", panel.label)))
            .wrap(Wrap { trim: false }),
        area,
    );
}

/// 把命中词标成黄色加粗；大小写不敏感只在不会改变字节长度时启用。
fn highlight_spans(text: &str, query: &str, style: Style) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::styled(text.to_string(), style)];
    }
    let lower = text.to_lowercase();
    let needle = query.to_lowercase();
    if needle.is_empty() || lower.len() != text.len() {
        return vec![Span::styled(text.to_string(), style)];
    }
    let mut spans = Vec::new();
    let mut cursor = 0usize;
    while let Some(offset) = lower[cursor..].find(&needle) {
        let start = cursor + offset;
        let end = start + needle.len();
        if start > cursor {
            spans.push(Span::styled(text[cursor..start].to_string(), style));
        }
        spans.push(Span::styled(
            text[start..end].to_string(),
            style.fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
        cursor = end;
    }
    if cursor < text.len() {
        spans.push(Span::styled(text[cursor..].to_string(), style));
    }
    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), style));
    }
    spans
}

fn draw_search(frame: &mut Frame, main: &MainState, state: &SearchState) {
    let area = centered_rect(72, 60, frame.area());
    frame.render_widget(Clear, area);
    let mut lines = vec![Line::from(Span::styled(
        format!(" 搜索: {}█", state.query),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))];
    lines.push(Line::from(""));
    let budget = area.height.saturating_sub(4) as usize;
    let (start, end) = window_range(state.results.len(), state.selected, budget);
    for (index, hit) in state.results.iter().enumerate().take(end).skip(start) {
        let style = if index == state.selected {
            selection_style()
        } else {
            Style::default()
        };
        let tag = match hit {
            crate::app::SearchHit::Page(_) => " [页面] ",
            crate::app::SearchHit::Block { .. } => " [内容块] ",
            crate::app::SearchHit::Habit(_) => " [习惯] ",
        };
        let mut spans = vec![Span::styled(tag, style.fg(Color::Cyan))];
        spans.extend(highlight_spans(
            &main.search_hit_label(hit),
            &state.query,
            style,
        ));
        lines.push(Line::from(spans).style(style));
    }
    if state.results.is_empty() && !state.query.is_empty() {
        lines.push(Line::from(Span::styled(
            " 没有匹配结果",
            Style::default().fg(Color::DarkGray),
        )));
    }
    let title = if start > 0 || end < state.results.len() {
        format!(
            " 搜索（{}-{}/{} · Enter 跳转 · Esc 关闭） ",
            start + 1,
            end,
            state.results.len()
        )
    } else {
        format!(
            " 搜索（{} 条 · Enter 跳转 · Esc 关闭） ",
            state.results.len()
        )
    };
    frame.render_widget(
        Paragraph::new(lines)
            .block(block_title(title))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_help(frame: &mut Frame, panel: &HelpPanel) {
    let screen = frame.area();
    let width = screen
        .width
        .saturating_sub(8)
        .clamp(40, 82)
        .min(screen.width);
    let height = screen
        .height
        .saturating_sub(4)
        .clamp(10, 30)
        .min(screen.height);
    let area = Rect {
        x: screen.x + (screen.width.saturating_sub(width)) / 2,
        y: screen.y + (screen.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, area);
    let lines = vec![
        Line::from(Span::styled(
            " 视图",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" 1 今日   2 工作区   3 日历   4 习惯   5 统计   6 留言"),
        Line::from(""),
        Line::from(Span::styled(
            " 今日（Tab 切换面板）",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" Tab 待办/习惯/总结   j/k 移动   Enter 编辑"),
        Line::from(" Space 勾选或打卡   a 写总结   t 新建待办"),
        Line::from(" h/l 或 [ ] 前一天/后一天   w 周视图   d 删除"),
        Line::from(" 周视图：j/k 选日 · h/l 平移一天 · Enter 查看该天"),
        Line::from(""),
        Line::from(Span::styled(
            " 工作区",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" Tab 页面/内容   j/k 移动   Enter 编辑   d 删除"),
        Line::from(" n 文本   t 待办   N 新页面   a 显示每日页面"),
        Line::from(" 右下常驻全部总结   J 全屏展开"),
        Line::from(""),
        Line::from(Span::styled(
            " 日历",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" h/l 选日   j/k 前后一周   [ ] 换月"),
        Line::from(" Enter 当天议程   v 框选日期   t 新建/批量待办"),
        Line::from(""),
        Line::from(Span::styled(
            " 习惯",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" n 新建   Space 今天打卡   u 撤销   e 改名称   d 删除"),
        Line::from(""),
        Line::from(Span::styled(
            " 留言 / 评论",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" 6 留言视图   j/k 选择   Enter 跳转   a 回复"),
        Line::from(" r 解决/取消   e 编辑   d 删除   C 打开当前对象评论"),
        Line::from(""),
        Line::from(Span::styled(
            " 其他",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" i 邀请码   m 成员视角   / 搜索   r 同步   ? 帮助   q 退出"),
        Line::from(" 成员视角：看某人的 1-6 界面（只读，可评论），Enter 切换"),
        Line::from(" 鼠标：点顶部标签切界面、点列表项选中、点日历格子选日期"),
        Line::from(" g / G 跳到列表首/尾；日历里 g 回到今天"),
        Line::from(" Enter 保存（多行内容用 Ctrl+S）   Esc 取消"),
        Line::from(""),
        Line::from(Span::styled(
            " Esc / q 关闭 · j/k 或滚轮滚动",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let total = lines.len();
    let content_height = area.height.saturating_sub(2) as usize;
    let max_scroll = total.saturating_sub(content_height);
    let scroll = panel.scroll.min(max_scroll);
    let title = if max_scroll > 0 {
        format!(
            " 帮助 · v{} · {}-{}/{} ",
            env!("CARGO_PKG_VERSION"),
            scroll + 1,
            (scroll + content_height).min(total),
            total
        )
    } else {
        format!(" 帮助 · v{} ", env!("CARGO_PKG_VERSION"))
    };
    frame.render_widget(
        Paragraph::new(lines.into_iter().skip(scroll).collect::<Vec<_>>())
            .block(block_title(title))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn days_in_month(date: NaiveDate) -> u32 {
    let next = if date.month() == 12 {
        NaiveDate::from_ymd_opt(date.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(date.year(), date.month() + 1, 1)
    };
    next.map(|next| (next - date).num_days() as u32)
        .unwrap_or(30)
}
