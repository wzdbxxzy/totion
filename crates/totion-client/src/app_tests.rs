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
fn switching_views_clears_transient_modes() {
    let mut harness = Harness::new();
    harness.state_mut().open_week_view();
    assert!(harness.state().week_mode());
    harness.state_mut().set_view(View::Calendar);
    assert!(!harness.state().week_mode());
}
