use std::time::{Duration, Instant};

use chrono::{Datelike, Duration as ChronoDuration, Local, Months, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use totion_core::{
    block_day_stats, checkin_on, completion_series, habit_check_in, habit_due_on, habit_stats,
    AuthResponse, Block, BlockKind, BlockSection, Change, CheckinRequest, Comment,
    CommentTargetKind, CreateBlockRequest, CreateCommentRequest, CreateHabitRequest,
    CreatePageRequest, DayStat, EntityKind, Habit, HabitCheckin, HabitFrequency, HabitStats, Page,
    PageKind, RegisterRequest, Snapshot, UpdateBlockRequest, UpdateCheckinRequest,
    UpdateCommentRequest, UpdateHabitRequest, UpdatePageRequest, User, VerifyRequest,
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
}

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Today => "今日",
            View::Workspace => "工作区",
            View::Calendar => "日历",
            View::Habits => "习惯",
            View::Stats => "统计",
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
}

#[derive(Debug, Clone)]
pub struct HabitForm {
    pub title: String,
    pub note: String,
    pub frequency: HabitFrequency,
    pub field: usize,
    pub error: Option<String>,
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
pub enum Modal {
    EventForm(EventForm),
    HabitForm(HabitForm),
    Comments(CommentPanel),
    Search(SearchState),
    Help,
}

#[derive(Debug, Clone)]
pub enum TodayRow {
    Todo(Block),
    Event(Block),
    Habit(Habit, HabitStats),
    Diary(Block),
}

impl TodayRow {
    pub fn block(&self) -> Option<&Block> {
        match self {
            TodayRow::Todo(block) | TodayRow::Event(block) | TodayRow::Diary(block) => Some(block),
            TodayRow::Habit(_, _) => None,
        }
    }
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
            Ok(snapshot) => Some(Screen::Main(MainState::new(api, name, snapshot))),
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
    pub fn boot() -> Self {
        let screen = match config::load_identity() {
            Ok(Some(identity)) => {
                let mut api = Api::new(&identity.server_url);
                match login(&api, &identity) {
                    Ok(auth) => {
                        api.set_token(Some(auth.token.clone()));
                        match api.snapshot() {
                            Ok(snapshot) => {
                                Screen::Main(MainState::new(api, identity.name.clone(), snapshot))
                            }
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
    workspace_name: String,
    users: Vec<User>,
    pub pages: Vec<Page>,
    pub blocks: Vec<Block>,
    pub habits: Vec<Habit>,
    pub checkins: Vec<HabitCheckin>,
    pub comments: Vec<Comment>,
    last_seq: i64,
    selected_page_index: usize,
    selected_block_index: usize,
    selected_today_index: usize,
    selected_habit_index: usize,
    selected_agenda_index: usize,
    focus: Focus,
    view: View,
    editing: Option<Editing>,
    modal: Option<Modal>,
    status: String,
    notice: Option<String>,
    last_sync: Option<Instant>,
    calendar_month: NaiveDate,
    selected_day: NaiveDate,
    today: NaiveDate,
    ensured_date: Option<String>,
    daily_page_id: Option<String>,
    show_all_pages: bool,
    caps: ServerCaps,
    stats_days: u32,
}

impl MainState {
    fn new(api: Api, me: String, snapshot: Snapshot) -> Self {
        let today = Local::now().date_naive();
        let mut state = Self {
            api,
            me,
            workspace_name: snapshot.workspace.name,
            users: snapshot.workspace.users,
            pages: snapshot.pages,
            blocks: snapshot.blocks,
            habits: snapshot.habits,
            checkins: snapshot.checkins,
            comments: snapshot.comments,
            last_seq: snapshot.last_seq,
            selected_page_index: 0,
            selected_block_index: 0,
            selected_today_index: 0,
            selected_habit_index: 0,
            selected_agenda_index: 0,
            focus: Focus::Pages,
            view: View::Today,
            editing: None,
            modal: None,
            status: "已连接".to_string(),
            notice: None,
            last_sync: Some(Instant::now()),
            calendar_month: today.with_day(1).unwrap_or(today),
            selected_day: today,
            today,
            ensured_date: None,
            daily_page_id: None,
            show_all_pages: false,
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

    pub fn status(&self) -> &str {
        &self.status
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

    pub fn selected_habit_index(&self) -> usize {
        self.selected_habit_index
    }

    pub fn selected_agenda_index(&self) -> usize {
        self.selected_agenda_index
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

    pub fn supports_v2(&self) -> bool {
        self.caps.supports_v2()
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
            .filter(|block| !block.deleted && block.page_id == page_id)
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
            .filter(|habit| !habit.deleted && !habit.archived)
            .cloned()
            .collect();
        habits.sort_by_key(|habit| (habit.position, habit.created_at));
        habits
    }

    pub fn habit_stats(&self, habit: &Habit) -> HabitStats {
        habit_stats(habit.frequency, &habit.id, &self.checkins, self.today)
    }

    pub fn today_key(&self) -> String {
        self.today.format("%Y-%m-%d").to_string()
    }

    pub fn daily_page(&self) -> Option<&Page> {
        let key = self.today_key();
        self.pages
            .iter()
            .find(|page| !page.deleted && page.date.as_deref() == Some(key.as_str()))
    }

    pub fn diary_block(&self, page_id: &str, section: BlockSection) -> Option<Block> {
        self.blocks
            .iter()
            .filter(|block| {
                !block.deleted && block.page_id == page_id && block.section == Some(section)
            })
            .max_by_key(|block| block.updated_at)
            .cloned()
    }

    pub fn today_rows(&self) -> Vec<TodayRow> {
        let key = self.today_key();
        let mut rows = Vec::new();
        let mut blocks: Vec<Block> = self
            .blocks
            .iter()
            .filter(|block| {
                !block.deleted
                    && block.due_date.as_deref() == Some(key.as_str())
                    && matches!(block.kind, BlockKind::Todo | BlockKind::CalendarEvent)
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
            let stats = self.habit_stats(&habit);
            if stats.checked_today || habit_due_on(habit.frequency, self.today, stats.weekly_done) {
                rows.push(TodayRow::Habit(habit, stats));
            }
        }

        if let Some(page) = self.daily_page() {
            for section in BlockSection::ALL {
                if let Some(block) = self.diary_block(&page.id, section) {
                    rows.push(TodayRow::Diary(block));
                }
            }
        }
        rows
    }

    pub fn today_completion(&self) -> DayStat {
        let mut stat = block_day_stats(&self.blocks, &self.today_key());
        for habit in self.visible_habits() {
            let stats = self.habit_stats(&habit);
            if habit_due_on(habit.frequency, self.today, stats.weekly_done) {
                stat.total += 1;
                if stats.checked_today {
                    stat.done += 1;
                }
            }
        }
        stat
    }

    pub fn completion_series(&self, days: u32) -> Vec<(String, DayStat)> {
        completion_series(&self.blocks, days, self.today)
    }

    pub fn heatmap_grid(&self, habit: &Habit, weeks: u32) -> Vec<Vec<bool>> {
        totion_core::heatmap_grid(&self.checkins, &habit.id, weeks, self.today)
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
        let today_count = self.today_rows().len();
        self.selected_today_index = if today_count == 0 {
            0
        } else {
            self.selected_today_index.min(today_count - 1)
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
    }

    fn refresh_daily_page(&mut self) {
        self.daily_page_id = self.daily_page().map(|page| page.id.clone());
    }

    pub fn tick(&mut self) {
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
                self.status = format!("创建 {date} 日记页失败: {err}");
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
            KeyCode::Char('?') => self.modal = Some(Modal::Help),
            KeyCode::Char('/') => self.open_search(),
            KeyCode::Char('C') => self.open_comments(),
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
            },
        }
        false
    }

    fn set_view(&mut self, view: View) {
        self.view = view;
        self.clamp();
    }

    fn handle_workspace_key(&mut self, key: KeyEvent) {
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
            KeyCode::Char('c') => self.new_workspace_block(BlockKind::CalendarEvent),
            KeyCode::Char('N') => self.new_page(),
            KeyCode::Char('a') => {
                self.show_all_pages = !self.show_all_pages;
                self.clamp();
            }
            KeyCode::Char(' ') => self.toggle_workspace_block(),
            KeyCode::Char('d') => self.delete_workspace_current(),
            _ => {}
        }
    }

    fn handle_today_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_today(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_today(1),
            KeyCode::Char(' ') => self.toggle_today_item(),
            KeyCode::Char('u') => self.undo_today_habit(),
            KeyCode::Enter | KeyCode::Char('e') => self.edit_today_item(),
            KeyCode::Char('d') => self.delete_today_item(),
            KeyCode::Char('t') => self.open_event_form(BlockKind::Todo, self.today_key()),
            KeyCode::Char('c') => self.open_event_form(BlockKind::CalendarEvent, self.today_key()),
            KeyCode::Char('n') => self.open_habit_form(),
            _ => {}
        }
    }

    fn handle_calendar_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.shift_day(-1),
            KeyCode::Right | KeyCode::Char('l') => self.shift_day(1),
            KeyCode::Up | KeyCode::Char('k') => self.shift_day(-7),
            KeyCode::Down | KeyCode::Char('j') => self.shift_day(7),
            KeyCode::Char('[') => self.shift_month(-1),
            KeyCode::Char(']') => self.shift_month(1),
            KeyCode::Char(' ') => self.toggle_agenda_item(),
            KeyCode::Enter | KeyCode::Char('e') => self.edit_agenda_item(),
            KeyCode::Char('d') => self.delete_agenda_item(),
            KeyCode::Char('c') => self.open_event_form(
                BlockKind::CalendarEvent,
                self.selected_day.format("%Y-%m-%d").to_string(),
            ),
            KeyCode::Char('t') => self.open_event_form(
                BlockKind::Todo,
                self.selected_day.format("%Y-%m-%d").to_string(),
            ),
            KeyCode::Char('n') => self.open_habit_form(),
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
            KeyCode::Char('n') => self.open_habit_form(),
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
        let count = self.today_rows().len();
        if count > 0 {
            let next = self.selected_today_index as i64 + delta;
            self.selected_today_index = next.clamp(0, count as i64 - 1) as usize;
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
        let rows = self.today_rows();
        let Some(row) = rows.get(self.selected_today_index) else {
            return;
        };
        match row {
            TodayRow::Todo(block) | TodayRow::Event(block) | TodayRow::Diary(block) => {
                self.editing = Some(Editing {
                    target: EditTarget::BlockText {
                        block_id: block.id.clone(),
                        base_version: block.version,
                    },
                    buffer: block.text.clone(),
                    cursor: block.text.len(),
                    multiline: block.section.is_some(),
                });
            }
            TodayRow::Habit(habit, _) => {
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
    }

    fn edit_agenda_item(&mut self) {
        let blocks = self.agenda_blocks();
        if let Some(block) = blocks.get(self.selected_agenda_index) {
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

    fn edit_selected_habit(&mut self) {
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
                        self.modal = Some(Modal::Comments(CommentPanel {
                            target_kind,
                            target_id,
                            label: "评论".to_string(),
                            selected: 0,
                        }));
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
        let rows = self.today_rows();
        let Some(row) = rows.get(self.selected_today_index) else {
            return;
        };
        match row {
            TodayRow::Todo(block) | TodayRow::Event(block) => {
                self.patch_block(
                    &block.id,
                    block.version,
                    BlockPatch {
                        checked: Some(!block.checked),
                        ..Default::default()
                    },
                );
            }
            TodayRow::Habit(habit, stats) => {
                if stats.checked_today {
                    self.status = "今天已经打过卡了".to_string();
                } else {
                    self.check_in(habit);
                }
            }
            TodayRow::Diary(_) => self.status = "日记记录不需要勾选".to_string(),
        }
    }

    fn check_in(&mut self, habit: &Habit) {
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
        let habits = self.visible_habits();
        if let Some(habit) = habits.get(self.selected_habit_index) {
            self.undo_checkin(habit);
        }
    }

    fn undo_today_habit(&mut self) {
        let rows = self.today_rows();
        if let Some(TodayRow::Habit(habit, _)) = rows.get(self.selected_today_index) {
            self.undo_checkin(habit);
        }
    }

    fn delete_workspace_current(&mut self) {
        match self.focus {
            Focus::Pages => {
                if let Some(page) = self.current_page() {
                    self.patch_page(&page.id, page.version, None, Some(true));
                    self.status = format!("已删除页面「{}」", page.title);
                }
            }
            Focus::Blocks => {
                if let Some(block) = self.current_block() {
                    self.patch_block(
                        &block.id,
                        block.version,
                        BlockPatch {
                            deleted: Some(true),
                            ..Default::default()
                        },
                    );
                    self.status = "已删除内容块".to_string();
                }
            }
        }
    }

    fn delete_today_item(&mut self) {
        let rows = self.today_rows();
        let Some(row) = rows.get(self.selected_today_index) else {
            return;
        };
        match row {
            TodayRow::Todo(block) | TodayRow::Event(block) | TodayRow::Diary(block) => {
                self.patch_block(
                    &block.id,
                    block.version,
                    BlockPatch {
                        deleted: Some(true),
                        ..Default::default()
                    },
                );
                self.status = "已删除内容块".to_string();
            }
            TodayRow::Habit(habit, _) => self.delete_habit(habit),
        }
    }

    fn delete_agenda_item(&mut self) {
        let blocks = self.agenda_blocks();
        if let Some(block) = blocks.get(self.selected_agenda_index) {
            self.patch_block(
                &block.id,
                block.version,
                BlockPatch {
                    deleted: Some(true),
                    ..Default::default()
                },
            );
            self.status = "已删除日程".to_string();
        }
    }

    fn delete_habit(&mut self, habit: &Habit) {
        self.update_habit(
            &habit.id,
            UpdateHabitRequest {
                expected_version: habit.version,
                title: None,
                icon: None,
                note: None,
                frequency: None,
                position: None,
                archived: None,
                deleted: Some(true),
            },
        );
        self.status = format!("已删除习惯「{}」", habit.title);
    }

    fn delete_selected_habit(&mut self) {
        let habits = self.visible_habits();
        if let Some(habit) = habits.get(self.selected_habit_index) {
            self.delete_habit(habit);
        }
    }

    fn toggle_agenda_item(&mut self) {
        let blocks = self.agenda_blocks();
        if let Some(block) = blocks.get(self.selected_agenda_index) {
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

    fn open_event_form(&mut self, kind: BlockKind, date: String) {
        self.modal = Some(Modal::EventForm(EventForm {
            kind,
            title: String::new(),
            date,
            start_time: String::new(),
            end_time: String::new(),
            field: 0,
            error: None,
        }));
    }

    fn open_habit_form(&mut self) {
        self.modal = Some(Modal::HabitForm(HabitForm {
            title: String::new(),
            note: String::new(),
            frequency: HabitFrequency::Daily,
            field: 0,
            error: None,
        }));
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
            View::Today => self
                .today_rows()
                .get(self.selected_today_index)
                .map(|row| match row {
                    TodayRow::Habit(habit, _) => (
                        CommentTargetKind::Habit,
                        habit.id.clone(),
                        format!("习惯「{}」", habit.title),
                    ),
                    _ => {
                        let block = row.block().expect("block row");
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
                    }
                }),
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
        }
    }

    fn open_search(&mut self) {
        self.modal = Some(Modal::Search(SearchState {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
        }));
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
        for block in self.blocks.iter().filter(|block| !block.deleted) {
            if block.text.to_lowercase().contains(&query) {
                results.push(SearchHit::Block {
                    page_id: block.page_id.clone(),
                    block_id: block.id.clone(),
                });
            }
        }
        for habit in self.habits.iter().filter(|habit| !habit.deleted) {
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
                        self.update_comment(
                            &comment.id,
                            UpdateCommentRequest {
                                expected_version: comment.version,
                                body: None,
                                resolved: None,
                                deleted: Some(true),
                            },
                        );
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
        let page_id = self.ensure_day(&form.date)?;
        let position = self
            .blocks
            .iter()
            .filter(|block| !block.deleted && block.page_id == page_id)
            .count() as i64;
        let request = CreateBlockRequest {
            page_id,
            kind: form.kind,
            text: form.title.trim().to_string(),
            due_date: Some(form.date.clone()),
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
                self.clamp();
                self.status = "已添加".to_string();
                None
            }
            Err(err) => Some(err.to_string()),
        }
    }

    fn submit_habit_form(&mut self, form: &HabitForm) -> Option<String> {
        let title = form.title.trim();
        if title.is_empty() {
            return Some("习惯名称不能为空".to_string());
        }
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
                self.view = View::Habits;
                self.clamp();
                if let Some(index) = self
                    .visible_habits()
                    .iter()
                    .position(|habit| habit.id == habit_id)
                {
                    self.selected_habit_index = index;
                }
                self.status = "习惯已创建".to_string();
                None
            }
            Err(err) => Some(err.to_string()),
        }
    }
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
