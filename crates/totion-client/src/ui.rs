use chrono::{Datelike, Local, NaiveDate};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, EditTarget, Focus, MainState, Screen, SetupForm, View};

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
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(area);

    draw_header(frame, chunks[0], main);

    match main.view() {
        View::Workspace => draw_workspace(frame, chunks[1], main),
        View::Calendar => draw_calendar(frame, chunks[1], main),
        View::Habits => draw_habits(frame, chunks[1], main),
    }

    draw_footer(frame, chunks[2], main);
}

fn draw_header(frame: &mut Frame, area: Rect, main: &MainState) {
    let view_label = match main.view() {
        View::Workspace => "工作区",
        View::Calendar => "日历",
        View::Habits => "习惯",
    };
    let text = format!(
        " totion · {} · {} · {} 位成员 · 视图: {}",
        main.workspace_name(),
        main.me(),
        main.users().len(),
        view_label
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

fn draw_workspace(frame: &mut Frame, area: Rect, main: &MainState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(area);
    draw_pages(frame, columns[0], main);
    draw_blocks(frame, columns[1], main);
}

fn draw_pages(frame: &mut Frame, area: Rect, main: &MainState) {
    let block = Block::default().borders(Borders::ALL).title(" 页面 ");
    let pages = main.visible_pages();
    let editing = main.editing().and_then(|edit| match &edit.target {
        EditTarget::PageTitle { page_id, .. } => Some((page_id.clone(), edit.buffer.clone())),
        _ => None,
    });

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

        let title = editing
            .as_ref()
            .filter(|(id, _)| *id == page.id)
            .map(|(_, buffer)| buffer.as_str())
            .unwrap_or(page.title.as_str());
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
        lines.push(Line::from(Span::styled(
            format!("{marker} {} {title}", page.icon),
            style,
        )));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "还没有页面，按 N 新建",
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
    let title = format!(" {page_title} ");
    let block = Block::default().borders(Borders::ALL).title(title);

    let blocks = main.visible_blocks();
    let editing = main.editing().and_then(|edit| match &edit.target {
        EditTarget::BlockText { block_id, .. } => Some((block_id.clone(), edit.buffer.clone())),
        _ => None,
    });

    let mut lines: Vec<Line> = Vec::new();
    for (index, block_item) in blocks.iter().enumerate() {
        let is_selected = index == main.selected_block_index();
        let editing_this = editing
            .as_ref()
            .filter(|(id, _)| *id == block_item.id)
            .map(|(_, buffer)| buffer.as_str());

        let mut spans: Vec<Span> = Vec::new();
        let marker = if is_selected { "▶ " } else { "  " };
        spans.push(Span::raw(marker));

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

        let text = match editing_this {
            Some(buffer) => format!("{buffer}█"),
            None => block_item.text.clone(),
        };
        let text_style = if block_item.checked && block_item.kind == totion_core::BlockKind::Todo {
            Style::default().fg(Color::DarkGray)
        } else if block_item.kind == totion_core::BlockKind::Heading {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        spans.push(Span::styled(text, text_style));

        if block_item.kind == totion_core::BlockKind::Habit && block_item.streak > 0 {
            spans.push(Span::styled(
                format!("  🔥 {} 天", block_item.streak),
                Style::default().fg(Color::Yellow),
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
            "空页面。n 文本 · t 待办 · h 习惯 · c 日历事件",
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

fn draw_calendar(frame: &mut Frame, area: Rect, main: &MainState) {
    let month = main.calendar_month();
    let title = format!(
        " 日历 · {} 年 {} 月（←/→ 切换月份） ",
        month.year(),
        month.month()
    );
    let block = Block::default().borders(Borders::ALL).title(title);

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
                format!("{:>2}• ", day)
            } else {
                format!("{:>2}  ", day)
            };
            let mut style = Style::default();
            if has_event {
                style = style.fg(Color::Green);
            }
            if date == today {
                style = style
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::White);
            }
            spans.push(Span::styled(label, style));
        }
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(""));
    let mut events = Vec::new();
    for day in 1..=days {
        if let Some(date) = NaiveDate::from_ymd_opt(month.year(), month.month(), day) {
            for block_item in main.calendar_blocks(date) {
                events.push((day, block_item));
            }
        }
    }
    if events.is_empty() {
        lines.push(Line::from(Span::styled(
            "本月没有待办或日程",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (day, block_item) in events.iter().take(12) {
            let mark = if block_item.checked { "[x]" } else { "[ ]" };
            lines.push(Line::from(format!(
                " {day:>2} 日  {mark} {}",
                if block_item.text.is_empty() {
                    "未命名"
                } else {
                    block_item.text.as_str()
                }
            )));
        }
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_habits(frame: &mut Frame, area: Rect, main: &MainState) {
    let block = Block::default().borders(Borders::ALL).title(" 习惯 ");
    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
    let habits = main.all_habit_blocks();

    let mut lines: Vec<Line> = Vec::new();
    for (habit, page) in &habits {
        let checked_today = habit.due_date.as_deref() == Some(today.as_str());
        let status = if checked_today {
            Span::styled("✅ 今日已打卡", Style::default().fg(Color::Green))
        } else {
            Span::styled("待打卡", Style::default().fg(Color::DarkGray))
        };
        let name = if habit.text.is_empty() {
            "未命名习惯"
        } else {
            habit.text.as_str()
        };
        lines.push(Line::from(vec![
            Span::styled("🌱 ", Style::default().fg(Color::Green)),
            Span::styled(
                name.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("   🔥 {} 天", habit.streak),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("   "),
            status,
            Span::styled(
                format!("   · {}", page.title),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "还没有习惯。在页面里按 h 新建一个习惯块。",
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
        " Enter 保存 · Esc 取消 · Backspace 删除"
    } else {
        " Tab 切换 · j/k 移动 · Enter/e 编辑 · n 文本 · t 待办 · h 习惯 · c 日程 · Space 勾选/打卡 · N 新页面 · d 删除 · 1/2/3 视图 · r 同步 · q 退出"
    };

    let lines = vec![
        status_line,
        Line::from(Span::styled(help, Style::default().fg(Color::DarkGray))),
    ];
    frame.render_widget(Paragraph::new(lines), area);
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
