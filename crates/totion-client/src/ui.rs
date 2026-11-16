use chrono::{Datelike, Local, NaiveDate};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, Paragraph, Sparkline, Wrap},
    Frame,
};
use totion_core::{BlockSection, HabitStats};

use crate::app::{
    App, CommentPanel, Editing, EventForm, Focus, HabitForm, MainState, Modal, Screen, SearchState,
    SetupForm, TodayRow, View,
};
use crate::charts;

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
    let editing_height = if main.editing().is_some() { 4 } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(editing_height),
            Constraint::Length(2),
        ])
        .split(area);

    draw_header(frame, chunks[0], main);

    match main.view() {
        View::Today => draw_today(frame, chunks[1], main),
        View::Workspace => draw_workspace(frame, chunks[1], main),
        View::Calendar => draw_calendar(frame, chunks[1], main),
        View::Habits => draw_habits(frame, chunks[1], main),
        View::Stats => draw_stats(frame, chunks[1], main),
    }

    if let Some(editing) = main.editing() {
        draw_editing(frame, chunks[2], editing);
    }

    draw_footer(frame, chunks[3], main);

    if let Some(modal) = main.modal() {
        draw_modal(frame, main, modal);
    }
}

fn draw_header(frame: &mut Frame, area: Rect, main: &MainState) {
    let text = format!(
        " totion · {} · {} · {} 位成员 · {} [{}/5]",
        main.workspace_name(),
        main.me(),
        main.users().len(),
        main.view().label(),
        view_number(main.view())
    );
    let paragraph = Paragraph::new(Line::from(Span::styled(
        text,
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(paragraph, area);
}

fn view_number(view: View) -> u8 {
    match view {
        View::Today => 1,
        View::Workspace => 2,
        View::Calendar => 3,
        View::Habits => 4,
        View::Stats => 5,
    }
}

fn block_title(title: impl Into<String>) -> Block<'static> {
    Block::default().borders(Borders::ALL).title(title.into())
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
            format!(" {}", main.status()),
            Style::default().fg(Color::Gray),
        ))
    };

    let help = if main.editing().is_some() {
        " Enter 保存 · Shift+Enter 无效 · Esc 取消 · 日记/评论用 Ctrl+S 保存"
    } else if let Some(modal) = main.modal() {
        match modal {
            Modal::EventForm(_) => " Tab 切换字段 · Enter 提交 · Esc 取消",
            Modal::HabitForm(_) => " Tab 切换字段 · Ctrl+F 切换频率 · Enter 提交 · Esc 取消",
            Modal::Comments(_) => " a 添加 · e 编辑 · r 解决/取消 · d 删除 · j/k 选择 · Esc 关闭",
            Modal::Search(_) => " 输入搜索 · j/k 选择 · Enter 跳转 · Esc 关闭",
            Modal::Help => " 按 Esc 关闭帮助",
        }
    } else {
        " 1-5 视图 · j/k 移动 · Enter 编辑 · Space 勾选/打卡 · C 评论 · / 搜索 · ? 帮助 · r 同步 · q 退出"
    };

    let lines = vec![
        status_line,
        Line::from(Span::styled(help, Style::default().fg(Color::DarkGray))),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_workspace(frame: &mut Frame, area: Rect, main: &MainState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(area);
    draw_pages(frame, columns[0], main);
    draw_blocks(frame, columns[1], main);
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
            style = style
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD);
        }
        let comments =
            main.unresolved_comment_count(totion_core::CommentTargetKind::Page, &page.id);
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
                if page.is_daily() { " [日记]" } else { "" },
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "还没有页面，按 N 新建",
            Style::default().fg(Color::DarkGray),
        )));
    }
    if main.show_all_pages() {
        lines.push(Line::from(Span::styled(
            "（a 隐藏日记页）",
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
    let block = block_title(format!(" {page_title} "));
    let blocks = main.visible_blocks();

    let mut lines: Vec<Line> = Vec::new();
    for (index, block_item) in blocks.iter().enumerate() {
        let is_selected = index == main.selected_block_index();
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::raw(if is_selected { "▶ " } else { "  " }));

        match block_item.kind {
            totion_core::BlockKind::Todo => {
                spans.push(Span::styled(
                    if block_item.checked { "[x] " } else { "[ ] " },
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
            totion_core::BlockKind::CalendarEvent => {
                let date = block_item
                    .due_date
                    .clone()
                    .unwrap_or_else(|| "未定日期".into());
                spans.push(Span::styled(
                    format!("📅 {date} "),
                    Style::default().fg(Color::Yellow),
                ));
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
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };
        lines.push(Line::from(spans).style(style));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "空页面。n 文本 · t 待办 · c 日程 · Enter 编辑",
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
    let rows = main.today_rows();
    let selected = main.selected_today_index();
    let stat = main.today_completion();
    let date_label = main.today().format("%Y-%m-%d").to_string();
    let weekday = weekday_label(main.today());
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
            Constraint::Min(4),
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {date_label} {weekday}  "),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(bar, Style::default().fg(Color::Green)),
            Span::styled(
                format!("  {}/{}  {rate:.0}%", stat.done, stat.total),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                if main.supports_v2() {
                    ""
                } else {
                    "  （服务器版本过旧，今日/习惯功能不可用）"
                },
                Style::default().fg(Color::Yellow),
            ),
        ])),
        outer[0],
    );

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(outer[1]);

    let mut item_lines: Vec<Line> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        match row {
            TodayRow::Todo(block) | TodayRow::Event(block) => {
                let time = block.start_time.as_deref().unwrap_or("--:--");
                let mark = if block.checked { "[x]" } else { "[ ]" };
                let icon = if matches!(row, TodayRow::Event(_)) {
                    "📅 "
                } else {
                    ""
                };
                let style = if index == selected {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };
                let comments =
                    main.unresolved_comment_count(totion_core::CommentTargetKind::Block, &block.id);
                item_lines.push(Line::from(vec![Span::styled(
                    format!(
                        " {time} {mark} {icon}{}{}",
                        block.text,
                        comment_suffix(comments)
                    ),
                    style,
                )]));
            }
            _ => {}
        }
    }
    if item_lines.is_empty() {
        item_lines.push(Line::from(Span::styled(
            " 今天还没有安排，按 t 新建待办、c 新建日程",
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(item_lines)
            .block(block_title(" 今日事项 "))
            .wrap(Wrap { trim: false }),
        columns[0],
    );

    let mut habit_lines: Vec<Line> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if let TodayRow::Habit(habit, stats) = row {
            let mark = if stats.checked_today { "[x]" } else { "[ ]" };
            let style = if index == selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            habit_lines.push(Line::from(Span::styled(
                format!(
                    " {mark} {} {}  🔥{} · {}",
                    habit.icon,
                    habit.title,
                    stats.current_streak,
                    weekly_label(*stats)
                ),
                style,
            )));
        }
    }
    if habit_lines.is_empty() {
        habit_lines.push(Line::from(Span::styled(
            " 还没有习惯，按 n 创建",
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(habit_lines)
            .block(block_title(" 今日习惯 "))
            .wrap(Wrap { trim: false }),
        columns[1],
    );

    let mut diary_lines: Vec<Line> = Vec::new();
    if let Some(page) = main.daily_page() {
        for section in BlockSection::ALL {
            if let Some(block) = main.diary_block(&page.id, section) {
                let selected_here = rows
                    .get(selected)
                    .map(|row| matches!(row, TodayRow::Diary(item) if item.id == block.id))
                    .unwrap_or(false);
                let preview = if block.text.trim().is_empty() {
                    "（Enter 编辑）".to_string()
                } else {
                    block.text.lines().take(2).collect::<Vec<_>>().join(" / ")
                };
                diary_lines.push(Line::from(vec![
                    Span::styled(
                        format!(" {}: ", section.label()),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        preview,
                        if selected_here {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        },
                    ),
                ]));
            }
        }
    } else {
        diary_lines.push(Line::from(Span::styled(
            " 正在创建今日页…（服务器需为 v0.2）",
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(diary_lines)
            .block(block_title(" 日记：总结 / 感悟 / 经验 "))
            .wrap(Wrap { trim: false }),
        outer[2],
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
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let month = main.calendar_month();
    let title = format!(
        " 日历 · {} 年 {} 月（←/→ 选日 · [ ] 换月） ",
        month.year(),
        month.month()
    );
    let block = block_title(title);
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        " 一   二   三   四   五   六   日",
        Style::default().fg(Color::DarkGray),
    )));

    let offset = month.weekday().num_days_from_monday() as usize;
    let days = days_in_month(month);
    let today = Local::now().date_naive();
    for week in 0..6 {
        let mut spans: Vec<Span> = Vec::new();
        for weekday in 0..7 {
            let index = week * 7 + weekday;
            if index < offset || index >= offset + days as usize {
                spans.push(Span::raw("    "));
                continue;
            }
            let day = (index - offset + 1) as u32;
            let date = NaiveDate::from_ymd_opt(month.year(), month.month(), day).unwrap_or(month);
            let has_event = !main.calendar_blocks(date).is_empty();
            let label = if has_event {
                format!("{day:>2}• ")
            } else {
                format!("{day:>2}  ")
            };
            let mut style = Style::default();
            if has_event {
                style = style.fg(Color::Green);
            }
            if date == main.selected_day() {
                style = style
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD);
            } else if date == today {
                style = style
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD);
            }
            spans.push(Span::styled(label, style));
        }
        lines.push(Line::from(spans));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        columns[0],
    );

    let selected = main.selected_day();
    let agenda = main.agenda_blocks();
    let mut agenda_lines: Vec<Line> = Vec::new();
    for (index, block) in agenda.iter().enumerate() {
        let time = block.start_time.as_deref().unwrap_or("--:--");
        let mark = if block.checked { "[x]" } else { "[ ]" };
        let style = if index == main.selected_agenda_index() {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };
        let comments =
            main.unresolved_comment_count(totion_core::CommentTargetKind::Block, &block.id);
        agenda_lines.push(Line::from(Span::styled(
            format!(" {time} {mark} {}{}", block.text, comment_suffix(comments)),
            style,
        )));
    }
    if agenda_lines.is_empty() {
        agenda_lines.push(Line::from(Span::styled(
            " 这天没有安排，按 t 新建待办、c 新建日程",
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(agenda_lines)
            .block(block_title(format!(
                " {} 议程 ",
                selected.format("%Y-%m-%d")
            )))
            .wrap(Wrap { trim: false }),
        columns[1],
    );
}

fn draw_habits(frame: &mut Frame, area: Rect, main: &MainState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(36), Constraint::Percentage(64)])
        .split(area);

    let habits = main.visible_habits();
    let mut lines: Vec<Line> = Vec::new();
    for (index, habit) in habits.iter().enumerate() {
        let stats = main.habit_stats(habit);
        let style = if index == main.selected_habit_index() {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };
        let mark = if stats.checked_today { "[x]" } else { "[ ]" };
        let comments =
            main.unresolved_comment_count(totion_core::CommentTargetKind::Habit, &habit.id);
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
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " 还没有习惯，按 n 创建第一个",
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(block_title(" 习惯 "))
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
        let grid = main.heatmap_grid(habit, 12);
        detail_lines.extend(charts::heatmap_lines(&grid));
        if !habit.note.trim().is_empty() {
            detail_lines.push(Line::from(""));
            detail_lines.push(Line::from(format!(" 备注: {}", habit.note)));
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

    let grid = main.aggregate_heatmap(16);
    frame.render_widget(
        Paragraph::new(charts::heatmap_lines(&grid))
            .block(block_title(" 最近 16 周打卡热力图 "))
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
        Modal::Search(state) => draw_search(frame, state),
        Modal::Help => draw_help(frame),
    }
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
            Style::default().bg(Color::DarkGray)
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
                Style::default().bg(Color::DarkGray)
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

fn draw_search(frame: &mut Frame, state: &SearchState) {
    let area = centered_rect(72, 60, frame.area());
    frame.render_widget(Clear, area);
    let mut lines = vec![Line::from(Span::styled(
        format!(" 搜索: {}█", state.query),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))];
    lines.push(Line::from(""));
    for (index, hit) in state.results.iter().enumerate() {
        let style = if index == state.selected {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };
        let text = match hit {
            crate::app::SearchHit::Page(id) => format!(" [页面] {id}"),
            crate::app::SearchHit::Block { block_id, .. } => format!(" [内容块] {block_id}"),
            crate::app::SearchHit::Habit(id) => format!(" [习惯] {id}"),
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    if state.results.is_empty() && !state.query.is_empty() {
        lines.push(Line::from(Span::styled(
            " 没有匹配结果",
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(block_title(" 搜索（Enter 跳转 · Esc 关闭） "))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_help(frame: &mut Frame) {
    let area = centered_rect(70, 80, frame.area());
    frame.render_widget(Clear, area);
    let lines = vec![
        Line::from(Span::styled(
            " 视图",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" 1 今日   2 工作区   3 日历   4 习惯   5 统计"),
        Line::from(""),
        Line::from(Span::styled(
            " 通用",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" j/k 移动   Enter/e 编辑   Space 勾选或打卡   u 撤销打卡"),
        Line::from(" C 评论   / 搜索   ? 帮助   r 同步   q 退出"),
        Line::from(""),
        Line::from(Span::styled(
            " 今日",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" t 新建待办   c 新建日程   n 新建习惯   d 删除"),
        Line::from(""),
        Line::from(Span::styled(
            " 日历",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" h/l 前后一天   j/k 前后一周   [ ] 换月"),
        Line::from(""),
        Line::from(Span::styled(
            " 习惯",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" n 新建习惯   Space 今天打卡   u 撤销   e 改名称   d 删除"),
        Line::from(""),
        Line::from(Span::styled(
            " 评论面板",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" a 添加   e 编辑   r 解决/取消解决   d 删除   Esc 关闭"),
        Line::from(""),
        Line::from(Span::styled(
            " 编辑",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(" Enter 保存（多行内容用 Ctrl+S）   Esc 取消"),
        Line::from(""),
        Line::from(Span::styled(
            " 按任意键关闭",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(block_title(" 帮助 "))
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
