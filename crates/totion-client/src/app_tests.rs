use super::*;

use ratatui::{backend::TestBackend, Terminal};
use totion_core::WorkspaceInfo;

fn sample_user(id: &str, name: &str) -> User {
    User {
        id: id.to_string(),
        name: name.to_string(),
        public_key: format!("key-{id}"),
        created_at: chrono::Utc::now(),
    }
}

fn sample_block(id: &str, text: &str, owner: &str, date: &str, time: &str) -> Block {
    Block {
        id: id.to_string(),
        page_id: "p1".to_string(),
        kind: BlockKind::Todo,
        text: text.to_string(),
        checked: false,
        due_date: Some(date.to_string()),
        streak: 0,
        position: 0,
        version: 1,
        updated_at: chrono::Utc::now(),
        updated_by: if owner == "u1" { "Guan" } else { "Bob" }.to_string(),
        deleted: false,
        start_time: Some(time.to_string()),
        end_time: None,
        completed_at: None,
        completed_by: None,
        section: None,
        owner_id: owner.to_string(),
    }
}

fn sample_state() -> MainState {
    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
    let page = Page {
        id: "p1".to_string(),
        title: "收件箱".to_string(),
        icon: "📄".to_string(),
        kind: PageKind::Todo,
        position: 0,
        version: 1,
        updated_at: chrono::Utc::now(),
        updated_by: "Guan".to_string(),
        deleted: false,
        date: None,
    };
    let habit = Habit {
        id: "h1".to_string(),
        title: "阅读".to_string(),
        icon: "📖".to_string(),
        note: String::new(),
        frequency: HabitFrequency::Daily,
        position: 0,
        archived: false,
        version: 1,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        updated_by: "Guan".to_string(),
        deleted: false,
        owner_id: "u1".to_string(),
    };
    let snapshot = Snapshot {
        workspace: WorkspaceInfo {
            name: "测试空间".to_string(),
            users: vec![sample_user("u1", "Guan"), sample_user("u2", "Bob")],
        },
        pages: vec![page],
        blocks: vec![
            sample_block("b1", "我的待办", "u1", &today, "09:00"),
            sample_block("b2", "Bob 的待办", "u2", &today, "10:00"),
        ],
        habits: vec![habit],
        checkins: Vec::new(),
        comments: Vec::new(),
        journals: Vec::new(),
        last_seq: 0,
        api_version: 5,
    };
    MainState::new(
        Api::new("http://127.0.0.1:9"),
        "Guan".to_string(),
        "u1".to_string(),
        snapshot,
    )
}

struct Harness {
    app: App,
    buffer: Option<ratatui::buffer::Buffer>,
}

impl Harness {
    fn new() -> Self {
        Self {
            app: App {
                screen: Screen::Main(sample_state()),
                should_quit: false,
            },
            buffer: None,
        }
    }

    fn state(&self) -> &MainState {
        match &self.app.screen {
            Screen::Main(state) => state,
            Screen::Setup(_) => panic!("测试应处于主界面"),
        }
    }

    fn state_mut(&mut self) -> &mut MainState {
        match &mut self.app.screen {
            Screen::Main(state) => state,
            Screen::Setup(_) => panic!("测试应处于主界面"),
        }
    }

    fn render(&mut self, width: u16, height: u16) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("创建测试终端");
        let app = &self.app;
        terminal
            .draw(|frame| crate::ui::draw(frame, app))
            .expect("绘制界面");
        self.buffer = Some(terminal.backend().buffer().clone());
    }

    fn action_rect(&self, action: MouseAction) -> Rect {
        self.state()
            .mouse_hits
            .borrow()
            .iter()
            .find(|(_, kind)| *kind == action)
            .map(|(rect, _)| *rect)
            .unwrap_or_else(|| panic!("找不到点击区域: {action:?}"))
    }

    fn click_action(&mut self, action: MouseAction) {
        let rect = self.action_rect(action);
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: rect.x + rect.width / 2,
            row: rect.y,
            modifiers: KeyModifiers::empty(),
        };
        self.app.handle_mouse(event);
    }

    fn click_at(&mut self, column: u16, row: u16) {
        self.app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::empty(),
        });
    }

    fn wheel(&mut self, down: bool) {
        let kind = if down {
            MouseEventKind::ScrollDown
        } else {
            MouseEventKind::ScrollUp
        };
        self.app.handle_mouse(MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::empty(),
        });
    }

    fn press(&mut self, code: KeyCode) {
        self.state_mut()
            .handle_today_key(KeyEvent::new(code, KeyModifiers::NONE));
    }
}

#[test]
fn nav_bar_switches_every_view() {
    let mut harness = Harness::new();
    harness.render(140, 40);
    for view in [
        View::Today,
        View::Workspace,
        View::Calendar,
        View::Habits,
        View::Stats,
        View::Inbox,
    ] {
        harness.render(140, 40);
        harness.click_action(MouseAction::SwitchView(view));
        assert_eq!(harness.state().view(), view);
    }
}

