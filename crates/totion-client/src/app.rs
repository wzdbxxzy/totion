use std::cell::RefCell;
use std::time::{Duration, Instant};

use chrono::{Datelike, Duration as ChronoDuration, Local, Months, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use totion_core::{
    block_day_stats, checkin_on, completion_series, habit_check_in, habit_due_on, habit_stats,
    AuthResponse, Block, BlockKind, Change, CheckinRequest, Comment, CommentTargetKind,
    CreateBlockRequest, CreateCommentRequest, CreateHabitRequest, CreateInviteRequest,
    CreateJournalRequest, CreatePageRequest, DayStat, EntityKind, Habit, HabitCheckin,
    HabitFrequency, HabitStats, Invite, JournalEntry, JournalTarget, Page, PageKind,
    RegisterRequest, Snapshot, UpdateBlockRequest, UpdateCheckinRequest, UpdateCommentRequest,
    UpdateHabitRequest, UpdateInviteRequest, UpdateJournalRequest, UpdatePageRequest, User,
    VerifyRequest,
};

use crate::api::{Api, ApiError};
use crate::config::{self, Identity};

pub const SYNC_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Pages,
    Blocks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Today,
    Workspace,
    Calendar,
    Habits,
    Stats,
    Inbox,
}

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Today => "今日",
            View::Workspace => "工作区",
            View::Calendar => "日历",
            View::Habits => "习惯",
            View::Stats => "统计",
            View::Inbox => "留言",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerCaps {
    pub api_version: u32,
}

impl ServerCaps {
    pub fn supports_v2(self) -> bool {
        self.api_version >= 2
    }
}

#[derive(Debug, Clone)]
pub enum EditTarget {
    PageTitle {
        page_id: String,
        base_version: i64,
    },
    BlockText {
        block_id: String,
        base_version: i64,
    },
    HabitTitle {
        habit_id: String,
        base_version: i64,
    },
    NewComment {
        target_kind: CommentTargetKind,
        target_id: String,
    },
    CommentBody {
        comment_id: String,
        base_version: i64,
    },
    JournalBody {
        journal_id: String,
        base_version: i64,
    },
    NewJournal {
        target_kind: JournalTarget,
        target_id: String,
        date: String,
        time: String,
    },
}

#[derive(Debug, Clone)]
pub struct Editing {
    pub target: EditTarget,
    pub buffer: String,
    pub cursor: usize,
    pub multiline: bool,
}