#[test]
fn clicking_today_rows_selects_items_and_habits() {
    let mut harness = Harness::new();
    harness.render(140, 40);
    harness.click_action(MouseAction::SelectTodayItem(1));
    assert_eq!(harness.state().today_focus(), TodayFocus::Items);
    assert_eq!(harness.state().selected_today_index(), 1);

    harness.render(140, 40);
    harness.click_action(MouseAction::SelectTodayHabit(0));
    assert_eq!(harness.state().today_focus(), TodayFocus::Habits);
    assert_eq!(harness.state().selected_today_habit_index(), 0);
}

#[test]
fn member_view_is_read_only_and_filters_data() {
    let mut harness = Harness::new();
    harness.state_mut().apply_member_view(2);
    let state = harness.state();
    assert!(state.read_only());
    let items = state.today_items();
    assert_eq!(items.len(), 1);
    match &items[0] {
        TodayRow::Todo(block) => assert_eq!(block.text, "Bob 的待办"),
        other => panic!("应为待办行: {other:?}"),
    }
    assert!(state.today_habits().is_empty());
}

#[test]
fn week_view_includes_today_and_pages_forward() {
    let mut harness = Harness::new();
    let today = harness.state().today();
    harness.state_mut().view_date = today - ChronoDuration::days(1);
    harness.state_mut().open_week_view();

    {
        let state = harness.state();
        assert!(state.week_mode());
        assert_eq!(state.week_end(), today);
        assert_eq!(state.week_selected(), 5);
        let overview = state.week_overview();
        assert_eq!(overview.len(), 7);
        assert_eq!(overview[6].0, today);
        assert_eq!(overview[6].2.len(), 2);
    }

    harness.press(KeyCode::Char('l'));
    assert_eq!(
        harness.state().week_end(),
        harness.state().today() + ChronoDuration::days(1)
    );

    harness.press(KeyCode::Enter);
    assert!(!harness.state().week_mode());
    // 窗口向前平移一天后，原来指向「昨天」的第 5 行现在是今天。
    assert_eq!(harness.state().view_date(), harness.state().today());
}

#[test]
fn week_view_keeps_far_past_date_in_window() {
    let mut harness = Harness::new();
    let old = harness.state().today() - ChronoDuration::days(20);
    harness.state_mut().view_date = old;
    harness.state_mut().open_week_view();
    let state = harness.state();
    assert_eq!(state.week_end(), old);
    assert_eq!(state.week_selected(), 6);
}

#[test]
fn search_results_show_readable_labels() {
    let harness = Harness::new();
    let hits = harness.state().compute_search("待办");
    assert!(!hits.is_empty());
    let labels: Vec<String> = hits
        .iter()
        .map(|hit| harness.state().search_hit_label(hit))
        .collect();
    assert!(labels.iter().any(|label| label.contains("我的待办")));
    assert!(labels.iter().any(|label| label.contains("收件箱")));
    assert!(labels.iter().all(|label| label != "b1" && label != "b2"));
}

#[test]
fn window_range_keeps_selection_visible() {
    assert_eq!(crate::ui::window_range(0, 0, 5), (0, 0));
    assert_eq!(crate::ui::window_range(3, 2, 5), (0, 3));
    assert_eq!(crate::ui::window_range(20, 0, 5), (0, 5));
    assert_eq!(crate::ui::window_range(20, 10, 5), (8, 13));
    assert_eq!(crate::ui::window_range(20, 19, 5), (15, 20));
}

#[test]
fn long_today_list_scrolls_to_selected_item() {
    let mut harness = Harness::new();
    {
        let state = harness.state_mut();
        let today = state.today().format("%Y-%m-%d").to_string();
        for index in 0..25u32 {
            state.blocks.push(sample_block(
                &format!("extra-{index}"),
                &format!("任务 {index}"),
                "u1",
                &today,
                "12:00",
            ));
        }
        state.selected_today_index = 24;
        state.clamp();
    }
    harness.render(120, 24);
    let actions: Vec<MouseAction> = harness
        .state()
        .mouse_hits
        .borrow()
        .iter()
        .map(|(_, action)| *action)
        .collect();
    assert!(actions.contains(&MouseAction::SelectTodayItem(24)));
    assert!(!actions.contains(&MouseAction::SelectTodayItem(0)));
}

#[test]
fn scroll_start_handles_variable_heights() {
    assert_eq!(crate::ui::scroll_start(&[], 0, 5), 0);
    assert_eq!(crate::ui::scroll_start(&[2, 2, 2], 2, 10), 0);
    assert_eq!(crate::ui::scroll_start(&[2, 2, 2, 2, 2], 0, 5), 0);
    assert_eq!(crate::ui::scroll_start(&[2, 2, 2, 2, 2], 4, 5), 3);
    assert_eq!(crate::ui::scroll_start(&[10, 2, 2], 1, 5), 1);
}

#[test]
fn inbox_scrolls_to_selected_comment() {
    let mut harness = Harness::new();
    {
        let state = harness.state_mut();
        for index in 0..20 {
            state.comments.push(Comment {
                id: format!("c{index}"),
                target_kind: CommentTargetKind::Block,
                target_id: "b1".to_string(),
                body: "第一行\n第二行".to_string(),
                resolved: false,
                author_id: "u1".to_string(),
                author_name: "Guan".to_string(),
                version: 1,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                deleted: false,
            });
        }
        state.selected_inbox_index = 19;
        state.set_view(View::Inbox);
        state.clamp();
    }
    harness.render(120, 24);
    let actions: Vec<MouseAction> = harness
        .state()
        .mouse_hits
        .borrow()
        .iter()
        .map(|(_, action)| *action)
        .collect();
    assert!(actions.contains(&MouseAction::SelectInbox(19)));
}

#[test]
fn week_view_scrolls_to_selected_day() {
    let mut harness = Harness::new();
    {
        let state = harness.state_mut();
        let today = state.today();
        for offset in 0..7 {
            let date = (today - ChronoDuration::days(6) + ChronoDuration::days(offset))
                .format("%Y-%m-%d")
                .to_string();
            for item in 0..4 {
                state.blocks.push(sample_block(
                    &format!("w{offset}-{item}"),
                    &format!("任务 {offset}-{item}"),
                    "u1",
                    &date,
                    "12:00",
                ));
            }
        }
        state.open_week_view();
        state.week_selected = 6;
        state.clamp();
    }
    harness.render(120, 24);
    let actions: Vec<MouseAction> = harness
        .state()
        .mouse_hits
        .borrow()
        .iter()
        .map(|(_, action)| *action)
        .collect();
    assert!(actions.contains(&MouseAction::WeekSelect(6)));
    assert!(!actions.contains(&MouseAction::WeekSelect(0)));
}

#[test]
fn status_fades_back_to_connection_state() {
    let mut harness = Harness::new();
    {
        let state = harness.state_mut();
        state.status = "已删除".to_string();
        state.status_seen = "已删除".to_string();
        state.status_since = Instant::now();
    }
    assert_eq!(harness.state().status(), "已删除");

    harness.state_mut().status_since = Instant::now() - Duration::from_secs(10);
    let idle = harness.state().status();
    assert!(idle.starts_with("已连接"), "期望回到连接状态，实际: {idle}");
}

#[test]
fn selected_rows_use_shared_highlight_style() {
    let mut harness = Harness::new();
    harness.render(120, 30);
    let selected = harness.action_rect(MouseAction::SelectTodayItem(0));
    let buffer = harness.buffer.as_ref().expect("buffer");
    let cell = buffer
        .cell((selected.x + 2, selected.y))
        .expect("选中行单元格");
    assert_eq!(cell.bg, crate::ui::SELECTION_BG);

    let other = harness.action_rect(MouseAction::SelectTodayItem(1));
    let cell = buffer.cell((other.x + 2, other.y)).expect("未选中行单元格");
    assert_ne!(cell.bg, crate::ui::SELECTION_BG);
}

#[test]
fn mouse_wheel_moves_selection() {
    let mut harness = Harness::new();
    harness.render(120, 30);
    assert_eq!(harness.state().selected_today_index(), 0);
    harness.wheel(true);
    assert_eq!(harness.state().selected_today_index(), 1);
    harness.wheel(false);
    assert_eq!(harness.state().selected_today_index(), 0);
}

fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
    let area = buffer.area;
    let mut output = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            if let Some(cell) = buffer.cell((x, y)) {
                output.push_str(cell.symbol());
            }
        }
        output.push('\n');
    }
    output
}

#[test]
fn stats_shows_heatmap_range_and_empty_hint() {
    let mut harness = Harness::new();
    harness.state_mut().set_view(View::Stats);
    harness.render(120, 40);
    let text = buffer_text(harness.buffer.as_ref().expect("buffer"));
    // 宽字符占用两个单元格，去掉空隙再匹配。
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    assert!(compact.contains("最近52周打卡热力图"), "缺少热力图标题");
    assert!(compact.contains("还没有打卡记录"), "缺少空状态提示");
}

#[test]
fn double_click_calendar_opens_agenda() {
    let mut harness = Harness::new();
    harness.state_mut().set_view(View::Calendar);
    harness.render(120, 40);
    let cell = {
        let cells = harness.state().calendar_cells.borrow();
        cells.first().map(|(rect, _)| *rect).expect("日历格子")
    };
    let column = cell.x + cell.width / 2;
    let row = cell.y + cell.height / 2;
    harness.click_at(column, row);
    assert!(harness.state().modal().is_none(), "单击只应选日");
    harness.click_at(column, row);
    assert!(
        matches!(harness.state().modal(), Some(Modal::Agenda(_))),
        "双击应打开议程"
    );
}