#[derive(Debug, Clone)]
pub struct EventForm {
    pub kind: BlockKind,
    pub title: String,
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    pub field: usize,
    pub error: Option<String>,
    /// 批量创建时的结束日期（含）
    pub range_end: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HabitForm {
    pub title: String,
    pub note: String,
    pub frequency: HabitFrequency,
    pub field: usize,
    pub error: Option<String>,
    /// 批量创建/打卡的起始日期
    pub date: String,
    /// 批量创建/打卡的结束日期（含）
    pub range_end: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CommentPanel {
    pub target_kind: CommentTargetKind,
    pub target_id: String,
    pub label: String,
    pub selected: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchHit {
    Page(String),
    Block { page_id: String, block_id: String },
    Habit(String),
}

#[derive(Debug, Clone)]
pub struct SearchState {
    pub query: String,
    pub results: Vec<SearchHit>,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub struct AgendaPanel {
    pub date: NaiveDate,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub struct InvitePanel {
    pub invites: Vec<Invite>,
    pub selected: usize,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub enum PendingDelete {
    Block {
        id: String,
        version: i64,
        label: String,
    },
    Habit {
        id: String,
        version: i64,
        label: String,
    },
    Page {
        id: String,
        version: i64,
        label: String,
    },
    Comment {
        id: String,
        version: i64,
    },
    Summary {
        id: String,
        version: i64,
    },
}

#[derive(Debug, Clone)]
pub struct ConfirmState {
    pub action: PendingDelete,
    pub input: String,
    pub label: String,
}

impl PendingDelete {
    pub fn label(&self) -> String {
        match self {
            PendingDelete::Block { label, .. }
            | PendingDelete::Habit { label, .. }
            | PendingDelete::Page { label, .. } => label.clone(),
            PendingDelete::Comment { .. } => "这条评论".to_string(),
            PendingDelete::Summary { .. } => "这条总结".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemberPanel {
    /// 0 = 共享视角，i+1 = users[i]
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub enum Modal {
    EventForm(EventForm),
    HabitForm(HabitForm),
    Comments(CommentPanel),
    Search(SearchState),
    Agenda(AgendaPanel),
    Invites(InvitePanel),
    Members(MemberPanel),
    Confirm(ConfirmState),
    Help,
}

#[derive(Debug, Clone)]
pub enum TodayRow {
    Todo(Block),
    Event(Block),
    Habit(Habit, HabitStats),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodayFocus {
    Items,
    Habits,
    Journal,
}

/// 鼠标点击目标；由绘制阶段登记，点击时反查。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseAction {
    SwitchView(View),
    FocusToday(TodayFocus),
    SelectTodayItem(usize),
    SelectTodayHabit(usize),
    SelectPage(usize),
    SelectBlock(usize),
    SelectHabit(usize),
    SelectInbox(usize),
    OpenJournal(usize),
    WeekSelect(usize),
    PrevDay,
    NextDay,
    PrevMonth,
    NextMonth,
    OpenWeek,
    OpenMembers,
    OpenInvites,
    OpenSearch,
    OpenHelp,
    SyncNow,
}

pub struct SetupForm {
    pub server_url: String,
    pub name: String,
    pub invite_code: String,
    pub field: usize,
    pub error: Option<String>,
    pub identity: Option<Identity>,
}

impl Default for SetupForm {
    fn default() -> Self {
        Self {
            server_url: "http://127.0.0.1:8080".to_string(),
            name: String::new(),
            invite_code: String::new(),
            field: 0,
            error: None,
            identity: None,
        }
    }
}

impl SetupForm {
    fn from_identity(identity: Identity, error: String) -> Self {
        Self {
            server_url: identity.server_url.clone(),
            name: identity.name.clone(),
            invite_code: String::new(),
            field: 2,
            error: Some(error),
            identity: Some(identity),
        }
    }

    fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    pub fn fields(&self) -> [(&'static str, &str); 3] {
        [
            ("服务器地址", &self.server_url),
            ("名字", &self.name),
            ("邀请码", &self.invite_code),
        ]
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Screen> {
        match key.code {
            KeyCode::Tab | KeyCode::Down => {
                self.field = (self.field + 1) % 3;
                None
            }
            KeyCode::Up => {
                self.field = (self.field + 2) % 3;
                None
            }
            KeyCode::Backspace => {
                self.current_field_mut().pop();
                None
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.identity = None;
                self.error = Some("已清除旧密钥，提交后将生成新密钥注册".to_string());
                None
            }
            KeyCode::Char(c) => {
                self.current_field_mut().push(c);
                None
            }
            KeyCode::Enter => self.submit(),
            _ => None,
        }
    }

    fn current_field_mut(&mut self) -> &mut String {
        match self.field {
            0 => &mut self.server_url,
            1 => &mut self.name,
            _ => &mut self.invite_code,
        }
    }

    fn submit(&mut self) -> Option<Screen> {
        self.error = None;
        let server_url = self.server_url.trim().to_string();
        let name = self.name.trim().to_string();
        if server_url.is_empty() || name.is_empty() {
            self.error = Some("服务器地址和名字不能为空".to_string());
            return None;
        }

        let mut identity = match &self.identity {
            Some(existing) => {
                let mut identity = existing.clone();
                identity.server_url = server_url.clone();
                identity.name = name.clone();
                identity
            }
            None => Identity::generate(&server_url, &name),
        };

        let mut api = Api::new(&server_url);
        let auth: Result<AuthResponse, ApiError> = if self.identity.is_some() {
            login(&api, &identity)
        } else {
            api.register(&RegisterRequest {
                invite_code: self.invite_code.trim().to_string(),
                name: name.clone(),
                public_key: identity.public_key.clone(),
            })
        };

        let auth = match auth {
            Ok(auth) => auth,
            Err(err) => {
                self.error = Some(err.to_string());
                return None;
            }
        };

        identity.user_id = auth.user.id.clone();
        identity.token = Some(auth.token.clone());
        if let Err(err) = config::save_identity(&identity) {
            self.error = Some(format!("保存身份失败: {err}"));
            return None;
        }

        api.set_token(Some(auth.token));
        match api.snapshot() {
            Ok(snapshot) => Some(Screen::Main(MainState::new(
                api,
                name,
                identity.user_id.clone(),
                snapshot,
            ))),
            Err(err) => {
                self.error = Some(format!("拉取数据失败: {err}"));
                None
            }
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub enum Screen {
    Setup(SetupForm),
    Main(MainState),
}

pub struct App {
    pub screen: Screen,
    pub should_quit: bool,
}

impl App {
    /// 是否需要输入法（编辑文本或首次连接界面）。
    pub fn needs_ime(&self) -> bool {
        match &self.screen {
            Screen::Setup(_) => true,
            Screen::Main(main) => main.editing().is_some(),
        }
    }

    pub fn boot() -> Self {
        let screen = match config::load_identity() {
            Ok(Some(identity)) => {
                let mut api = Api::new(&identity.server_url);
                match login(&api, &identity) {
                    Ok(auth) => {
                        api.set_token(Some(auth.token.clone()));
                        match api.snapshot() {
                            Ok(snapshot) => Screen::Main(MainState::new(
                                api,
                                identity.name.clone(),
                                identity.user_id.clone(),
                                snapshot,
                            )),
                            Err(err) => Screen::Setup(SetupForm::from_identity(
                                identity,
                                format!("拉取数据失败: {err}"),
                            )),
                        }
                    }
                    Err(err) => Screen::Setup(SetupForm::from_identity(
                        identity,
                        format!("自动登录失败: {err}（Ctrl+R 可换新密钥重新注册）"),
                    )),
                }
            }
            Ok(None) => Screen::Setup(SetupForm::default()),
            Err(err) => {
                Screen::Setup(SetupForm::default().with_error(format!("读取配置失败: {err}")))
            }
        };

        Self {
            screen,
            should_quit: false,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != crossterm::event::KeyEventKind::Press {
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        match &mut self.screen {
            Screen::Setup(form) => {
                if let Some(screen) = form.handle_key(key) {
                    self.screen = screen;
                }
            }
            Screen::Main(main) => {
                if main.handle_key(key) {
                    self.should_quit = true;
                }
            }
        }
    }

    pub fn tick(&mut self) {
        if let Screen::Main(main) = &mut self.screen {
            main.tick();
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        // 滚轮复用上下键：列表/弹窗里滚动即移动选中项（lazygit/yazi 风格）。
        let wheel = match mouse.kind {
            MouseEventKind::ScrollUp => Some(KeyCode::Up),
            MouseEventKind::ScrollDown => Some(KeyCode::Down),
            _ => None,
        };
        if let Some(code) = wheel {
            self.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
            return;
        }
        if let Screen::Main(main) = &mut self.screen {
            main.handle_mouse(mouse);
        }
    }
}

fn login(api: &Api, identity: &Identity) -> Result<AuthResponse, ApiError> {
    let challenge = api.challenge(&identity.public_key)?;
    let signature = identity
        .sign(challenge.nonce.as_bytes())
        .map_err(|err| ApiError::Other(format!("签名失败: {err}")))?;
    api.verify(&VerifyRequest {
        challenge_id: challenge.challenge_id,
        signature,
    })
}

pub struct MainState {
    api: Api,
    me: String,
    me_id: String,
    /// 成员视角：Some(user_id) 时 1-6 界面只显示该成员的数据（他人视角只读）。
    view_user: Option<String>,
    workspace_name: String,
    users: Vec<User>,
    pub pages: Vec<Page>,
    pub blocks: Vec<Block>,
    pub habits: Vec<Habit>,
    pub checkins: Vec<HabitCheckin>,
    pub comments: Vec<Comment>,
    pub journals: Vec<JournalEntry>,
    last_seq: i64,
    selected_page_index: usize,
    selected_block_index: usize,
    selected_today_index: usize,
    selected_today_habit_index: usize,
    selected_habit_index: usize,
    selected_agenda_index: usize,
    selected_inbox_index: usize,
    focus: Focus,
    today_focus: TodayFocus,
    /// 从哪个面板进入「总结」面板，决定总结跟随待办还是习惯。
    journal_source: TodayFocus,
    view: View,
    editing: Option<Editing>,
    modal: Option<Modal>,
    status: String,
    /// 上一条已展示过的状态，用来判断状态是否变化。
    status_seen: String,
    status_since: Instant,
    notice: Option<String>,
    last_sync: Option<Instant>,
    calendar_month: NaiveDate,
    selected_day: NaiveDate,
    today: NaiveDate,
    view_date: NaiveDate,
    week_mode: bool,
    /// 周视图窗口的最后一天（含）
    week_end: NaiveDate,
    week_selected: usize,
    ensured_date: Option<String>,
    daily_page_id: Option<String>,
    show_all_pages: bool,
    workspace_journal_mode: bool,
    selected_journal_index: usize,
    range_anchor: Option<NaiveDate>,
    range_focus: Option<NaiveDate>,
    pub calendar_cells: RefCell<Vec<(Rect, NaiveDate)>>,
    mouse_hits: RefCell<Vec<(Rect, MouseAction)>>,
    /// 上一次左键点击的时间与坐标，用于识别双击。
    last_click: Option<(Instant, u16, u16)>,
    caps: ServerCaps,
    stats_days: u32,
}

impl MainState {
    fn new(api: Api, me: String, me_id: String, snapshot: Snapshot) -> Self {
        let today = Local::now().date_naive();
        let mut state = Self {
            api,
            me,
            me_id,
            view_user: None,
            workspace_name: snapshot.workspace.name,
            users: snapshot.workspace.users,
            pages: snapshot.pages,
            blocks: snapshot.blocks,
            habits: snapshot.habits,
            checkins: snapshot.checkins,
            comments: snapshot.comments,
            journals: snapshot.journals,
            last_seq: snapshot.last_seq,
            selected_page_index: 0,
            selected_block_index: 0,
            selected_today_index: 0,
            selected_today_habit_index: 0,
            selected_habit_index: 0,
            selected_agenda_index: 0,
            selected_inbox_index: 0,
            focus: Focus::Pages,
            today_focus: TodayFocus::Items,
            journal_source: TodayFocus::Items,
            view: View::Today,
            editing: None,
            modal: None,
            status: "已连接".to_string(),
            status_seen: "已连接".to_string(),
            status_since: Instant::now(),
            notice: None,
            last_sync: Some(Instant::now()),
            calendar_month: today.with_day(1).unwrap_or(today),
            selected_day: today,
            today,
            view_date: today,
            week_mode: false,
            week_end: today,
            week_selected: 6,
            ensured_date: None,
            daily_page_id: None,
            show_all_pages: false,
            workspace_journal_mode: false,
            selected_journal_index: 0,
            range_anchor: None,
            range_focus: None,
            calendar_cells: RefCell::new(Vec::new()),
            mouse_hits: RefCell::new(Vec::new()),
            last_click: None,
            caps: ServerCaps {
                api_version: snapshot.api_version,
            },
            stats_days: 14,
        };
        state.refresh_daily_page();
        state.clamp();
        state
    }

    pub fn workspace_name(&self) -> &str {
        &self.workspace_name
    }

    pub fn me(&self) -> &str {
        &self.me
    }

    pub fn users(&self) -> &[User] {
        &self.users
    }

    /// 正在查看的成员（None = 共享视角）。
    pub fn viewed_user(&self) -> Option<&User> {
        let id = self.view_user.as_deref()?;
        self.users.iter().find(|user| user.id == id)
    }

    /// 是否处于他人视角（只读）。
    pub fn read_only(&self) -> bool {
        matches!(&self.view_user, Some(id) if id != &self.me_id)
    }

    fn owner_visible(&self, owner_id: &str) -> bool {
        match &self.view_user {
            Some(id) => owner_id == id,
            None => true,
        }
    }

    fn author_visible(&self, author_id: &str) -> bool {
        match &self.view_user {
            Some(id) => author_id == id,
            None => true,
        }
    }

    fn scoped_blocks(&self) -> Vec<Block> {
        self.blocks
            .iter()
            .filter(|block| !block.deleted && self.owner_visible(&block.owner_id))
            .cloned()
            .collect()
    }

    fn scoped_checkins(&self) -> Vec<HabitCheckin> {
        match self.view_user.as_deref() {
            Some(id) => {
                let name = self
                    .users
                    .iter()
                    .find(|user| user.id == id)
                    .map(|user| user.name.as_str())
                    .unwrap_or("");
                self.checkins
                    .iter()
                    .filter(|checkin| !checkin.deleted && checkin.updated_by == name)
                    .cloned()
                    .collect()
            }
            None => self
                .checkins
                .iter()
                .filter(|checkin| !checkin.deleted)
                .cloned()
                .collect(),
        }
    }

    /// 只读视角下拦截写操作；返回 true 表示可以继续写。
    fn ensure_writable(&mut self) -> bool {
        if self.read_only() {
            let name = self
                .viewed_user()
                .map(|user| user.name.clone())
                .unwrap_or_else(|| "该成员".to_string());
            self.status = format!("正在查看 {name} 的视角，只能评论，不能修改");
            return false;
        }
        true
    }

    /// 状态栏文案：操作提示显示几秒后淡出，回到连接状态（k9s/lazygit 风格）。
    pub fn status(&self) -> String {
        let is_error = ["失败", "错误", "冲突", "无法", "过旧"]
            .iter()
            .any(|word| self.status.contains(word));
        let ttl = if is_error {
            Duration::from_secs(8)
        } else {
            Duration::from_secs(4)
        };
        if self.status_since.elapsed() < ttl {
            return self.status.clone();
        }
        match self.last_sync {
            Some(at) if at.elapsed().as_secs() >= 2 => {
                format!("已连接 · {} 秒前同步", at.elapsed().as_secs())
            }
            _ => "已连接".to_string(),
        }
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub fn focus(&self) -> Focus {
        self.focus
    }

    pub fn view(&self) -> View {
        self.view
    }

    pub fn selected_page_index(&self) -> usize {
        self.selected_page_index
    }

    pub fn selected_block_index(&self) -> usize {
        self.selected_block_index
    }

    pub fn selected_today_index(&self) -> usize {
        self.selected_today_index
    }

    pub fn selected_today_habit_index(&self) -> usize {
        self.selected_today_habit_index
    }

    pub fn today_focus(&self) -> TodayFocus {
        self.today_focus
    }

    pub fn selected_habit_index(&self) -> usize {
        self.selected_habit_index
    }

    pub fn editing(&self) -> Option<&Editing> {
        self.editing.as_ref()
    }

    pub fn modal(&self) -> Option<&Modal> {
        self.modal.as_ref()
    }

    pub fn calendar_month(&self) -> NaiveDate {
        self.calendar_month
    }

    pub fn selected_day(&self) -> NaiveDate {
        self.selected_day
    }

    pub fn today(&self) -> NaiveDate {
        self.today
    }

    pub fn stats_days(&self) -> u32 {
        self.stats_days
    }

    pub fn show_all_pages(&self) -> bool {
        self.show_all_pages
    }

    pub fn range_is_active(&self) -> bool {
        self.range_anchor.is_some()
    }

    pub fn selected_inbox_index(&self) -> usize {
        self.selected_inbox_index
    }

    pub fn supports_v2(&self) -> bool {
        self.caps.api_version >= 3
    }

    pub fn supports_member_view(&self) -> bool {
        self.caps.api_version >= 5
    }

    pub fn visible_pages(&self) -> Vec<&Page> {
        let mut pages: Vec<&Page> = self
            .pages
            .iter()
            .filter(|page| !page.deleted && (self.show_all_pages || !page.is_daily()))
            .collect();
        pages.sort_by_key(|page| (page.position, page.updated_at));
        pages
    }

    pub fn visible_blocks(&self) -> Vec<&Block> {
        let Some(page_id) = self.current_page_id() else {
            return Vec::new();
        };
        let mut blocks: Vec<&Block> = self
            .blocks
            .iter()
            .filter(|block| {
                !block.deleted && block.page_id == page_id && self.owner_visible(&block.owner_id)
            })
            .collect();
        blocks.sort_by_key(|block| (block.position, block.updated_at));
        blocks
    }

    pub fn calendar_blocks(&self, date: NaiveDate) -> Vec<&Block> {
        let key = date.format("%Y-%m-%d").to_string();
        let mut blocks: Vec<&Block> = self
            .blocks
            .iter()
            .filter(|block| {
                !block.deleted
                    && block.due_date.as_deref() == Some(key.as_str())
                    && matches!(block.kind, BlockKind::Todo | BlockKind::CalendarEvent)
                    && self.owner_visible(&block.owner_id)
            })
            .collect();
        blocks.sort_by_key(|block| {
            (
                block
                    .start_time
                    .clone()
                    .unwrap_or_else(|| "99:99".to_string()),
                block.position,
            )
        });
        blocks
    }

    pub fn agenda_blocks(&self) -> Vec<Block> {
        self.calendar_blocks(self.selected_day)
            .into_iter()
            .cloned()
            .collect()
    }

    pub fn visible_habits(&self) -> Vec<Habit> {
        let mut habits: Vec<Habit> = self
            .habits
            .iter()
            .filter(|habit| {
                !habit.deleted && !habit.archived && self.owner_visible(&habit.owner_id)
            })
            .cloned()
            .collect();
        habits.sort_by_key(|habit| (habit.position, habit.created_at));
        habits
    }

    pub fn habit_stats(&self, habit: &Habit) -> HabitStats {
        habit_stats(
            habit.frequency,
            &habit.id,
            &self.scoped_checkins(),
            self.today,
        )
    }

    pub fn today_key(&self) -> String {
        self.today.format("%Y-%m-%d").to_string()
    }

    pub fn view_key(&self) -> String {
        self.view_date.format("%Y-%m-%d").to_string()
    }

    pub fn view_date(&self) -> NaiveDate {
        self.view_date
    }

    pub fn week_mode(&self) -> bool {
        self.week_mode
    }

    pub fn week_selected(&self) -> usize {
        self.week_selected
    }

    pub fn week_end(&self) -> NaiveDate {
        self.week_end
    }

    pub fn clear_mouse_hits(&self) {
        self.mouse_hits.borrow_mut().clear();
    }

    /// 绘制时登记一块可点击区域；后登记的优先级更高。
    pub fn push_mouse_hit(&self, rect: Rect, action: MouseAction) {
        if rect.width > 0 && rect.height > 0 {
            self.mouse_hits.borrow_mut().push((rect, action));
        }
    }

    fn mouse_hit_at(&self, column: u16, row: u16) -> Option<MouseAction> {
        self.mouse_hits
            .borrow()
            .iter()
            .rev()
            .find(|(rect, _)| {
                column >= rect.x
                    && column < rect.x.saturating_add(rect.width)
                    && row >= rect.y
                    && row < rect.y.saturating_add(rect.height)
            })
            .map(|(_, action)| *action)
    }

    /// 周视图：每天返回完成统计和当天的待办列表（不包含总结/评论）。
    pub fn week_overview(&self) -> Vec<(NaiveDate, DayStat, Vec<Block>)> {
        let blocks = self.scoped_blocks();
        (0..7)
            .map(|index| {
                let date = self.week_end - ChronoDuration::days(6 - index);
                let key = date.format("%Y-%m-%d").to_string();
                let stat = block_day_stats(&blocks, &key);
                let mut day_blocks: Vec<Block> = blocks
                    .iter()
                    .filter(|block| {
                        block.due_date.as_deref() == Some(key.as_str())
                            && matches!(block.kind, BlockKind::Todo | BlockKind::CalendarEvent)
                    })
                    .cloned()
                    .collect();
                day_blocks.sort_by_key(|block| {
                    (
                        block
                            .start_time
                            .clone()
                            .unwrap_or_else(|| "99:99".to_string()),
                        block.position,
                    )
                });
                (date, stat, day_blocks)
            })
            .collect()
    }

    pub fn workspace_journal_mode(&self) -> bool {
        self.workspace_journal_mode
    }

    pub fn selected_journal_index(&self) -> usize {
        self.selected_journal_index
    }

    pub fn habit_stats_on(&self, habit: &Habit, date: NaiveDate) -> HabitStats {
        habit_stats(habit.frequency, &habit.id, &self.scoped_checkins(), date)
    }

    pub fn daily_page(&self) -> Option<&Page> {
        let key = self.today_key();
        self.pages
            .iter()
            .find(|page| !page.deleted && page.date.as_deref() == Some(key.as_str()))
    }

    pub fn today_rows(&self) -> Vec<TodayRow> {
        let key = self.view_key();
        let mut rows = Vec::new();
        let mut blocks: Vec<Block> = self
            .blocks
            .iter()
            .filter(|block| {
                !block.deleted
                    && block.due_date.as_deref() == Some(key.as_str())
                    && matches!(block.kind, BlockKind::Todo | BlockKind::CalendarEvent)
                    && self.owner_visible(&block.owner_id)
            })
            .cloned()
            .collect();
        blocks.sort_by_key(|block| {
            (
                block
                    .start_time
                    .clone()
                    .unwrap_or_else(|| "99:99".to_string()),
                block.position,
            )
        });
        for block in blocks {
            if block.kind == BlockKind::CalendarEvent {
                rows.push(TodayRow::Event(block));
            } else {
                rows.push(TodayRow::Todo(block));
            }
        }

        for habit in self.visible_habits() {
            let stats = self.habit_stats_on(&habit, self.view_date);
            if stats.checked_today
                || habit_due_on(habit.frequency, self.view_date, stats.weekly_done)
            {
                rows.push(TodayRow::Habit(habit, stats));
            }
        }
        rows
    }

    pub fn today_items(&self) -> Vec<TodayRow> {
        self.today_rows()
            .into_iter()
            .filter(|row| !matches!(row, TodayRow::Habit(_, _)))
            .collect()
    }

    pub fn today_habits(&self) -> Vec<(Habit, HabitStats)> {
        self.today_rows()
            .into_iter()
            .filter_map(|row| match row {
                TodayRow::Habit(habit, stats) => Some((habit, stats)),
                _ => None,
            })
            .collect()
    }

    pub fn all_journals(&self) -> Vec<JournalEntry> {
        let mut journals: Vec<JournalEntry> = self
            .journals
            .iter()
            .filter(|journal| !journal.deleted && self.author_visible(&journal.author_id))
            .cloned()
            .collect();
        journals.sort_by_key(|journal| {
            (
                std::cmp::Reverse(journal.date.clone()),
                std::cmp::Reverse(journal.created_at),
            )
        });
        journals
    }

    pub fn journals_for_target(&self, kind: JournalTarget, target_id: &str) -> Vec<JournalEntry> {
        let mut journals: Vec<JournalEntry> = self
            .journals
            .iter()
            .filter(|journal| {
                !journal.deleted
                    && journal.target_kind == kind
                    && journal.target_id == target_id
                    && self.author_visible(&journal.author_id)
            })
            .cloned()
            .collect();
        journals.sort_by_key(|journal| std::cmp::Reverse(journal.created_at));
        journals
    }

    /// 当前选中的待办/习惯；总结面板跟随它。
    pub fn current_summary_target(&self) -> Option<(JournalTarget, String)> {
        match self.today_focus {
            TodayFocus::Items => {
                self.today_items()
                    .get(self.selected_today_index)
                    .and_then(|row| match row {
                        TodayRow::Todo(block) | TodayRow::Event(block) => {
                            Some((JournalTarget::Block, block.id.clone()))
                        }
                        TodayRow::Habit(_, _) => None,
                    })
            }
            TodayFocus::Habits => self
                .today_habits()
                .get(self.selected_today_habit_index)
                .map(|(habit, _)| (JournalTarget::Habit, habit.id.clone())),
            // 总结面板跟随「待办 / 习惯」列表里当前高亮的对象；没有选中对象时返回 None，
            // 面板隐藏。不再回退到「当日总结」，避免显示不是当前对象的内容。
            TodayFocus::Journal => match self.journal_source {
                TodayFocus::Habits => self
                    .today_habits()
                    .get(self.selected_today_habit_index)
                    .map(|(habit, _)| (JournalTarget::Habit, habit.id.clone())),
                _ => self
                    .today_items()
                    .get(self.selected_today_index)
                    .and_then(|row| match row {
                        TodayRow::Todo(block) | TodayRow::Event(block) => {
                            Some((JournalTarget::Block, block.id.clone()))
                        }
                        TodayRow::Habit(_, _) => None,
                    }),
            },
        }
    }

    pub fn today_summary_entries(&self) -> Vec<JournalEntry> {
        let key = self.view_key();
        self.current_summary_target()
            .map(|(kind, id)| {
                self.journals_for_target(kind, &id)
                    .into_iter()
                    .filter(|journal| journal.date == key)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn inbox_comments(&self) -> Vec<Comment> {
        let mut comments: Vec<Comment> = self
            .comments
            .iter()
            .filter(|comment| !comment.deleted && self.author_visible(&comment.author_id))
            .cloned()
            .collect();
        comments.sort_by_key(|comment| (comment.resolved, std::cmp::Reverse(comment.updated_at)));
        comments
    }

    /// 总结挂载对象的可读名称，用于工作区汇总列表。
    pub fn journal_target_label(&self, kind: JournalTarget, id: &str) -> String {
        match kind {
            JournalTarget::Daily => "当日总结".to_string(),
            JournalTarget::Block => self
                .blocks
                .iter()
                .find(|block| block.id == id && !block.deleted)
                .map(|block| {
                    if block.text.is_empty() {
                        block.kind.icon().to_string()
                    } else {
                        block.text.clone()
                    }
                })
                .unwrap_or_else(|| "已删除待办".to_string()),
            JournalTarget::Habit => self
                .habits
                .iter()
                .find(|habit| habit.id == id)
                .map(|habit| habit.title.clone())
                .unwrap_or_else(|| "已删除习惯".to_string()),
        }
    }

    pub fn comment_target_label(&self, kind: CommentTargetKind, id: &str) -> String {
        match kind {
            CommentTargetKind::Page => self
                .pages
                .iter()
                .find(|page| page.id == id)
                .map(|page| format!("页面「{}」", page.title))
                .unwrap_or_else(|| "已删除页面".to_string()),
            CommentTargetKind::Block => self
                .blocks
                .iter()
                .find(|block| block.id == id)
                .map(|block| {
                    format!(
                        "内容块「{}」",
                        if block.text.is_empty() {
                            block.kind.icon()
                        } else {
                            block.text.as_str()
                        }
                    )
                })
                .unwrap_or_else(|| "已删除内容块".to_string()),
            CommentTargetKind::Habit => self
                .habits
                .iter()
                .find(|habit| habit.id == id)
                .map(|habit| format!("习惯「{}」", habit.title))
                .unwrap_or_else(|| "已删除习惯".to_string()),
        }
    }

    pub fn today_completion(&self) -> DayStat {
        let mut stat = block_day_stats(&self.scoped_blocks(), &self.view_key());
        for habit in self.visible_habits() {
            let stats = self.habit_stats_on(&habit, self.view_date);
            if habit_due_on(habit.frequency, self.view_date, stats.weekly_done) {
                stat.total += 1;
                if stats.checked_today {
                    stat.done += 1;
                }
            }
        }
        stat
    }

    pub fn completion_series(&self, days: u32) -> Vec<(String, DayStat)> {
        completion_series(&self.scoped_blocks(), days, self.today)
    }

    pub fn heatmap_grid(&self, habit: &Habit, weeks: u32) -> Vec<Vec<bool>> {
        totion_core::heatmap_grid(&self.scoped_checkins(), &habit.id, weeks, self.today)
    }

    pub fn aggregate_heatmap(&self, weeks: u32) -> Vec<Vec<bool>> {
        let habits = self.visible_habits();
        let mut grid = vec![vec![false; 7]; weeks as usize];
        for habit in &habits {
            let single = self.heatmap_grid(habit, weeks);
            for (week, column) in single.iter().enumerate() {
                for (day, hit) in column.iter().enumerate() {
                    if *hit {
                        grid[week][day] = true;
                    }
                }
            }
        }
        grid
    }

    pub fn unresolved_comment_count(&self, kind: CommentTargetKind, id: &str) -> usize {
        self.comments
            .iter()
            .filter(|comment| {
                !comment.deleted
                    && !comment.resolved
                    && comment.target_kind == kind
                    && comment.target_id == id
            })
            .count()
    }

    pub fn comments_for(&self, target_kind: CommentTargetKind, target_id: &str) -> Vec<Comment> {
        let mut comments: Vec<Comment> = self
            .comments
            .iter()
            .filter(|comment| {
                !comment.deleted
                    && comment.target_kind == target_kind
                    && comment.target_id == target_id
            })
            .cloned()
            .collect();
        comments.sort_by_key(|comment| comment.created_at);
        comments
    }

    fn current_page_id(&self) -> Option<String> {
        self.visible_pages()
            .get(self.selected_page_index)
            .map(|page| page.id.clone())
    }

    fn current_page(&self) -> Option<Page> {
        self.visible_pages()
            .get(self.selected_page_index)
            .map(|page| (*page).clone())
    }

    fn current_block(&self) -> Option<Block> {
        self.visible_blocks()
            .get(self.selected_block_index)
            .map(|block| (*block).clone())
    }

    fn clamp(&mut self) {
        let page_count = self.visible_pages().len();
        self.selected_page_index = if page_count == 0 {
            0
        } else {
            self.selected_page_index.min(page_count - 1)
        };
        let block_count = self.visible_blocks().len();
        self.selected_block_index = if block_count == 0 {
            0
        } else {
            self.selected_block_index.min(block_count - 1)
        };
        let today_count = self.today_items().len();
        self.selected_today_index = if today_count == 0 {
            0
        } else {
            self.selected_today_index.min(today_count - 1)
        };
        let today_habit_count = self.today_habits().len();
        self.selected_today_habit_index = if today_habit_count == 0 {
            0
        } else {
            self.selected_today_habit_index.min(today_habit_count - 1)
        };
        let habit_count = self.visible_habits().len();
        self.selected_habit_index = if habit_count == 0 {
            0
        } else {
            self.selected_habit_index.min(habit_count - 1)
        };
        let agenda_count = self.agenda_blocks().len();
        self.selected_agenda_index = if agenda_count == 0 {
            0
        } else {
            self.selected_agenda_index.min(agenda_count - 1)
        };
        let inbox_count = self.inbox_comments().len();
        self.selected_inbox_index = if inbox_count == 0 {
            0
        } else {
            self.selected_inbox_index.min(inbox_count - 1)
        };
        let journal_count = self.all_journals().len();
        self.selected_journal_index = if journal_count == 0 {
            0
        } else {
            self.selected_journal_index.min(journal_count - 1)
        };
    }

    fn refresh_daily_page(&mut self) {
        self.daily_page_id = self.daily_page().map(|page| page.id.clone());
    }

    pub fn tick(&mut self) {
        if self.status != self.status_seen {
            self.status_seen = self.status.clone();
            self.status_since = Instant::now();
        }
        let now = Local::now().date_naive();
        if now != self.today {
            self.today = now;
            self.calendar_month = now.with_day(1).unwrap_or(now);
            self.selected_day = now;
            self.ensured_date = None;
            self.refresh_daily_page();
        }
        if self.caps.supports_v2() && self.ensured_date.as_deref() != Some(&self.today_key()) {
            self.ensure_today_page();
        }
        let due = self
            .last_sync
            .map(|at| at.elapsed() >= SYNC_INTERVAL)
            .unwrap_or(true);
        if due {
            self.sync_now(false);
        }
    }

    fn ensure_today_page(&mut self) {
        // 他人视角只读，不去替自己创建每日页，避免污染数据。
        if self.read_only() {
            return;
        }
        let date = self.today_key();
        if self.ensured_date.as_deref() == Some(date.as_str()) {
            return;
        }
        self.ensured_date = Some(date.clone());
        match self.api.ensure_day(&date) {
            Ok(response) => {
                let page_id = response.page.id.clone();
                self.upsert_page(response.page);
                for block in response.blocks {
                    self.upsert_block(block);
                }
                self.daily_page_id = Some(page_id);
            }
            Err(err) => {
                self.status = format!("创建今日页失败: {err}");
            }
        }
    }

    fn ensure_day(&mut self, date: &str) -> Option<String> {
        if let Some(page) = self
            .pages
            .iter()
            .find(|page| !page.deleted && page.date.as_deref() == Some(date))
        {
            return Some(page.id.clone());
        }
        match self.api.ensure_day(date) {
            Ok(response) => {
                let page_id = response.page.id.clone();
                self.upsert_page(response.page);
                for block in response.blocks {
                    self.upsert_block(block);
                }
                Some(page_id)
            }
            Err(err) => {
                self.status = format!("创建 {date} 页面失败: {err}");
                None
            }
        }
    }

    fn sync_now(&mut self, manual: bool) {
        self.last_sync = Some(Instant::now());
        let mut total = 0;
        let mut rounds = 0;
        loop {
            match self.api.sync(self.last_seq) {
                Ok(response) => {
                    total += response.changes.len();
                    let has_more = response.has_more;
                    for change in response.changes {
                        self.apply_change(change);
                    }
                    let advanced = response.last_seq > self.last_seq;
                    self.last_seq = self.last_seq.max(response.last_seq);
                    rounds += 1;
                    if !has_more || !advanced || rounds >= 20 {
                        break;
                    }
                }
                Err(err) => {
                    if manual {
                        self.status = format!("同步失败: {err}");
                    }
                    break;
                }
            }
        }
        self.refresh_daily_page();
        self.clamp();
        if manual {
            self.status = format!("已同步（{total} 条变更）");
        }
    }

    fn apply_change(&mut self, change: Change) {
        match change.entity {
            EntityKind::Page => {
                if let Ok(page) = serde_json::from_value::<Page>(change.data.clone()) {
                    let title = page.title.clone();
                    let deleted = page.deleted;
                    self.upsert_page(page);
                    if change.actor != self.me {
                        self.notice = Some(format!(
                            "{} {}了页面「{}」",
                            change.actor,
                            if deleted { "删除" } else { "更新" },
                            title
                        ));
                    }
                }
            }
            EntityKind::Block => {
                if let Ok(block) = serde_json::from_value::<Block>(change.data.clone()) {
                    let text = block.text.clone();
                    let deleted = block.deleted;
                    self.upsert_block(block);
                    if change.actor != self.me {
                        self.notice = Some(format!(
                            "{} {}{}",
                            change.actor,
                            if deleted { "删除" } else { "更新" },
                            if text.is_empty() {
                                "内容块".to_string()
                            } else {
                                format!("「{text}」")
                            }
                        ));
                    }
                }
            }
            EntityKind::Habit => {
                if let Ok(habit) = serde_json::from_value::<Habit>(change.data.clone()) {
                    let title = habit.title.clone();
                    self.upsert_habit(habit);
                    if change.actor != self.me {
                        self.notice = Some(format!("{} 更新了习惯「{}」", change.actor, title));
                    }
                }
            }
            EntityKind::HabitCheckin => {
                if let Ok(checkin) = serde_json::from_value::<HabitCheckin>(change.data.clone()) {
                    let date = checkin.date.clone();
                    self.upsert_checkin(checkin);
                    if change.actor != self.me {
                        self.notice = Some(format!("{} 在 {date} 打卡了习惯", change.actor));
                    }
                }
            }
            EntityKind::Comment => {
                if let Ok(comment) = serde_json::from_value::<Comment>(change.data.clone()) {
                    let author = comment.author_name.clone();
                    self.upsert_comment(comment);
                    if change.actor != self.me {
                        self.notice = Some(format!("{author} 发表了评论"));
                    }
                }
            }
            EntityKind::Journal => {
                if let Ok(journal) = serde_json::from_value::<JournalEntry>(change.data.clone()) {
                    let author = journal.author_name.clone();
                    self.upsert_journal(journal);
                    if change.actor != self.me {
                        self.notice = Some(format!("{author} 更新了总结"));
                    }
                }
            }
        }
        self.last_seq = self.last_seq.max(change.seq);
    }

    fn upsert_page(&mut self, page: Page) {
        if let Some(existing) = self.pages.iter_mut().find(|item| item.id == page.id) {
            *existing = page;
        } else {
            self.pages.push(page);
        }
    }

    fn upsert_block(&mut self, block: Block) {
        if let Some(existing) = self.blocks.iter_mut().find(|item| item.id == block.id) {
            *existing = block;
        } else {
            self.blocks.push(block);
        }
    }

    fn upsert_habit(&mut self, habit: Habit) {
        if let Some(existing) = self.habits.iter_mut().find(|item| item.id == habit.id) {
            *existing = habit;
        } else {
            self.habits.push(habit);
        }
    }

    fn upsert_checkin(&mut self, checkin: HabitCheckin) {
        if let Some(existing) = self.checkins.iter_mut().find(|item| item.id == checkin.id) {
            *existing = checkin;
        } else {
            self.checkins.push(checkin);
        }
    }

    fn upsert_comment(&mut self, comment: Comment) {
        if let Some(existing) = self.comments.iter_mut().find(|item| item.id == comment.id) {
            *existing = comment;
        } else {
            self.comments.push(comment);
        }
    }

    fn upsert_journal(&mut self, journal: JournalEntry) {
        if let Some(existing) = self.journals.iter_mut().find(|item| item.id == journal.id) {
            *existing = journal;
        } else {
            self.journals.push(journal);
        }
    }

    fn patch_page(
        &mut self,
        page_id: &str,
        base_version: i64,
        title: Option<String>,
        deleted: Option<bool>,
    ) {
        let request = UpdatePageRequest {
            expected_version: base_version,
            title,
            icon: None,
            kind: None,
            position: None,
            deleted,
            date: None,
        };
        match self.api.update_page(page_id, &request) {
            Ok(response) => {
                self.upsert_page(response.page);
                self.clamp();
            }
            Err(ApiError::Conflict { message, current }) => {
                if let Ok(page) = serde_json::from_value::<Page>(current) {
                    self.upsert_page(page);
                    self.clamp();
                }
                self.status = format!("页面冲突，已刷新：{message}");
            }
            Err(ApiError::Other(message)) => self.status = message,
        }
    }

    fn patch_block(&mut self, block_id: &str, base_version: i64, patch: BlockPatch) {
        let request = UpdateBlockRequest {
            expected_version: base_version,
            text: patch.text,
            checked: patch.checked,
            due_date: patch.due_date,
            streak: patch.streak,
            position: None,
            deleted: patch.deleted,
            start_time: patch.start_time,
            end_time: patch.end_time,
        };
        match self.api.update_block(block_id, &request) {
            Ok(response) => {
                self.upsert_block(response.block);
                self.clamp();
            }
            Err(ApiError::Conflict { message, current }) => {
                if let Ok(block) = serde_json::from_value::<Block>(current) {
                    self.upsert_block(block);
                    self.clamp();
                }
                self.status = format!("内容冲突，已刷新：{message}");
            }
            Err(ApiError::Other(message)) => self.status = message,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if self.editing.is_some() {
            self.handle_edit_key(key);
            return false;
        }
        if self.modal.is_some() {
            self.handle_modal_key(key);
            return false;
        }

        match key.code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('1') => self.set_view(View::Today),
            KeyCode::Char('2') => self.set_view(View::Workspace),
            KeyCode::Char('3') => self.set_view(View::Calendar),
            KeyCode::Char('4') => self.set_view(View::Habits),
            KeyCode::Char('5') => self.set_view(View::Stats),
            KeyCode::Char('6') => self.set_view(View::Inbox),
            KeyCode::Char('?') => self.modal = Some(Modal::Help),
            KeyCode::Char('/') => self.open_search(),
            KeyCode::Char('C') => self.open_comments(),
            KeyCode::Char('i') => self.open_invites(),
            KeyCode::Char('m') => self.open_members(),
            KeyCode::Char('r') => {
                self.ensured_date = None;
                self.ensure_today_page();
                self.sync_now(true);
            }
            KeyCode::Esc => self.notice = None,
            _ => match self.view {
                View::Today => self.handle_today_key(key),
                View::Workspace => self.handle_workspace_key(key),
                View::Calendar => self.handle_calendar_key(key),
                View::Habits => self.handle_habits_key(key),
                View::Stats => self.handle_stats_key(key),
                View::Inbox => self.handle_inbox_key(key),
            },
        }
        false
    }

    fn set_view(&mut self, view: View) {
        self.view = view;
        self.week_mode = false;
        self.workspace_journal_mode = false;
        self.clamp();
    }

    fn handle_workspace_key(&mut self, key: KeyEvent) {
        if self.workspace_journal_mode {
            match key.code {
                KeyCode::Char('J') | KeyCode::Esc => self.workspace_journal_mode = false,
                KeyCode::Up | KeyCode::Char('k') => self.move_journal(-1),
                KeyCode::Down | KeyCode::Char('j') => self.move_journal(1),
                KeyCode::Enter | KeyCode::Char('e') => self.edit_selected_journal(),
                KeyCode::Char('d') => self.delete_selected_journal(),
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Pages => Focus::Blocks,
                    Focus::Blocks => Focus::Pages,
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Enter | KeyCode::Char('e') => self.start_edit(),
            KeyCode::Char('n') => self.new_workspace_block(BlockKind::Text),
            KeyCode::Char('t') => self.new_workspace_block(BlockKind::Todo),
            KeyCode::Char('N') => self.new_page(),
            KeyCode::Char('a') => {
                self.show_all_pages = !self.show_all_pages;
                self.clamp();
            }
            KeyCode::Char('J') => {
                self.workspace_journal_mode = true;
                self.selected_journal_index = 0;
            }
            KeyCode::Char(' ') => self.toggle_workspace_block(),
            KeyCode::Char('d') => self.delete_workspace_current(),
            _ => {}
        }
    }

    fn move_journal(&mut self, delta: i64) {
        let count = self.all_journals().len();
        if count > 0 {
            let next = self.selected_journal_index as i64 + delta;
            self.selected_journal_index = next.clamp(0, count as i64 - 1) as usize;
        }
    }

    fn edit_selected_journal(&mut self) {
        if !self.ensure_writable() {
            return;
        }
        let journals = self.all_journals();
        if let Some(journal) = journals.get(self.selected_journal_index) {
            self.editing = Some(Editing {
                target: EditTarget::JournalBody {
                    journal_id: journal.id.clone(),
                    base_version: journal.version,
                },
                buffer: journal.body.clone(),
                cursor: journal.body.len(),
                multiline: true,
            });
        }
    }

    fn delete_selected_journal(&mut self) {
        if !self.ensure_writable() {
            return;
        }
        let journals = self.all_journals();
        if let Some(journal) = journals.get(self.selected_journal_index) {
            self.request_delete(PendingDelete::Summary {
                id: journal.id.clone(),
                version: journal.version,
            });
        }
    }

    fn open_week_view(&mut self) {
        self.week_mode = true;
        // 当前查看日离今天不超过 6 天时，窗口对齐到今天（包含今天和未来）；
        // 更早的日期则让窗口以它结尾，避免被挤到窗口外。
        self.week_end = if self.view_date >= self.today - ChronoDuration::days(6) {
            self.today.max(self.view_date)
        } else {
            self.view_date
        };
        let start = self.week_end - ChronoDuration::days(6);
        self.week_selected = (self.view_date - start).num_days().clamp(0, 6) as usize;
    }

    fn handle_today_key(&mut self, key: KeyEvent) {
        if self.week_mode {
            match key.code {
                KeyCode::Char('w') | KeyCode::Char('W') | KeyCode::Esc => self.week_mode = false,
                KeyCode::Up | KeyCode::Char('k') => {
                    self.week_selected = self.week_selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.week_selected = (self.week_selected + 1).min(6);
                }
                // 整周前后平移，可以回到今天和未来。
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('[') => {
                    self.week_end -= ChronoDuration::days(1);
                }
                KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(']') => {
                    self.week_end += ChronoDuration::days(1);
                }
                KeyCode::Enter => {
                    let overview = self.week_overview();
                    if let Some((date, _, _)) = overview.get(self.week_selected) {
                        self.view_date = *date;
                        self.week_mode = false;
                        self.clamp();
                    }
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('[') | KeyCode::Left | KeyCode::Char('h') => {
                self.view_date -= ChronoDuration::days(1);
                self.clamp();
            }
            KeyCode::Char(']') | KeyCode::Right | KeyCode::Char('l') => {
                self.view_date += ChronoDuration::days(1);
                self.clamp();
            }
            KeyCode::Char('w') | KeyCode::Char('W') => self.open_week_view(),
            KeyCode::Tab => match self.today_focus {
                TodayFocus::Items => {
                    self.journal_source = TodayFocus::Items;
                    self.today_focus = TodayFocus::Habits;
                }
                TodayFocus::Habits => {
                    self.journal_source = TodayFocus::Habits;
                    self.today_focus = TodayFocus::Journal;
                }
                TodayFocus::Journal => {
                    self.today_focus = TodayFocus::Items;
                }
            },
            KeyCode::Up | KeyCode::Char('k') => self.move_today(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_today(1),
            KeyCode::Char(' ') => self.toggle_today_item(),
            KeyCode::Char('u') => self.undo_today_habit(),
            KeyCode::Enter | KeyCode::Char('e') => self.edit_today_item(),
            KeyCode::Char('a') => self.edit_today_summary(),
            KeyCode::Char('d') => self.delete_today_item(),
            KeyCode::Char('t') => self.open_event_form(BlockKind::Todo, self.view_key(), None),
            _ => {}
        }
    }

    fn handle_calendar_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.range_anchor = None;
                self.range_focus = None;
            }
            KeyCode::Char('v') => self.toggle_range_anchor(),
            KeyCode::Left | KeyCode::Char('h') => self.shift_day(-1),
            KeyCode::Right | KeyCode::Char('l') => self.shift_day(1),
            KeyCode::Up | KeyCode::Char('k') => self.shift_day(-7),
            KeyCode::Down | KeyCode::Char('j') => self.shift_day(7),
            KeyCode::Char('[') => self.shift_month(-1),
            KeyCode::Char(']') => self.shift_month(1),
            KeyCode::Enter | KeyCode::Char(' ') => self.open_agenda(),
            KeyCode::Char('t') => {
                let (start, end) = self.range_or_selected();
                self.open_event_form(BlockKind::Todo, start, end);
            }
            _ => {}
        }
    }

    fn handle_habits_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_habit(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_habit(1),
            KeyCode::Char(' ') => self.check_in_selected_habit(),
            KeyCode::Char('u') => self.undo_selected_habit(),
            KeyCode::Enter | KeyCode::Char('e') => self.edit_selected_habit(),
            KeyCode::Char('d') => self.delete_selected_habit(),
            KeyCode::Char('n') => self.open_habit_form(self.today_key(), None),
            _ => {}
        }
    }

    fn handle_stats_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.stats_days = match self.stats_days {
                    7 => 90,
                    14 => 7,
                    30 => 14,
                    _ => 30,
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.stats_days = match self.stats_days {
                    7 => 14,
                    14 => 30,
                    30 => 90,
                    _ => 7,
                }
            }
            _ => {}
        }
    }

    fn handle_inbox_key(&mut self, key: KeyEvent) {
        if self.read_only()
            && matches!(
                key.code,
                KeyCode::Char('r') | KeyCode::Char('e') | KeyCode::Char('d')
            )
        {
            self.status = "正在查看他人视角（只读），只能回复评论".to_string();
            return;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_inbox(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_inbox(1),
            KeyCode::Enter => self.jump_to_selected_comment(),
            KeyCode::Char('a') => self.reply_selected_comment(),
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.toggle_selected_comment_resolved()
            }
            KeyCode::Char('r') => self.toggle_selected_comment_resolved(),
            KeyCode::Char('e') => self.edit_selected_comment(),
            KeyCode::Char('d') => self.delete_selected_comment(),
            _ => {}
        }
    }

    fn move_selection(&mut self, delta: i64) {
        match self.focus {
            Focus::Pages => {
                let count = self.visible_pages().len();
                if count > 0 {
                    let next = self.selected_page_index as i64 + delta;
                    self.selected_page_index = next.clamp(0, count as i64 - 1) as usize;
                    self.selected_block_index = 0;
                }
            }
            Focus::Blocks => {
                let count = self.visible_blocks().len();
                if count > 0 {
                    let next = self.selected_block_index as i64 + delta;
                    self.selected_block_index = next.clamp(0, count as i64 - 1) as usize;
                }
            }
        }
    }

    fn move_today(&mut self, delta: i64) {
        match self.today_focus {
            TodayFocus::Items => {
                let count = self.today_items().len();
                if count > 0 {
                    let next = self.selected_today_index as i64 + delta;
                    self.selected_today_index = next.clamp(0, count as i64 - 1) as usize;
                }
            }
            TodayFocus::Habits => {
                let count = self.today_habits().len();
                if count > 0 {
                    let next = self.selected_today_habit_index as i64 + delta;
                    self.selected_today_habit_index = next.clamp(0, count as i64 - 1) as usize;
                }
            }
            TodayFocus::Journal => {}
        }
    }

    fn move_habit(&mut self, delta: i64) {
        let count = self.visible_habits().len();
        if count > 0 {
            let next = self.selected_habit_index as i64 + delta;
            self.selected_habit_index = next.clamp(0, count as i64 - 1) as usize;
        }
    }

    fn shift_day(&mut self, delta: i64) {
        self.selected_day += ChronoDuration::days(delta);
        self.calendar_month = self.selected_day.with_day(1).unwrap_or(self.selected_day);
        self.selected_agenda_index = 0;
        if self.range_anchor.is_some() {
            self.range_focus = Some(self.selected_day);
        }
    }

    fn shift_month(&mut self, delta: i32) {
        let result = if delta > 0 {
            self.calendar_month
                .checked_add_months(Months::new(delta as u32))
        } else {
            self.calendar_month
                .checked_sub_months(Months::new((-delta) as u32))
        };
        if let Some(date) = result {
            self.calendar_month = date.with_day(1).unwrap_or(date);
            self.selected_day = self.calendar_month;
            self.selected_agenda_index = 0;
        }
    }

    fn start_edit(&mut self) {
        if !self.ensure_writable() {
            return;
        }
        match self.focus {
            Focus::Pages => {
                if let Some(page) = self.current_page() {
                    self.editing = Some(Editing {
                        target: EditTarget::PageTitle {
                            page_id: page.id,
                            base_version: page.version,
                        },
                        cursor: page.title.len(),
                        buffer: page.title,
                        multiline: false,
                    });
                }
            }
            Focus::Blocks => {
                if let Some(block) = self.current_block() {
                    self.editing = Some(Editing {
                        target: EditTarget::BlockText {
                            block_id: block.id,
                            base_version: block.version,
                        },
                        cursor: block.text.len(),
                        buffer: block.text,
                        multiline: block.section.is_some(),
                    });
                }
            }
        }
    }

    fn edit_today_item(&mut self) {
        if !self.ensure_writable() {
            return;
        }
        match self.today_focus {
            TodayFocus::Items => {
                let rows = self.today_items();
                if let Some(TodayRow::Todo(block) | TodayRow::Event(block)) =
                    rows.get(self.selected_today_index)
                {
                    self.editing = Some(Editing {
                        target: EditTarget::BlockText {
                            block_id: block.id.clone(),
                            base_version: block.version,
                        },
                        buffer: block.text.clone(),
                        cursor: block.text.len(),
                        multiline: false,
                    });
                }
            }
            TodayFocus::Habits => {
                self.status = "习惯的名称/频率只能在习惯界面修改".to_string();
            }
            TodayFocus::Journal => self.edit_today_summary(),
        }
    }

    fn edit_selected_habit(&mut self) {
        if !self.ensure_writable() {
            return;
        }
        let habits = self.visible_habits();
        if let Some(habit) = habits.get(self.selected_habit_index) {
            self.editing = Some(Editing {
                target: EditTarget::HabitTitle {
                    habit_id: habit.id.clone(),
                    base_version: habit.version,
                },
                buffer: habit.title.clone(),
                cursor: habit.title.len(),
                multiline: false,
            });
        }
    }

    fn handle_edit_key(&mut self, key: KeyEvent) {
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => self.editing = None,
            KeyCode::Enter if editing.multiline => {
                editing.buffer.insert(editing.cursor, '\n');
                editing.cursor += 1;
            }
            KeyCode::Enter => {
                let editing = self.editing.take().expect("editing");
                self.commit_edit(editing);
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let editing = self.editing.take().expect("editing");
                self.commit_edit(editing);
            }
            KeyCode::Backspace => {
                if editing.cursor > 0 {
                    let previous = editing.buffer[..editing.cursor]
                        .char_indices()
                        .last()
                        .map(|(index, _)| index)
                        .unwrap_or(0);
                    editing.buffer.remove(previous);
                    editing.cursor = previous;
                }
            }
            KeyCode::Delete => {
                if editing.cursor < editing.buffer.len() {
                    editing.buffer.remove(editing.cursor);
                }
            }
            KeyCode::Left => {
                if editing.cursor > 0 {
                    editing.cursor = editing.buffer[..editing.cursor]
                        .char_indices()
                        .last()
                        .map(|(index, _)| index)
                        .unwrap_or(0);
                }
            }
            KeyCode::Right => {
                if editing.cursor < editing.buffer.len() {
                    if let Some((index, character)) =
                        editing.buffer[editing.cursor..].char_indices().next()
                    {
                        editing.cursor = editing.cursor + index + character.len_utf8();
                    }
                }
            }
            KeyCode::Home => editing.cursor = 0,
            KeyCode::End => editing.cursor = editing.buffer.len(),
            KeyCode::Char(c) => {
                editing.buffer.insert(editing.cursor, c);
                editing.cursor += c.len_utf8();
            }
            _ => {}
        }
    }

    fn commit_edit(&mut self, editing: Editing) {
        match editing.target {
            EditTarget::PageTitle {
                page_id,
                base_version,
            } => {
                let title = editing.buffer.trim().to_string();
                let title = if title.is_empty() {
                    "未命名页面".to_string()
                } else {
                    title
                };
                self.patch_page(&page_id, base_version, Some(title), None);
            }
            EditTarget::BlockText {
                block_id,
                base_version,
            } => {
                self.patch_block(
                    &block_id,
                    base_version,
                    BlockPatch {
                        text: Some(editing.buffer),
                        ..Default::default()
                    },
                );
            }
            EditTarget::HabitTitle {
                habit_id,
                base_version,
            } => {
                let title = editing.buffer.trim().to_string();
                if title.is_empty() {
                    self.status = "习惯名称不能为空".to_string();
                    return;
                }
                self.update_habit(
                    &habit_id,
                    UpdateHabitRequest {
                        expected_version: base_version,
                        title: Some(title),
                        icon: None,
                        note: None,
                        frequency: None,
                        position: None,
                        archived: None,
                        deleted: None,
                    },
                );
            }
            EditTarget::NewComment {
                target_kind,
                target_id,
            } => {
                let body = editing.buffer.trim().to_string();
                if body.is_empty() {
                    self.status = "评论内容不能为空".to_string();
                    return;
                }
                match self.api.create_comment(&CreateCommentRequest {
                    target_kind,
                    target_id: target_id.clone(),
                    body,
                }) {
                    Ok(response) => {
                        self.upsert_comment(response.comment);
                        self.status = "评论已发表".to_string();
                    }
                    Err(err) => self.status = err.to_string(),
                }
            }
            EditTarget::CommentBody {
                comment_id,
                base_version,
            } => {
                let body = editing.buffer.trim().to_string();
                if body.is_empty() {
                    self.status = "评论内容不能为空".to_string();
                    return;
                }
                self.update_comment(
                    &comment_id,
                    UpdateCommentRequest {
                        expected_version: base_version,
                        body: Some(body),
                        resolved: None,
                        deleted: None,
                    },
                );
            }
            EditTarget::JournalBody {
                journal_id,
                base_version,
            } => {
                let body = editing.buffer.trim().to_string();
                if body.is_empty() {
                    self.status = "总结内容不能为空".to_string();
                    return;
                }
                self.update_journal(
                    &journal_id,
                    UpdateJournalRequest {
                        expected_version: base_version,
                        body: Some(body),
                        deleted: None,
                    },
                );
            }
            EditTarget::NewJournal {
                target_kind,
                target_id,
                date,
                time,
            } => {
                let body = editing.buffer.trim().to_string();
                if body.is_empty() {
                    self.status = "总结内容不能为空".to_string();
                    return;
                }
                match self.api.create_journal(&CreateJournalRequest {
                    target_kind,
                    target_id,
                    date,
                    time,
                    body,
                }) {
                    Ok(response) => {
                        self.upsert_journal(response.journal);
                        self.status = "总结已保存".to_string();
                        self.clamp();
                    }
                    Err(err) => self.status = err.to_string(),
                }
            }
        }
    }

    fn update_habit(&mut self, id: &str, request: UpdateHabitRequest) {
        match self.api.update_habit(id, &request) {
            Ok(response) => {
                self.upsert_habit(response.habit);
                self.clamp();
            }
            Err(ApiError::Conflict { message, current }) => {
                if let Ok(habit) = serde_json::from_value::<Habit>(current) {
                    self.upsert_habit(habit);
                    self.clamp();
                }
                self.status = format!("习惯冲突，已刷新：{message}");
            }
            Err(ApiError::Other(message)) => self.status = message,
        }
    }

    fn update_comment(&mut self, id: &str, request: UpdateCommentRequest) {
        match self.api.update_comment(id, &request) {
            Ok(response) => {
                self.upsert_comment(response.comment);
            }
            Err(ApiError::Conflict { message, current }) => {
                if let Ok(comment) = serde_json::from_value::<Comment>(current) {
                    self.upsert_comment(comment);
                }
                self.status = format!("评论冲突，已刷新：{message}");
            }
            Err(ApiError::Other(message)) => self.status = message,
        }
    }

    fn new_workspace_block(&mut self, kind: BlockKind) {
        if !self.ensure_writable() {
            return;
        }
        let Some(page_id) = self.current_page_id() else {
            self.status = "请先创建或选择一个页面".to_string();
            return;
        };
        let position = self.visible_blocks().len() as i64;
        let request = CreateBlockRequest {
            page_id: page_id.clone(),
            kind,
            text: String::new(),
            due_date: None,
            position,
            start_time: None,
            end_time: None,
            section: None,
        };
        match self.api.create_block(&request) {
            Ok(response) => {
                let block = response.block;
                let block_id = block.id.clone();
                self.upsert_block(block);
                self.focus = Focus::Blocks;
                self.clamp();
                if let Some(index) = self
                    .visible_blocks()
                    .iter()
                    .position(|item| item.id == block_id)
                {
                    self.selected_block_index = index;
                }
                self.editing = Some(Editing {
                    target: EditTarget::BlockText {
                        block_id,
                        base_version: 1,
                    },
                    buffer: String::new(),
                    cursor: 0,
                    multiline: false,
                });
                self.status = "输入内容，Enter 保存，Esc 取消".to_string();
            }
            Err(err) => self.status = err.to_string(),
        }
    }

    fn new_page(&mut self) {
        if !self.ensure_writable() {
            return;
        }
        let position = self.visible_pages().len() as i64;
        let request = CreatePageRequest {
            title: "新页面".to_string(),
            icon: PageKind::Doc.icon().to_string(),
            kind: PageKind::Doc,
            position,
            date: None,
        };
        match self.api.create_page(&request) {
            Ok(response) => {
                let page = response.page;
                let page_id = page.id.clone();
                self.upsert_page(page);
                self.focus = Focus::Pages;
                self.clamp();
                if let Some(index) = self
                    .visible_pages()
                    .iter()
                    .position(|item| item.id == page_id)
                {
                    self.selected_page_index = index;
                }
                self.editing = Some(Editing {
                    target: EditTarget::PageTitle {
                        page_id,
                        base_version: 1,
                    },
                    buffer: "新页面".to_string(),
                    cursor: "新页面".len(),
                    multiline: false,
                });
                self.status = "输入页面标题，Enter 保存".to_string();
            }
            Err(err) => self.status = err.to_string(),
        }
    }

    fn toggle_workspace_block(&mut self) {
        if !self.ensure_writable() {
            return;
        }
        let Some(block) = self.current_block() else {
            return;
        };
        match block.kind {
            BlockKind::Todo => {
                self.patch_block(
                    &block.id,
                    block.version,
                    BlockPatch {
                        checked: Some(!block.checked),
                        ..Default::default()
                    },
                );
            }
            BlockKind::Habit => {
                match habit_check_in(block.due_date.as_deref(), &self.today_key(), block.streak) {
                    Some(streak) => {
                        self.patch_block(
                            &block.id,
                            block.version,
                            BlockPatch {
                                checked: Some(true),
                                due_date: Some(self.today_key()),
                                streak: Some(streak),
                                ..Default::default()
                            },
                        );
                    }
                    None => self.status = "今天已经打过卡了".to_string(),
                }
            }
            _ => self.status = "这个内容块不支持勾选".to_string(),
        }
    }

    fn toggle_today_item(&mut self) {
        if !self.ensure_writable() {
            return;
        }
        match self.today_focus {
            TodayFocus::Items => {
                let rows = self.today_items();
                if let Some(TodayRow::Todo(block) | TodayRow::Event(block)) =
                    rows.get(self.selected_today_index)
                {
                    self.patch_block(
                        &block.id,
                        block.version,
                        BlockPatch {
                            checked: Some(!block.checked),
                            ..Default::default()
                        },
                    );
                }
            }
            TodayFocus::Habits => {
                let habits = self.today_habits();
                if let Some((habit, stats)) = habits.get(self.selected_today_habit_index) {
                    if stats.checked_today {
                        self.status = "今天已经打过卡了".to_string();
                    } else {
                        self.check_in(habit);
                    }
                }
            }
            TodayFocus::Journal => {
                self.status = "总结不需要勾选，按 Enter 编辑".to_string();
            }
        }
    }

    fn check_in(&mut self, habit: &Habit) {
        if self.view_date != self.today {
            self.status = "只能给今天打卡，历史日期只读".to_string();
            return;
        }
        match self.api.check_in(
            &habit.id,
            &CheckinRequest {
                date: self.today_key(),
                note: None,
            },
        ) {
            Ok(response) => {
                if response.created {
                    self.upsert_checkin(response.checkin);
                    self.status = format!("「{}」打卡成功", habit.title);
                } else {
                    self.upsert_checkin(response.checkin);
                    self.status = "今天已经打过卡了".to_string();
                }
            }
            Err(err) => self.status = err.to_string(),
        }
    }

    fn check_in_selected_habit(&mut self) {
        if !self.ensure_writable() {
            return;
        }
        let habits = self.visible_habits();
        if let Some(habit) = habits.get(self.selected_habit_index) {
            self.check_in(habit);
        }
    }

    fn today_checkin(&self, habit: &Habit) -> Option<HabitCheckin> {
        let key = self.today_key();
        checkin_on(&self.checkins, &habit.id, &key).cloned()
    }

    fn undo_checkin(&mut self, habit: &Habit) {
        if self.view_date != self.today {
            self.status = "只能撤销今天的打卡".to_string();
            return;
        }
        if let Some(checkin) = self.today_checkin(habit) {
            match self.api.update_checkin(
                &checkin.id,
                &UpdateCheckinRequest {
                    expected_version: checkin.version,
                    note: None,
                    deleted: Some(true),
                },
            ) {
                Ok(response) => {
                    self.upsert_checkin(response.checkin);
                    self.status = "已撤销今天的打卡".to_string();
                }
                Err(err) => self.status = err.to_string(),
            }
        } else {
            self.status = "今天还没有打卡".to_string();
        }
    }

    fn undo_selected_habit(&mut self) {
        if !self.ensure_writable() {
            return;
        }
        let habits = self.visible_habits();
        if let Some(habit) = habits.get(self.selected_habit_index) {
            self.undo_checkin(habit);
        }
    }

    fn undo_today_habit(&mut self) {
        if !self.ensure_writable() {
            return;
        }
        if self.today_focus != TodayFocus::Habits {
            self.status = "按 Tab 切到习惯面板后再撤销".to_string();
            return;
        }
        let habits = self.today_habits();
        if let Some((habit, _)) = habits.get(self.selected_today_habit_index) {
            self.undo_checkin(habit);
        }
    }

    fn request_delete(&mut self, action: PendingDelete) {
        let label = action.label();
        self.modal = Some(Modal::Confirm(ConfirmState {
            action,
            input: String::new(),
            label,
        }));
    }

    fn confirm_delete(&mut self, action: PendingDelete) {
        match action {
            PendingDelete::Block { id, version, .. } => self.patch_block(
                &id,
                version,
                BlockPatch {
                    deleted: Some(true),
                    ..Default::default()
                },
            ),
            PendingDelete::Page { id, version, .. } => {
                self.patch_page(&id, version, None, Some(true))
            }
            PendingDelete::Habit { id, version, .. } => self.update_habit(
                &id,
                UpdateHabitRequest {
                    expected_version: version,
                    title: None,
                    icon: None,
                    note: None,
                    frequency: None,
                    position: None,
                    archived: Some(true),
                    deleted: None,
                },
            ),
            PendingDelete::Comment { id, version } => self.update_comment(
                &id,
                UpdateCommentRequest {
                    expected_version: version,
                    body: None,
                    resolved: None,
                    deleted: Some(true),
                },
            ),
            PendingDelete::Summary { id, version } => self.update_journal(
                &id,
                UpdateJournalRequest {
                    expected_version: version,
                    body: None,
                    deleted: Some(true),
                },
            ),
        }
        self.status = "已删除".to_string();
    }

    fn delete_workspace_current(&mut self) {
        if !self.ensure_writable() {
            return;
        }
        match self.focus {
            Focus::Pages => {
                if let Some(page) = self.current_page() {
                    self.request_delete(PendingDelete::Page {
                        id: page.id,
                        version: page.version,
                        label: format!("页面「{}」", page.title),
                    });
                }
            }
            Focus::Blocks => {
                if let Some(block) = self.current_block() {
                    self.request_delete(PendingDelete::Block {
                        id: block.id,
                        version: block.version,
                        label: format!(
                            "待办「{}」",
                            if block.text.is_empty() {
                                block.kind.icon()
                            } else {
                                block.text.as_str()
                            }
                        ),
                    });
                }
            }
        }
    }

    fn delete_today_item(&mut self) {
        if !self.ensure_writable() {
            return;
        }
        match self.today_focus {
            TodayFocus::Items => {
                let rows = self.today_items();
                if let Some(TodayRow::Todo(block) | TodayRow::Event(block)) =
                    rows.get(self.selected_today_index)
                {
                    self.request_delete(PendingDelete::Block {
                        id: block.id.clone(),
                        version: block.version,
                        label: format!(
                            "待办「{}」",
                            if block.text.is_empty() {
                                block.kind.icon()
                            } else {
                                block.text.as_str()
                            }
                        ),
                    });
                }
            }
            TodayFocus::Habits => {
                let habits = self.today_habits();
                if let Some((habit, _)) = habits.get(self.selected_today_habit_index) {
                    self.request_delete(PendingDelete::Habit {
                        id: habit.id.clone(),
                        version: habit.version,
                        label: format!("习惯「{}」", habit.title),
                    });
                }
            }
            TodayFocus::Journal => {
                let entries = self.today_summary_entries();
                if let Some(summary) = entries.first() {
                    self.request_delete(PendingDelete::Summary {
                        id: summary.id.clone(),
                        version: summary.version,
                    });
                } else {
                    self.status = "当前对象没有总结".to_string();
                }
            }
        }
    }

    fn delete_selected_habit(&mut self) {
        if !self.ensure_writable() {
            return;
        }
        let habits = self.visible_habits();
        if let Some(habit) = habits.get(self.selected_habit_index) {
            self.request_delete(PendingDelete::Habit {
                id: habit.id.clone(),
                version: habit.version,
                label: format!("习惯「{}」", habit.title),
            });
        }
    }

    fn open_event_form(&mut self, kind: BlockKind, date: String, range_end: Option<String>) {
        if !self.ensure_writable() {
            return;
        }
        self.modal = Some(Modal::EventForm(EventForm {
            kind,
            title: String::new(),
            date,
            start_time: String::new(),
            end_time: String::new(),
            field: 0,
            error: None,
            range_end,
        }));
    }

    fn open_habit_form(&mut self, date: String, range_end: Option<String>) {
        if !self.ensure_writable() {
            return;
        }
        self.modal = Some(Modal::HabitForm(HabitForm {
            title: String::new(),
            note: String::new(),
            frequency: HabitFrequency::Daily,
            field: 0,
            error: None,
            date,
            range_end,
        }));
    }

    fn toggle_range_anchor(&mut self) {
        if self.range_anchor.is_some() {
            self.range_anchor = None;
            self.range_focus = None;
            self.status = "已取消日期范围".to_string();
        } else {
            self.range_anchor = Some(self.selected_day);
            self.range_focus = Some(self.selected_day);
            self.status = "已设置范围起点，移动光标选择终点，再按 v 取消".to_string();
        }
    }

    pub fn selected_range(&self) -> Option<(NaiveDate, NaiveDate)> {
        let anchor = self.range_anchor?;
        let focus = self.range_focus.unwrap_or(anchor);
        if anchor <= focus {
            Some((anchor, focus))
        } else {
            Some((focus, anchor))
        }
    }

    fn range_or_selected(&self) -> (String, Option<String>) {
        match self.selected_range() {
            Some((start, end)) if start != end => (
                start.format("%Y-%m-%d").to_string(),
                Some(end.format("%Y-%m-%d").to_string()),
            ),
            _ => (self.selected_day.format("%Y-%m-%d").to_string(), None),
        }
    }

    fn open_agenda(&mut self) {
        self.modal = Some(Modal::Agenda(AgendaPanel {
            date: self.selected_day,
            selected: 0,
        }));
    }

    fn edit_today_summary(&mut self) {
        if !self.ensure_writable() {
            return;
        }
        let Some((kind, target_id)) = self.current_summary_target() else {
            self.status = "先选中一条待办或习惯".to_string();
            return;
        };
        let date = self.view_key();
        let existing = self
            .journals_for_target(kind, &target_id)
            .into_iter()
            .find(|journal| journal.date == date);
        if let Some(summary) = existing {
            self.editing = Some(Editing {
                target: EditTarget::JournalBody {
                    journal_id: summary.id,
                    base_version: summary.version,
                },
                buffer: summary.body.clone(),
                cursor: summary.body.len(),
                multiline: true,
            });
        } else {
            self.editing = Some(Editing {
                target: EditTarget::NewJournal {
                    target_kind: kind,
                    target_id,
                    date: date.clone(),
                    time: Local::now().format("%H:%M").to_string(),
                },
                buffer: String::new(),
                cursor: 0,
                multiline: true,
            });
        }
        self.status = "写总结，Ctrl+S 保存，Esc 取消".to_string();
    }

    fn update_journal(&mut self, id: &str, request: UpdateJournalRequest) {
        match self.api.update_journal(id, &request) {
            Ok(response) => {
                self.upsert_journal(response.journal);
                self.clamp();
            }
            Err(ApiError::Conflict { message, current }) => {
                if let Ok(journal) = serde_json::from_value::<JournalEntry>(current) {
                    self.upsert_journal(journal);
                }
                self.status = format!("总结冲突，已刷新：{message}");
            }
            Err(ApiError::Other(message)) => self.status = message,
        }
    }

    fn move_inbox(&mut self, delta: i64) {
        let count = self.inbox_comments().len();
        if count > 0 {
            let next = self.selected_inbox_index as i64 + delta;
            self.selected_inbox_index = next.clamp(0, count as i64 - 1) as usize;
        }
    }

    fn jump_to_selected_comment(&mut self) {
        let comments = self.inbox_comments();
        if let Some(comment) = comments.get(self.selected_inbox_index).cloned() {
            self.jump_to_comment_target(&comment);
        }
    }

    fn jump_to_comment_target(&mut self, comment: &Comment) {
        match comment.target_kind {
            CommentTargetKind::Page => {
                self.show_all_pages = true;
                self.view = View::Workspace;
                self.focus = Focus::Pages;
                if let Some(index) = self
                    .visible_pages()
                    .iter()
                    .position(|page| page.id == comment.target_id)
                {
                    self.selected_page_index = index;
                }
            }
            CommentTargetKind::Block => {
                self.show_all_pages = true;
                self.view = View::Workspace;
                self.focus = Focus::Blocks;
                let page_id = self
                    .blocks
                    .iter()
                    .find(|block| block.id == comment.target_id)
                    .map(|block| block.page_id.clone());
                if let Some(page_id) = page_id {
                    if let Some(index) = self
                        .visible_pages()
                        .iter()
                        .position(|page| page.id == page_id)
                    {
                        self.selected_page_index = index;
                        self.selected_block_index = 0;
                        if let Some(block_index) = self
                            .visible_blocks()
                            .iter()
                            .position(|block| block.id == comment.target_id)
                        {
                            self.selected_block_index = block_index;
                        }
                    }
                }
            }
            CommentTargetKind::Habit => {
                self.view = View::Habits;
                if let Some(index) = self
                    .visible_habits()
                    .iter()
                    .position(|habit| habit.id == comment.target_id)
                {
                    self.selected_habit_index = index;
                }
            }
        }
        self.clamp();
    }

    fn reply_selected_comment(&mut self) {
        let comments = self.inbox_comments();
        if let Some(comment) = comments.get(self.selected_inbox_index) {
            self.editing = Some(Editing {
                target: EditTarget::NewComment {
                    target_kind: comment.target_kind,
                    target_id: comment.target_id.clone(),
                },
                buffer: String::new(),
                cursor: 0,
                multiline: true,
            });
        }
    }

    fn toggle_selected_comment_resolved(&mut self) {
        let comments = self.inbox_comments();
        if let Some(comment) = comments.get(self.selected_inbox_index).cloned() {
            self.update_comment(
                &comment.id,
                UpdateCommentRequest {
                    expected_version: comment.version,
                    body: None,
                    resolved: Some(!comment.resolved),
                    deleted: None,
                },
            );
        }
    }

    fn edit_selected_comment(&mut self) {
        let comments = self.inbox_comments();
        if let Some(comment) = comments.get(self.selected_inbox_index).cloned() {
            self.editing = Some(Editing {
                target: EditTarget::CommentBody {
                    comment_id: comment.id.clone(),
                    base_version: comment.version,
                },
                buffer: comment.body.clone(),
                cursor: comment.body.len(),
                multiline: true,
            });
        }
    }

    fn delete_selected_comment(&mut self) {
        let comments = self.inbox_comments();
        if let Some(comment) = comments.get(self.selected_inbox_index).cloned() {
            self.request_delete(PendingDelete::Comment {
                id: comment.id,
                version: comment.version,
            });
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        if self.editing.is_some() || self.modal.is_some() {
            return;
        }

        // 导航栏、列表行等登记的点击目标优先。
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            if let Some(action) = self.mouse_hit_at(mouse.column, mouse.row) {
                self.apply_mouse_action(action);
                return;
            }
        }

        // 日历格子：左键选日，Shift + 右键框选。
        if self.view == View::Calendar {
            let Some(date) = self.calendar_date_at(mouse.column, mouse.row) else {
                return;
            };
            let shift = mouse.modifiers.contains(KeyModifiers::SHIFT);
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let double_click = matches!(
                        self.last_click,
                        Some((at, column, row))
                            if at.elapsed() < Duration::from_millis(400)
                                && column == mouse.column
                                && row == mouse.row
                    );
                    self.last_click = Some((Instant::now(), mouse.column, mouse.row));
                    self.selected_day = date;
                    self.calendar_month = date.with_day(1).unwrap_or(date);
                    self.selected_agenda_index = 0;
                    if double_click {
                        self.open_agenda();
                    }
                }
                MouseEventKind::Down(MouseButton::Right) if shift => {
                    self.range_anchor = Some(date);
                    self.range_focus = Some(date);
                    self.selected_day = date;
                    self.status = "已开始框选日期，拖动选择范围".to_string();
                }
                MouseEventKind::Drag(MouseButton::Right) if self.range_anchor.is_some() => {
                    self.range_focus = Some(date);
                }
                MouseEventKind::Up(MouseButton::Right) if self.range_anchor.is_some() => {
                    self.range_focus = Some(date);
                    self.selected_day = date;
                }
                _ => {}
            }
        }
    }

    fn apply_mouse_action(&mut self, action: MouseAction) {
        match action {
            MouseAction::SwitchView(view) => {
                self.view = view;
                self.week_mode = false;
                self.workspace_journal_mode = false;
                self.clamp();
            }
            MouseAction::FocusToday(focus) => {
                if focus == TodayFocus::Journal && self.today_focus != TodayFocus::Journal {
                    self.journal_source = self.today_focus;
                }
                self.today_focus = focus;
            }
            MouseAction::SelectTodayItem(index) => {
                self.today_focus = TodayFocus::Items;
                self.selected_today_index = index;
                self.clamp();
            }
            MouseAction::SelectTodayHabit(index) => {
                self.today_focus = TodayFocus::Habits;
                self.selected_today_habit_index = index;
                self.clamp();
            }
            MouseAction::SelectPage(index) => {
                self.focus = Focus::Pages;
                self.selected_page_index = index;
                self.selected_block_index = 0;
                self.workspace_journal_mode = false;
                self.clamp();
            }
            MouseAction::SelectBlock(index) => {
                self.focus = Focus::Blocks;
                self.selected_block_index = index;
                self.clamp();
            }
            MouseAction::SelectHabit(index) => {
                self.selected_habit_index = index;
                self.clamp();
            }
            MouseAction::SelectInbox(index) => {
                self.selected_inbox_index = index;
                self.clamp();
            }
            MouseAction::OpenJournal(index) => {
                self.workspace_journal_mode = true;
                self.selected_journal_index = index;
                self.clamp();
            }
            MouseAction::WeekSelect(index) => {
                self.week_selected = index.min(6);
            }
            MouseAction::PrevDay => {
                self.view_date -= ChronoDuration::days(1);
                self.clamp();
            }
            MouseAction::NextDay => {
                self.view_date += ChronoDuration::days(1);
                self.clamp();
            }
            MouseAction::PrevMonth => self.shift_month(-1),
            MouseAction::NextMonth => self.shift_month(1),
            MouseAction::OpenWeek => self.open_week_view(),
            MouseAction::OpenMembers => self.open_members(),
            MouseAction::OpenInvites => self.open_invites(),
            MouseAction::OpenSearch => self.open_search(),
            MouseAction::OpenHelp => self.modal = Some(Modal::Help),
            MouseAction::SyncNow => {
                self.ensured_date = None;
                self.ensure_today_page();
                self.sync_now(true);
            }
        }
    }

    fn calendar_date_at(&self, column: u16, row: u16) -> Option<NaiveDate> {
        self.calendar_cells
            .borrow()
            .iter()
            .find(|(rect, _)| {
                column >= rect.x
                    && column < rect.x.saturating_add(rect.width)
                    && row >= rect.y
                    && row < rect.y.saturating_add(rect.height)
            })
            .map(|(_, date)| *date)
    }

    fn open_comments(&mut self) {
        let Some((kind, id, label)) = self.current_comment_target() else {
            self.status = "当前没有可评论的对象".to_string();
            return;
        };
        self.modal = Some(Modal::Comments(CommentPanel {
            target_kind: kind,
            target_id: id,
            label,
            selected: 0,
        }));
    }

    fn current_comment_target(&self) -> Option<(CommentTargetKind, String, String)> {
        match self.view {
            View::Workspace => match self.focus {
                Focus::Pages => self.current_page().map(|page| {
                    (
                        CommentTargetKind::Page,
                        page.id.clone(),
                        format!("页面「{}」", page.title),
                    )
                }),
                Focus::Blocks => self.current_block().map(|block| {
                    (
                        CommentTargetKind::Block,
                        block.id.clone(),
                        format!(
                            "内容块「{}」",
                            if block.text.is_empty() {
                                block.kind.icon()
                            } else {
                                block.text.as_str()
                            }
                        ),
                    )
                }),
            },
            View::Today => match self.today_focus {
                TodayFocus::Items => {
                    self.today_items()
                        .get(self.selected_today_index)
                        .and_then(|row| match row {
                            TodayRow::Todo(block) | TodayRow::Event(block) => Some((
                                CommentTargetKind::Block,
                                block.id.clone(),
                                format!(
                                    "待办「{}」",
                                    if block.text.is_empty() {
                                        block.kind.icon()
                                    } else {
                                        block.text.as_str()
                                    }
                                ),
                            )),
                            TodayRow::Habit(_, _) => None,
                        })
                }
                TodayFocus::Habits => self
                    .today_habits()
                    .get(self.selected_today_habit_index)
                    .map(|(habit, _)| {
                        (
                            CommentTargetKind::Habit,
                            habit.id.clone(),
                            format!("习惯「{}」", habit.title),
                        )
                    }),
                TodayFocus::Journal => None,
            },
            View::Calendar => self
                .agenda_blocks()
                .get(self.selected_agenda_index)
                .map(|block| {
                    (
                        CommentTargetKind::Block,
                        block.id.clone(),
                        format!("日程「{}」", block.text),
                    )
                }),
            View::Habits => self
                .visible_habits()
                .get(self.selected_habit_index)
                .map(|habit| {
                    (
                        CommentTargetKind::Habit,
                        habit.id.clone(),
                        format!("习惯「{}」", habit.title),
                    )
                }),
            View::Stats => None,
            View::Inbox => None,
        }
    }

    fn open_search(&mut self) {
        self.modal = Some(Modal::Search(SearchState {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
        }));
    }

    fn open_invites(&mut self) {
        match self.api.list_invites() {
            Ok(response) => {
                self.modal = Some(Modal::Invites(InvitePanel {
                    invites: response.invites,
                    selected: 0,
                    message: None,
                }));
            }
            Err(err) => self.status = err.to_string(),
        }
    }

    fn open_members(&mut self) {
        if !self.supports_member_view() {
            self.status = "服务器版本过旧，需要升级后才能使用成员视角".to_string();
            return;
        }
        let selected = match &self.view_user {
            None => 0,
            Some(id) => self
                .users
                .iter()
                .position(|user| &user.id == id)
                .map(|index| index + 1)
                .unwrap_or(0),
        };
        self.modal = Some(Modal::Members(MemberPanel { selected }));
    }

    fn apply_member_view(&mut self, selected: usize) {
        if selected == 0 {
            self.view_user = None;
            self.status = "已回到共享视角".to_string();
        } else if let Some(user) = self.users.get(selected - 1) {
            let name = user.name.clone();
            let own = user.id == self.me_id;
            self.view_user = Some(user.id.clone());
            self.status = if own {
                format!("正在查看自己的视角（{name}）")
            } else {
                format!("正在查看 {name} 的视角（只读，可以评论）")
            };
        }
        self.selected_page_index = 0;
        self.selected_block_index = 0;
        self.selected_today_index = 0;
        self.selected_today_habit_index = 0;
        self.selected_habit_index = 0;
        self.selected_agenda_index = 0;
        self.selected_inbox_index = 0;
        self.selected_journal_index = 0;
        self.focus = Focus::Pages;
        self.today_focus = TodayFocus::Items;
        self.journal_source = TodayFocus::Items;
        self.week_mode = false;
        self.workspace_journal_mode = false;
        self.clamp();
    }

    /// 搜索结果的可读标签（页面标题 / 内容块文字 / 习惯名）。
    pub fn search_hit_label(&self, hit: &SearchHit) -> String {
        match hit {
            SearchHit::Page(id) => self
                .pages
                .iter()
                .find(|page| &page.id == id && !page.deleted)
                .map(|page| page.title.clone())
                .unwrap_or_else(|| "已删除页面".to_string()),
            SearchHit::Block { page_id, block_id } => {
                let text = self
                    .blocks
                    .iter()
                    .find(|block| &block.id == block_id && !block.deleted)
                    .map(|block| {
                        if block.text.is_empty() {
                            block.kind.icon().to_string()
                        } else {
                            block.text.clone()
                        }
                    })
                    .unwrap_or_else(|| "已删除内容块".to_string());
                match self.pages.iter().find(|page| &page.id == page_id) {
                    Some(page) => format!("{text}  ·  {}", page.title),
                    None => text,
                }
            }
            SearchHit::Habit(id) => self
                .habits
                .iter()
                .find(|habit| &habit.id == id && !habit.deleted)
                .map(|habit| habit.title.clone())
                .unwrap_or_else(|| "已删除习惯".to_string()),
        }
    }

    fn compute_search(&self, query: &str) -> Vec<SearchHit> {
        let query = query.to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let mut results = Vec::new();
        for page in self.pages.iter().filter(|page| !page.deleted) {
            if page.title.to_lowercase().contains(&query) {
                results.push(SearchHit::Page(page.id.clone()));
            }
        }
        for block in self
            .blocks
            .iter()
            .filter(|block| !block.deleted && self.owner_visible(&block.owner_id))
        {
            if block.text.to_lowercase().contains(&query) {
                results.push(SearchHit::Block {
                    page_id: block.page_id.clone(),
                    block_id: block.id.clone(),
                });
            }
        }
        for habit in self
            .habits
            .iter()
            .filter(|habit| !habit.deleted && self.owner_visible(&habit.owner_id))
        {
            if habit.title.to_lowercase().contains(&query) {
                results.push(SearchHit::Habit(habit.id.clone()));
            }
        }
        results
    }

    fn jump_to_search_hit(&mut self, hit: SearchHit) {
        self.modal = None;
        match hit {
            SearchHit::Page(page_id) => {
                self.show_all_pages = true;
                self.view = View::Workspace;
                self.focus = Focus::Pages;
                if let Some(index) = self
                    .visible_pages()
                    .iter()
                    .position(|page| page.id == page_id)
                {
                    self.selected_page_index = index;
                }
            }
            SearchHit::Block { page_id, block_id } => {
                self.show_all_pages = true;
                self.view = View::Workspace;
                self.focus = Focus::Blocks;
                if let Some(index) = self
                    .visible_pages()
                    .iter()
                    .position(|page| page.id == page_id)
                {
                    self.selected_page_index = index;
                }
                if let Some(index) = self
                    .visible_blocks()
                    .iter()
                    .position(|block| block.id == block_id)
                {
                    self.selected_block_index = index;
                }
            }
            SearchHit::Habit(habit_id) => {
                self.view = View::Habits;
                if let Some(index) = self
                    .visible_habits()
                    .iter()
                    .position(|habit| habit.id == habit_id)
                {
                    self.selected_habit_index = index;
                }
            }
        }
        self.clamp();
    }

    fn handle_modal_key(&mut self, key: KeyEvent) {
        let Some(mut modal) = self.modal.take() else {
            return;
        };
        // 他人视角下议程只读；评论面板只能浏览和新增，不能改/删/解决。
        let read_only_blocked = self.read_only()
            && match &modal {
                Modal::Agenda(_) => !matches!(
                    key.code,
                    KeyCode::Esc
                        | KeyCode::Up
                        | KeyCode::Down
                        | KeyCode::Char('j')
                        | KeyCode::Char('k')
                ),
                Modal::Comments(_) => !matches!(
                    key.code,
                    KeyCode::Esc
                        | KeyCode::Up
                        | KeyCode::Down
                        | KeyCode::Char('j')
                        | KeyCode::Char('k')
                        | KeyCode::Char('a')
                ),
                _ => false,
            };
        if read_only_blocked {
            self.status = "正在查看他人视角（只读），可以评论但不能修改".to_string();
            self.modal = Some(modal);
            return;
        }
        match &mut modal {
            Modal::EventForm(form) => match key.code {
                KeyCode::Esc => return,
                KeyCode::Tab | KeyCode::Down => form.field = (form.field + 1) % 4,
                KeyCode::Up => form.field = (form.field + 3) % 4,
                KeyCode::Backspace => {
                    form.field_mut().pop();
                }
                KeyCode::Char(c) => form.field_mut().push(c),
                KeyCode::Enter => {
                    if let Some(message) = self.submit_event_form(form) {
                        form.error = Some(message);
                    } else {
                        return;
                    }
                }
                _ => {}
            },
            Modal::HabitForm(form) => match key.code {
                KeyCode::Esc => return,
                KeyCode::Tab | KeyCode::Down => form.field = (form.field + 1) % 2,
                KeyCode::Up => form.field = (form.field + 1) % 2,
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    form.frequency = next_frequency(form.frequency)
                }
                KeyCode::Backspace => {
                    form.field_mut().pop();
                }
                KeyCode::Char(c) => form.field_mut().push(c),
                KeyCode::Enter => {
                    if let Some(message) = self.submit_habit_form(form) {
                        form.error = Some(message);
                    } else {
                        return;
                    }
                }
                _ => {}
            },
            Modal::Agenda(panel) => match key.code {
                KeyCode::Esc => return,
                KeyCode::Up | KeyCode::Char('k') => {
                    panel.selected = panel.selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let count = self.calendar_blocks(panel.date).len();
                    if count > 0 {
                        panel.selected = (panel.selected + 1).min(count - 1);
                    }
                }
                KeyCode::Char(' ') => {
                    if let Some(block) = self
                        .calendar_blocks(panel.date)
                        .get(panel.selected)
                        .map(|block| (*block).clone())
                    {
                        self.patch_block(
                            &block.id,
                            block.version,
                            BlockPatch {
                                checked: Some(!block.checked),
                                ..Default::default()
                            },
                        );
                    }
                }
                KeyCode::Enter | KeyCode::Char('e') => {
                    if let Some(block) = self
                        .calendar_blocks(panel.date)
                        .get(panel.selected)
                        .map(|block| (*block).clone())
                    {
                        self.editing = Some(Editing {
                            target: EditTarget::BlockText {
                                block_id: block.id.clone(),
                                base_version: block.version,
                            },
                            buffer: block.text.clone(),
                            cursor: block.text.len(),
                            multiline: false,
                        });
                        return;
                    }
                }
                KeyCode::Char('d') => {
                    if let Some(block) = self
                        .calendar_blocks(panel.date)
                        .get(panel.selected)
                        .map(|block| (*block).clone())
                    {
                        self.request_delete(PendingDelete::Block {
                            id: block.id.clone(),
                            version: block.version,
                            label: format!(
                                "待办「{}」",
                                if block.text.is_empty() {
                                    block.kind.icon()
                                } else {
                                    block.text.as_str()
                                }
                            ),
                        });
                        return;
                    }
                }
                KeyCode::Char('t') => {
                    let date = panel.date.format("%Y-%m-%d").to_string();
                    self.open_event_form(BlockKind::Todo, date, None);
                    return;
                }
                KeyCode::Char('c') => {
                    let date = panel.date.format("%Y-%m-%d").to_string();
                    self.open_event_form(BlockKind::CalendarEvent, date, None);
                    return;
                }
                _ => {}
            },
            Modal::Comments(panel) => match key.code {
                KeyCode::Esc => return,
                KeyCode::Up | KeyCode::Char('k') => {
                    panel.selected = panel.selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let count = self.comments_for(panel.target_kind, &panel.target_id).len();
                    if count > 0 {
                        panel.selected = (panel.selected + 1).min(count - 1);
                    }
                }
                KeyCode::Char('a') => {
                    self.editing = Some(Editing {
                        target: EditTarget::NewComment {
                            target_kind: panel.target_kind,
                            target_id: panel.target_id.clone(),
                        },
                        buffer: String::new(),
                        cursor: 0,
                        multiline: true,
                    });
                    return;
                }
                KeyCode::Char('r') => {
                    if let Some(comment) = self
                        .comments_for(panel.target_kind, &panel.target_id)
                        .get(panel.selected)
                        .cloned()
                    {
                        self.update_comment(
                            &comment.id,
                            UpdateCommentRequest {
                                expected_version: comment.version,
                                body: None,
                                resolved: Some(!comment.resolved),
                                deleted: None,
                            },
                        );
                    }
                }
                KeyCode::Char('d') => {
                    if let Some(comment) = self
                        .comments_for(panel.target_kind, &panel.target_id)
                        .get(panel.selected)
                        .cloned()
                    {
                        self.request_delete(PendingDelete::Comment {
                            id: comment.id,
                            version: comment.version,
                        });
                        return;
                    }
                }
                KeyCode::Char('e') => {
                    if let Some(comment) = self
                        .comments_for(panel.target_kind, &panel.target_id)
                        .get(panel.selected)
                        .cloned()
                    {
                        self.editing = Some(Editing {
                            target: EditTarget::CommentBody {
                                comment_id: comment.id.clone(),
                                base_version: comment.version,
                            },
                            buffer: comment.body.clone(),
                            cursor: comment.body.len(),
                            multiline: true,
                        });
                        return;
                    }
                }
                _ => {}
            },
            Modal::Invites(panel) => match key.code {
                KeyCode::Esc => return,
                KeyCode::Up | KeyCode::Char('k') => {
                    panel.selected = panel.selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !panel.invites.is_empty() {
                        panel.selected = (panel.selected + 1).min(panel.invites.len() - 1);
                    }
                }
                KeyCode::Char('g') => {
                    let request = CreateInviteRequest {
                        note: format!("由 {} 创建", self.me),
                        expires_in_days: None,
                        // 默认一次性邀请码，用一个后自动失效。
                        max_uses: Some(1),
                    };
                    match self.api.create_invite(&request) {
                        Ok(response) => {
                            panel.invites.insert(0, response.invite);
                            panel.selected = 0;
                            panel.message = Some("已生成新邀请码".to_string());
                        }
                        Err(err) => panel.message = Some(err.to_string()),
                    }
                }
                KeyCode::Char('x') => {
                    if let Some(invite) = panel.invites.get(panel.selected).cloned() {
                        match self.api.update_invite(
                            &invite.id,
                            &UpdateInviteRequest {
                                revoked: Some(!invite.revoked),
                            },
                        ) {
                            Ok(response) => {
                                panel.invites[panel.selected] = response.invite;
                                panel.message = Some("状态已更新".to_string());
                            }
                            Err(err) => panel.message = Some(err.to_string()),
                        }
                    }
                }
                _ => {}
            },
            Modal::Members(panel) => match key.code {
                KeyCode::Esc => return,
                KeyCode::Up | KeyCode::Char('k') => {
                    panel.selected = panel.selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    panel.selected = (panel.selected + 1).min(self.users.len());
                }
                KeyCode::Enter => {
                    let selected = panel.selected;
                    self.apply_member_view(selected);
                    return;
                }
                _ => {}
            },
            Modal::Search(state) => match key.code {
                KeyCode::Esc => return,
                KeyCode::Backspace => {
                    state.query.pop();
                    state.results = self.compute_search(&state.query);
                    state.selected = 0;
                }
                KeyCode::Char(c) => {
                    state.query.push(c);
                    state.results = self.compute_search(&state.query);
                    state.selected = 0;
                }
                KeyCode::Up => state.selected = state.selected.saturating_sub(1),
                KeyCode::Down => {
                    if !state.results.is_empty() {
                        state.selected = (state.selected + 1).min(state.results.len() - 1);
                    }
                }
                KeyCode::Enter => {
                    if let Some(hit) = state.results.get(state.selected).cloned() {
                        self.jump_to_search_hit(hit);
                    }
                    return;
                }
                _ => {}
            },
            Modal::Confirm(confirm) => match key.code {
                KeyCode::Esc => return,
                KeyCode::Backspace => {
                    confirm.input.pop();
                }
                KeyCode::Char(c) => confirm.input.push(c),
                KeyCode::Enter => {
                    if confirm.input.trim() == "delete" {
                        let action = confirm.action.clone();
                        self.confirm_delete(action);
                        return;
                    }
                    confirm.input.clear();
                }
                _ => {}
            },
            Modal::Help => return,
        }
        self.modal = Some(modal);
    }

    fn submit_event_form(&mut self, form: &EventForm) -> Option<String> {
        if form.title.trim().is_empty() {
            return Some("标题不能为空".to_string());
        }
        if !totion_core::valid_date(&form.date) {
            return Some("日期格式应为 YYYY-MM-DD".to_string());
        }
        for time in [&form.start_time, &form.end_time] {
            if !time.is_empty() && !totion_core::valid_time(time) {
                return Some("时间格式应为 HH:MM".to_string());
            }
        }
        let dates = match expand_range(&form.date, form.range_end.as_deref()) {
            Some(dates) => dates,
            None => return Some("日期范围无效或超过 62 天".to_string()),
        };

        let mut created = 0;
        for date in &dates {
            let Some(page_id) = self.ensure_day(date) else {
                return Some(format!("创建 {date} 的页面失败"));
            };
            let position = self
                .blocks
                .iter()
                .filter(|block| {
                    !block.deleted
                        && block.page_id == page_id
                        && self.owner_visible(&block.owner_id)
                })
                .count() as i64;
            let request = CreateBlockRequest {
                page_id,
                kind: form.kind,
                text: form.title.trim().to_string(),
                due_date: Some(date.clone()),
                position,
                start_time: if form.start_time.is_empty() {
                    None
                } else {
                    Some(form.start_time.clone())
                },
                end_time: if form.end_time.is_empty() {
                    None
                } else {
                    Some(form.end_time.clone())
                },
                section: None,
            };
            match self.api.create_block(&request) {
                Ok(response) => {
                    self.upsert_block(response.block);
                    created += 1;
                }
                Err(err) => return Some(err.to_string()),
            }
        }
        self.clamp();
        self.status = format!("已添加 {created} 条");
        None
    }

    fn submit_habit_form(&mut self, form: &HabitForm) -> Option<String> {
        let title = form.title.trim();
        if title.is_empty() {
            return Some("习惯名称不能为空".to_string());
        }
        let dates = match expand_range(&form.date, form.range_end.as_deref()) {
            Some(dates) => dates,
            None => return Some("日期范围无效或超过 62 天".to_string()),
        };
        let position = self.visible_habits().len() as i64;
        let request = CreateHabitRequest {
            title: title.to_string(),
            icon: "🌱".to_string(),
            note: form.note.trim().to_string(),
            frequency: form.frequency,
            position,
        };
        match self.api.create_habit(&request) {
            Ok(response) => {
                let habit_id = response.habit.id.clone();
                self.upsert_habit(response.habit);
                let mut checked = 0;
                if form.range_end.is_some() {
                    for date in &dates {
                        match self.api.check_in(
                            &habit_id,
                            &CheckinRequest {
                                date: date.clone(),
                                note: None,
                            },
                        ) {
                            Ok(checkin) => {
                                self.upsert_checkin(checkin.checkin);
                                checked += 1;
                            }
                            Err(err) => {
                                self.status = err.to_string();
                                break;
                            }
                        }
                    }
                }
                self.view = View::Habits;
                self.clamp();
                if let Some(index) = self
                    .visible_habits()
                    .iter()
                    .position(|habit| habit.id == habit_id)
                {
                    self.selected_habit_index = index;
                }
                self.status = if checked > 0 {
                    format!("习惯已创建，并在 {checked} 天打卡")
                } else {
                    "习惯已创建".to_string()
                };
                None
            }
            Err(err) => Some(err.to_string()),
        }
    }
}

fn expand_range(start: &str, end: Option<&str>) -> Option<Vec<String>> {
    let start_date = totion_core::parse_date(start)?;
    let end_date = match end.filter(|value| !value.is_empty()) {
        Some(value) => totion_core::parse_date(value)?,
        None => start_date,
    };
    let (start_date, end_date) = if start_date <= end_date {
        (start_date, end_date)
    } else {
        (end_date, start_date)
    };
    let mut dates = Vec::new();
    let mut cursor = start_date;
    while cursor <= end_date {
        dates.push(cursor.format("%Y-%m-%d").to_string());
        if dates.len() > 62 {
            return None;
        }
        cursor += ChronoDuration::days(1);
    }
    Some(dates)
}

#[derive(Default)]
struct BlockPatch {
    text: Option<String>,
    checked: Option<bool>,
    due_date: Option<String>,
    streak: Option<i64>,
    start_time: Option<String>,
    end_time: Option<String>,
    deleted: Option<bool>,
}

fn next_frequency(frequency: HabitFrequency) -> HabitFrequency {
    match frequency {
        HabitFrequency::Daily => HabitFrequency::Weekdays,
        HabitFrequency::Weekdays => HabitFrequency::Weekly { times: 3 },
        HabitFrequency::Weekly { times } if times < 7 => {
            HabitFrequency::Weekly { times: times + 1 }
        }
        HabitFrequency::Weekly { .. } => HabitFrequency::Daily,
    }
}

impl EventForm {
    fn field_mut(&mut self) -> &mut String {
        match self.field {
            0 => &mut self.title,
            1 => &mut self.date,
            2 => &mut self.start_time,
            _ => &mut self.end_time,
        }
    }
}

impl HabitForm {
    fn field_mut(&mut self) -> &mut String {
        match self.field {
            0 => &mut self.title,
            _ => &mut self.note,
        }
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