#[test]
fn g_and_shift_g_jump_to_list_ends() {
    let mut harness = Harness::new();
    harness.press(KeyCode::Char('G'));
    assert_eq!(harness.state().selected_today_index(), 1);
    harness.press(KeyCode::Char('g'));
    assert_eq!(harness.state().selected_today_index(), 0);

    harness.state_mut().set_view(View::Inbox);
    {
        let state = harness.state_mut();
        for index in 0..3 {
            state.comments.push(Comment {
                id: format!("g{index}"),
                target_kind: CommentTargetKind::Block,
                target_id: "b1".to_string(),
                body: "内容".to_string(),
                resolved: false,
                author_id: "u1".to_string(),
                author_name: "Guan".to_string(),
                version: 1,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                deleted: false,
            });
        }
        state.clamp();
    }
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE));
    assert_eq!(harness.state().selected_inbox_index(), 2);
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
    assert_eq!(harness.state().selected_inbox_index(), 0);
}

#[test]
fn g_jumps_calendar_back_to_today() {
    let mut harness = Harness::new();
    harness.state_mut().set_view(View::Calendar);
    let today = harness.state().today();
    harness.state_mut().selected_day = today - ChronoDuration::days(10);
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
    assert_eq!(harness.state().selected_day(), harness.state().today());
}

#[test]
fn checkbox_hit_area_sits_on_the_toggle_marker() {
    let mut harness = Harness::new();
    harness.render(120, 30);
    let row = harness.action_rect(MouseAction::SelectTodayItem(0));
    let checkbox = harness.action_rect(MouseAction::ToggleTodayItem(0));
    assert_eq!(checkbox.x, row.x + 8);
    assert_eq!(checkbox.y, row.y);
    assert_eq!(checkbox.width, 2);
}

#[test]
fn checkbox_click_is_blocked_in_member_view() {
    let mut harness = Harness::new();
    harness.state_mut().apply_member_view(2);
    harness.render(120, 30);
    harness.click_action(MouseAction::ToggleTodayItem(0));
    assert!(
        harness.state().status.contains("只能评论"),
        "只读视角应拦下勾选，实际状态: {}",
        harness.state().status
    );
}

#[test]
fn calendar_banner_shows_month_totals() {
    let mut harness = Harness::new();
    let today = harness.state().today();
    let stat = harness.state().month_block_stats(today);
    assert_eq!((stat.total, stat.done), (2, 0));

    harness.state_mut().set_view(View::Calendar);
    harness.render(120, 40);
    let text = buffer_text(harness.buffer.as_ref().expect("buffer"));
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    assert!(
        compact.contains("待办2·完成0"),
        "日历横幅缺少统计: {compact}"
    );
}

#[test]
fn help_modal_scrolls_and_closes() {
    let mut harness = Harness::new();
    harness.state_mut().modal = Some(Modal::Help(HelpPanel { scroll: 0 }));
    harness.render(100, 24);
    for _ in 0..5 {
        harness
            .app
            .handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
    }
    match harness.state().modal() {
        Some(Modal::Help(panel)) => assert!(panel.scroll > 0, "j 应滚动帮助"),
        _ => panic!("帮助弹窗应仍然打开"),
    }
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(harness.state().modal().is_none());
}

#[test]
fn modal_lists_support_g_and_shift_g() {
    let mut harness = Harness::new();
    harness.state_mut().modal = Some(Modal::Members(MemberPanel { selected: 0 }));
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE));
    match harness.state().modal() {
        Some(Modal::Members(panel)) => assert_eq!(panel.selected, 2),
        _ => panic!("成员面板应仍然打开"),
    }

    {
        let state = harness.state_mut();
        for index in 0..3 {
            state.comments.push(Comment {
                id: format!("m{index}"),
                target_kind: CommentTargetKind::Block,
                target_id: "b1".to_string(),
                body: "内容".to_string(),
                resolved: false,
                author_id: "u1".to_string(),
                author_name: "Guan".to_string(),
                version: 1,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                deleted: false,
            });
        }
        state.modal = Some(Modal::Comments(CommentPanel {
            target_kind: CommentTargetKind::Block,
            target_id: "b1".to_string(),
            label: "待办".to_string(),
            selected: 0,
        }));
    }
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE));
    match harness.state().modal() {
        Some(Modal::Comments(panel)) => assert_eq!(panel.selected, 2),
        _ => panic!("评论面板应仍然打开"),
    }
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
    match harness.state().modal() {
        Some(Modal::Comments(panel)) => assert_eq!(panel.selected, 0),
        _ => panic!("评论面板应仍然打开"),
    }
}

#[test]
fn modals_use_unified_double_border() {
    let mut harness = Harness::new();
    harness.state_mut().modal = Some(Modal::Members(MemberPanel { selected: 0 }));
    harness.render(120, 36);
    let text = buffer_text(harness.buffer.as_ref().expect("buffer"));
    assert!(text.contains('╔'), "弹窗应使用双线边框");
    assert!(text.contains('╭'), "普通面板应保持圆角边框");
}

#[test]
fn comments_modal_scrolls_to_selected_comment() {
    let mut harness = Harness::new();
    {
        let state = harness.state_mut();
        for index in 0..5 {
            state.comments.push(Comment {
                id: format!("k{index}"),
                target_kind: CommentTargetKind::Block,
                target_id: "b1".to_string(),
                body: "第一行\n第二行\n第三行".to_string(),
                resolved: false,
                author_id: "u1".to_string(),
                author_name: "Guan".to_string(),
                version: 1,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                deleted: false,
            });
        }
        state.modal = Some(Modal::Comments(CommentPanel {
            target_kind: CommentTargetKind::Block,
            target_id: "b1".to_string(),
            label: "待办".to_string(),
            selected: 4,
        }));
    }
    harness.render(120, 24);
    let text = buffer_text(harness.buffer.as_ref().expect("buffer"));
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    assert!(
        compact.contains("2-5/5"),
        "评论面板标题应显示滚动范围: {compact}"
    );
}

#[test]
fn long_page_list_scrolls_to_selected_page() {
    let mut harness = Harness::new();
    {
        let state = harness.state_mut();
        for index in 0..30 {
            state.pages.push(Page {
                id: format!("p{index}"),
                title: format!("页面 {index}"),
                icon: "📄".to_string(),
                kind: PageKind::Doc,
                position: index,
                version: 1,
                updated_at: chrono::Utc::now(),
                updated_by: "Guan".to_string(),
                deleted: false,
                date: None,
            });
        }
        state.selected_page_index = 30;
        state.set_view(View::Workspace);
        state.clamp();
    }
    harness.render(120, 24);
    let actions: Vec<MouseAction> = harness
        .state()
        .mouse_hits
        .borrow()
        .iter()
        .map(|(_, action)| *action)
        .collect();
    assert!(actions.contains(&MouseAction::SelectPage(30)));
    assert!(!actions.contains(&MouseAction::SelectPage(0)));
}

#[test]
fn long_journal_list_scrolls_to_selected_entry() {
    let mut harness = Harness::new();
    {
        let state = harness.state_mut();
        for index in 0..10 {
            state.journals.push(totion_core::JournalEntry {
                id: format!("j{index}"),
                target_kind: totion_core::JournalTarget::Block,
                target_id: "b1".to_string(),
                date: format!("2026-11-{:02}", index + 1),
                time: "21:00".to_string(),
                body: "第一行\n第二行\n第三行".to_string(),
                author_id: "u1".to_string(),
                author_name: "Guan".to_string(),
                version: 1,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                deleted: false,
            });
        }
        state.set_view(View::Workspace);
        state.workspace_journal_mode = true;
        state.selected_journal_index = 9;
        state.clamp();
    }
    harness.render(120, 24);
    let actions: Vec<MouseAction> = harness
        .state()
        .mouse_hits
        .borrow()
        .iter()
        .map(|(_, action)| *action)
        .collect();
    assert!(actions.contains(&MouseAction::OpenJournal(9)));
    assert!(!actions.contains(&MouseAction::OpenJournal(0)));
}

#[test]
fn inbox_r_resolves_instead_of_triggering_sync() {
    let mut harness = Harness::new();
    {
        let state = harness.state_mut();
        state.comments.push(Comment {
            id: "c-r".to_string(),
            target_kind: CommentTargetKind::Block,
            target_id: "b1".to_string(),
            body: "内容".to_string(),
            resolved: false,
            author_id: "u1".to_string(),
            author_name: "Guan".to_string(),
            version: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted: false,
        });
        state.set_view(View::Inbox);
        state.clamp();
    }
    let before = *harness.state().last_sync.as_ref().expect("last_sync");
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    let after = *harness.state().last_sync.as_ref().expect("last_sync");
    assert_eq!(before, after, "留言视图的 r 不应触发全局同步");
}

#[test]
fn esc_cancels_transient_view_modes() {
    let mut harness = Harness::new();
    harness.state_mut().open_week_view();
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!harness.state().week_mode(), "Esc 应退出周视图");

    harness.state_mut().set_view(View::Workspace);
    harness.state_mut().workspace_journal_mode = true;
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        !harness.state().workspace_journal_mode,
        "Esc 应退出总结列表"
    );

    harness.state_mut().set_view(View::Calendar);
    let today = harness.state().today();
    harness.state_mut().range_anchor = Some(today);
    harness.state_mut().range_focus = Some(today);
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(harness.state().range_anchor.is_none(), "Esc 应取消日期框选");
}

#[test]
fn confirm_shows_hint_on_wrong_input() {
    let mut harness = Harness::new();
    harness.state_mut().modal = Some(Modal::Confirm(ConfirmState {
        action: PendingDelete::Block {
            id: "b1".to_string(),
            version: 1,
            label: "待办「写周报」".to_string(),
        },
        input: "del".to_string(),
        label: "待办「写周报」".to_string(),
        hint: None,
    }));
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    match harness.state().modal() {
        Some(Modal::Confirm(confirm)) => {
            assert!(confirm.hint.is_some(), "错误输入应给出提示");
            assert!(confirm.input.is_empty(), "错误输入后应清空输入框");
        }
        _ => panic!("确认框应仍然打开"),
    }
}

#[test]
fn calendar_shows_adjacent_month_days_and_click_navigates() {
    let mut harness = Harness::new();
    harness.state_mut().set_view(View::Calendar);
    harness.render(120, 40);
    let month = harness.state().calendar_month();
    let neighbor = {
        let cells = harness.state().calendar_cells.borrow();
        cells
            .iter()
            .find(|(_, date)| date.month() != month.month() || date.year() != month.year())
            .map(|(rect, date)| (*rect, *date))
            .expect("应显示相邻月份的日期")
    };
    let (rect, date) = neighbor;
    harness.click_at(rect.x + 1, rect.y + 1);
    assert_eq!(harness.state().selected_day(), date);
    assert_eq!(
        harness.state().calendar_month().month(),
        date.month(),
        "点击相邻月份日期应切到该月"
    );
}

fn buffer_line(buffer: &ratatui::buffer::Buffer, row: u16) -> String {
    let area = buffer.area;
    let mut output = String::new();
    for x in 0..area.width {
        if let Some(cell) = buffer.cell((x, row)) {
            output.push_str(cell.symbol());
        }
    }
    output
}

#[test]
fn scrolled_page_list_keeps_group_header_sticky() {
    let mut harness = Harness::new();
    {
        let state = harness.state_mut();
        for index in 0..40 {
            state.pages.push(Page {
                id: format!("d{index}"),
                title: format!("文档 {index}"),
                icon: "📄".to_string(),
                kind: PageKind::Doc,
                position: index as i64 + 1,
                version: 1,
                updated_at: chrono::Utc::now(),
                updated_by: "Guan".to_string(),
                deleted: false,
                date: None,
            });
        }
        state.set_view(View::Workspace);
        state.selected_page_index = 40;
        state.clamp();
    }
    harness.render(120, 24);
    let first_content_row = buffer_line(harness.buffer.as_ref().expect("buffer"), 3);
    let compact: String = first_content_row
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    assert!(
        compact.contains("文档"),
        "滚动后第一行应是固定的分组标题，实际: {compact}"
    );
}

#[test]
fn multiline_editing_moves_cursor_between_lines() {
    let mut harness = Harness::new();
    harness.state_mut().editing = Some(Editing {
        target: EditTarget::NewComment {
            target_kind: CommentTargetKind::Block,
            target_id: "b1".to_string(),
        },
        buffer: "ab\ncdef".to_string(),
        cursor: 7,
        multiline: true,
    });

    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(harness.state().editing.as_ref().unwrap().cursor, 2);

    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(harness.state().editing.as_ref().unwrap().cursor, 5);

    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    assert_eq!(harness.state().editing.as_ref().unwrap().cursor, 3);

    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    assert_eq!(harness.state().editing.as_ref().unwrap().cursor, 7);

    // 中文按字符移动，不会把字节切开。
    harness.state_mut().editing = Some(Editing {
        target: EditTarget::NewComment {
            target_kind: CommentTargetKind::Block,
            target_id: "b1".to_string(),
        },
        buffer: "你好\n世界".to_string(),
        cursor: 13,
        multiline: true,
    });
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(harness.state().editing.as_ref().unwrap().cursor, 6);
}

#[test]
fn search_jump_clears_workspace_transient_modes() {
    let mut harness = Harness::new();
    harness.state_mut().set_view(View::Workspace);
    harness.state_mut().workspace_journal_mode = true;
    let hit = harness
        .state()
        .compute_search("我的待办")
        .into_iter()
        .next()
        .expect("应搜到待办");
    harness.state_mut().jump_to_search_hit(hit);
    assert!(
        !harness.state().workspace_journal_mode,
        "跳转应退出总结列表"
    );
    assert_eq!(harness.state().view(), View::Workspace);
    assert_eq!(harness.state().focus(), Focus::Blocks);
}

#[test]
fn editing_header_reflects_multiline_mode() {
    let mut harness = Harness::new();
    harness.state_mut().editing = Some(Editing {
        target: EditTarget::NewComment {
            target_kind: CommentTargetKind::Block,
            target_id: "b1".to_string(),
        },
        buffer: "第一行\n第二行".to_string(),
        cursor: 19,
        multiline: true,
    });
    harness.render(120, 30);
    let text = buffer_text(harness.buffer.as_ref().expect("buffer"));
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    assert!(compact.contains("Ctrl+S保存"), "多行编辑应提示 Ctrl+S 保存");
    assert!(compact.contains("Enter换行"), "多行编辑应提示 Enter 换行");
    assert!(compact.contains("L2:C4"), "应显示光标行列: {compact}");

    harness.state_mut().editing = Some(Editing {
        target: EditTarget::PageTitle {
            page_id: "p1".to_string(),
            base_version: 1,
        },
        buffer: "标题".to_string(),
        cursor: 6,
        multiline: false,
    });
    harness.render(120, 30);
    let text = buffer_text(harness.buffer.as_ref().expect("buffer"));
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    assert!(
        compact.contains("Enter保存·Esc取消"),
        "单行编辑应提示 Enter 保存"
    );
    assert!(
        !compact.contains("Enter换行"),
        "单行编辑不应提示 Enter 换行"
    );
}

#[test]
fn long_edit_line_scrolls_horizontally() {
    let mut harness = Harness::new();
    harness.state_mut().editing = Some(Editing {
        target: EditTarget::NewComment {
            target_kind: CommentTargetKind::Block,
            target_id: "b1".to_string(),
        },
        buffer: "a".repeat(60),
        cursor: 60,
        multiline: false,
    });
    harness.render(40, 24);
    let text = buffer_text(harness.buffer.as_ref().expect("buffer"));
    assert!(text.contains('█'), "长行时光标应保持在可视区内");
    assert!(!text.contains(&"a".repeat(40)), "超宽行不应整行绘制");
}

#[test]
fn members_modal_scrolls_to_selected_member() {
    let mut harness = Harness::new();
    {
        let state = harness.state_mut();
        for index in 0..30 {
            state
                .users
                .push(sample_user(&format!("u{index}"), &format!("成员{index}")));
        }
        state.modal = Some(Modal::Members(MemberPanel { selected: 30 }));
    }
    harness.render(120, 24);
    let text = buffer_text(harness.buffer.as_ref().expect("buffer"));
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    assert!(
        compact.contains("24-33/33"),
        "成员面板应显示滚动范围: {compact}"
    );
}

#[test]
fn failed_manual_sync_reports_failure_not_success() {
    let mut harness = Harness::new();
    harness.state_mut().sync_now(true);
    let status = &harness.state().status;
    assert!(
        status.contains("同步失败"),
        "手动同步失败时应显示失败，实际: {status}"
    );
    assert!(
        !status.contains("已同步"),
        "失败时不应覆盖成成功提示，实际: {status}"
    );
}

#[test]
fn auto_sync_failure_keeps_offline_status() {
    let mut harness = Harness::new();
    harness.state_mut().sync_now(false);
    assert!(
        harness.state().sync_error.is_some(),
        "自动同步失败应记录错误"
    );
    harness.state_mut().status_since = Instant::now() - Duration::from_secs(10);
    let status = harness.state().status();
    assert!(
        status.contains("同步失败"),
        "提示淡出后应显示离线状态，实际: {status}"
    );
}

#[test]
fn failed_edit_commit_keeps_draft() {
    let mut harness = Harness::new();
    {
        let state = harness.state_mut();
        state.comments.push(Comment {
            id: "c1".to_string(),
            target_kind: CommentTargetKind::Block,
            target_id: "b1".to_string(),
            body: "原内容".to_string(),
            resolved: false,
            author_id: "u1".to_string(),
            author_name: "Guan".to_string(),
            version: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted: false,
        });
        state.editing = Some(Editing {
            target: EditTarget::CommentBody {
                comment_id: "c1".to_string(),
                base_version: 1,
            },
            buffer: "我的草稿".to_string(),
            cursor: 12,
            multiline: true,
        });
    }
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    let editing = harness
        .state()
        .editing
        .as_ref()
        .expect("保存失败后应保留编辑状态");
    assert_eq!(editing.buffer, "我的草稿", "草稿内容不应丢失");
    assert!(
        harness.state().status.contains("内容已保留"),
        "应提示草稿已保留，实际: {}",
        harness.state().status
    );
}

#[test]
fn failed_day_ensure_allows_retry() {
    let mut harness = Harness::new();
    harness.state_mut().ensure_today_page();
    assert!(
        harness.state().ensured_date.is_none(),
        "创建今日页失败后应允许下一轮重试"
    );
    assert!(
        harness.state().status.contains("创建今日页失败"),
        "应提示创建失败，实际: {}",
        harness.state().status
    );
}

#[test]
fn batch_forms_report_failure_without_fake_success() {
    let mut harness = Harness::new();
    let today = harness.state().today_key();
    let mut form = EventForm {
        kind: BlockKind::Todo,
        title: "任务".to_string(),
        date: today.clone(),
        start_time: String::new(),
        end_time: String::new(),
        field: 0,
        error: None,
        range_end: Some(today.clone()),
    };
    let message = harness
        .state_mut()
        .submit_event_form(&mut form)
        .expect("离线创建应返回错误");
    assert!(
        message.contains("已创建"),
        "错误信息应包含已创建数量: {message}"
    );
    assert_eq!(form.date, today, "失败后应从失败日期继续");

    let habit = HabitForm {
        title: "习惯".to_string(),
        note: String::new(),
        frequency: HabitFrequency::Daily,
        field: 0,
        error: None,
        date: today,
        range_end: None,
    };
    let err = harness
        .state_mut()
        .submit_habit_form(&habit)
        .expect("离线创建习惯应返回错误");
    assert!(!err.is_empty());
    assert!(
        !harness.state().status.contains("习惯已创建"),
        "失败时不应显示习惯创建成功"
    );
}

#[test]
fn space_on_checked_habit_attempts_undo() {
    let mut harness = Harness::new();
    {
        let state = harness.state_mut();
        let date = state.today_key();
        state.checkins.push(totion_core::HabitCheckin {
            id: "c1".to_string(),
            habit_id: "h1".to_string(),
            date,
            note: String::new(),
            version: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            updated_by: "Guan".to_string(),
            deleted: false,
        });
        state.today_focus = TodayFocus::Habits;
        state.selected_today_habit_index = 0;
        state.clamp();
    }
    harness.state_mut().toggle_today_item();
    let status = &harness.state().status;
    assert!(
        !status.contains("已经打过卡"),
        "已打卡时再按一次应尝试撤销，实际: {status}"
    );
}

#[test]
fn week_view_footer_shows_week_totals() {
    let mut harness = Harness::new();
    harness.state_mut().open_week_view();
    harness.render(120, 40);
    let text = buffer_text(harness.buffer.as_ref().expect("buffer"));
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    assert!(
        compact.contains("本周0/2"),
        "周视图底部应有本周统计: {compact}"
    );
}

#[test]
fn habits_view_can_show_archived_and_block_checkin() {
    let mut harness = Harness::new();
    {
        let state = harness.state_mut();
        state.habits.push(Habit {
            id: "h2".to_string(),
            title: "旧习惯".to_string(),
            icon: "🌱".to_string(),
            note: String::new(),
            frequency: HabitFrequency::Daily,
            position: 1,
            archived: true,
            version: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            updated_by: "Guan".to_string(),
            deleted: false,
            owner_id: "u1".to_string(),
        });
        state.set_view(View::Habits);
        state.clamp();
    }
    assert_eq!(harness.state().habits_list().len(), 1);
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::NONE));
    assert!(harness.state().show_archived_habits());
    assert_eq!(harness.state().habits_list().len(), 2);

    harness.state_mut().selected_habit_index = 1;
    harness.state_mut().check_in_selected_habit();
    assert!(
        harness.state().status.contains("不能打卡"),
        "已归档习惯不应能打卡，实际: {}",
        harness.state().status
    );
    harness.render(120, 24);
    let text = buffer_text(harness.buffer.as_ref().expect("buffer"));
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    assert!(
        compact.contains("已归档"),
        "列表应标记已归档习惯: {compact}"
    );
}

#[test]
fn opening_members_works_offline_with_cached_users() {
    let mut harness = Harness::new();
    harness.state_mut().open_members();
    assert!(matches!(harness.state().modal(), Some(Modal::Members(_))));
    assert_eq!(harness.state().users().len(), 2);
}

#[test]
fn home_and_end_jump_lists() {
    let mut harness = Harness::new();
    harness.press(KeyCode::End);
    assert_eq!(harness.state().selected_today_index(), 1);
    harness.press(KeyCode::Home);
    assert_eq!(harness.state().selected_today_index(), 0);

    harness.state_mut().modal = Some(Modal::Members(MemberPanel { selected: 0 }));
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    match harness.state().modal() {
        Some(Modal::Members(panel)) => assert_eq!(panel.selected, 2),
        _ => panic!("成员面板应仍然打开"),
    }
    harness
        .app
        .handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
    match harness.state().modal() {
        Some(Modal::Members(panel)) => assert_eq!(panel.selected, 0),
        _ => panic!("成员面板应仍然打开"),
    }
}

#[test]
fn comments_work_when_summary_panel_focused() {
    let mut harness = Harness::new();
    {
        let state = harness.state_mut();
        state.today_focus = TodayFocus::Journal;
        state.journal_source = TodayFocus::Items;
        state.selected_today_index = 0;
        state.open_comments();
    }
    assert!(
        matches!(harness.state().modal(), Some(Modal::Comments(_))),
        "总结面板聚焦时应能评论当前待办"
    );
}

#[test]
fn calendar_footer_shows_selected_day_and_count() {
    let mut harness = Harness::new();
    harness.state_mut().set_view(View::Calendar);
    harness.render(120, 40);
    let text = buffer_text(harness.buffer.as_ref().expect("buffer"));
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    assert!(
        compact.contains("2条待办"),
        "日历底部应显示选中日期的待办数: {compact}"
    );
}

#[test]
fn stats_shows_empty_data_hint() {
    let mut harness = Harness::new();
    harness.state_mut().blocks.clear();
    harness.state_mut().set_view(View::Stats);
    harness.render(120, 40);
    let text = buffer_text(harness.buffer.as_ref().expect("buffer"));
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
    assert!(
        compact.contains("暂无数据"),
        "没有待办时统计图应提示暂无数据: {compact}"
    );
}

#[test]
fn switching_views_clears_transient_modes() {
    let mut harness = Harness::new();
    harness.state_mut().open_week_view();
    assert!(harness.state().week_mode());
    harness.state_mut().set_view(View::Calendar);
    assert!(!harness.state().week_mode());
}
