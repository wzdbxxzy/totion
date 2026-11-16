use std::time::{Duration, Instant};

use chrono::{Datelike, Local, Months, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use totion_core::{
    habit_check_in, AuthResponse, Block, BlockKind, Change, CreateBlockRequest, CreatePageRequest,
    EntityKind, Page, PageKind, RegisterRequest, Snapshot, UpdateBlockRequest, UpdatePageRequest,
    User, VerifyRequest,
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
    Workspace,
    Calendar,
    Habits,
}

#[derive(Debug, Clone)]
pub enum EditTarget {
    PageTitle { page_id: String, base_version: i64 },
    BlockText { block_id: String, base_version: i64 },
}

#[derive(Debug, Clone)]
pub struct Editing {
    pub target: EditTarget,
    pub buffer: String,
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

#[derive(Default)]
struct BlockPatch {
    text: Option<String>,
    checked: Option<bool>,
    due_date: Option<String>,
    streak: Option<i64>,
    deleted: Option<bool>,
}

pub struct MainState {
    api: Api,
    me: String,
    workspace_name: String,
    users: Vec<User>,
    pages: Vec<Page>,
    blocks: Vec<Block>,
    last_seq: i64,
    selected_page_index: usize,
    selected_block_index: usize,
    focus: Focus,
    view: View,
    editing: Option<Editing>,
    status: String,
    notice: Option<String>,
    last_sync: Option<Instant>,
    calendar_month: NaiveDate,
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
            last_seq: snapshot.last_seq,
            selected_page_index: 0,
            selected_block_index: 0,
            focus: Focus::Pages,
            view: View::Workspace,
            editing: None,
            status: "已连接".to_string(),
            notice: None,
            last_sync: Some(Instant::now()),
            calendar_month: today.with_day(1).unwrap_or(today),
        };
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

    pub fn editing(&self) -> Option<&Editing> {
        self.editing.as_ref()
    }

    pub fn calendar_month(&self) -> NaiveDate {
        self.calendar_month
    }

    pub fn visible_pages(&self) -> Vec<&Page> {
        let mut pages: Vec<&Page> = self.pages.iter().filter(|page| !page.deleted).collect();
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

    pub fn all_habit_blocks(&self) -> Vec<(&Block, &Page)> {
        let mut result: Vec<(&Block, &Page)> = Vec::new();
        for block in self
            .blocks
            .iter()
            .filter(|b| !b.deleted && b.kind == BlockKind::Habit)
        {
            if let Some(page) = self
                .pages
                .iter()
                .find(|p| p.id == block.page_id && !p.deleted)
            {
                result.push((block, page));
            }
        }
        result.sort_by_key(|(block, _)| block.position);
        result
    }

    pub fn calendar_blocks(&self, date: NaiveDate) -> Vec<&Block> {
        let key = date.format("%Y-%m-%d").to_string();
        self.blocks
            .iter()
            .filter(|block| {
                !block.deleted
                    && block.due_date.as_deref() == Some(key.as_str())
                    && matches!(block.kind, BlockKind::Todo | BlockKind::CalendarEvent)
            })
            .collect()
    }

    fn current_page_id(&self) -> Option<String> {
        self.visible_pages()
            .get(self.selected_page_index)
            .map(|page| page.id.clone())
    }

    fn current_page(&self) -> Option<Page> {
        self.visible_pages()
            .get(self.selected_page_index)
            .cloned()
            .cloned()
    }

    fn current_block(&self) -> Option<Block> {
        self.visible_blocks()
            .get(self.selected_block_index)
            .cloned()
            .cloned()
    }

    fn clamp(&mut self) {
        let page_count = self.visible_pages().len();
        if page_count == 0 {
            self.selected_page_index = 0;
        } else {
            self.selected_page_index = self.selected_page_index.min(page_count - 1);
        }
        let block_count = self.visible_blocks().len();
        if block_count == 0 {
            self.selected_block_index = 0;
        } else {
            self.selected_block_index = self.selected_block_index.min(block_count - 1);
        }
    }

    pub fn tick(&mut self) {
        let due = self
            .last_sync
            .map(|at| at.elapsed() >= SYNC_INTERVAL)
            .unwrap_or(true);
        if due {
            self.sync_now(false);
        }
    }

    fn sync_now(&mut self, manual: bool) {
        self.last_sync = Some(Instant::now());
        match self.api.sync(self.last_seq) {
            Ok(response) => {
                let count = response.changes.len();
                for change in response.changes {
                    self.apply_change(change);
                }
                self.last_seq = self.last_seq.max(response.last_seq);
                self.clamp();
                if manual {
                    self.status = format!("已同步（{count} 条变更）");
                }
            }
            Err(err) => {
                if manual {
                    self.status = format!("同步失败: {err}");
                }
            }
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
        if let Some(editing) = self.editing.as_mut() {
            match key.code {
                KeyCode::Esc => self.editing = None,
                KeyCode::Enter => {
                    let editing = self.editing.take().expect("editing");
                    self.commit_edit(editing);
                }
                KeyCode::Backspace => {
                    editing.buffer.pop();
                }
                KeyCode::Char(c) => editing.buffer.push(c),
                _ => {}
            }
            return false;
        }

        match key.code {
            KeyCode::Char('q') => return true,
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Pages => Focus::Blocks,
                    Focus::Blocks => Focus::Pages,
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Left if self.view == View::Calendar => self.shift_month(-1),
            KeyCode::Right if self.view == View::Calendar => self.shift_month(1),
            KeyCode::Enter | KeyCode::Char('e') => self.start_edit(),
            KeyCode::Char('n') => self.new_block(BlockKind::Text),
            KeyCode::Char('t') => self.new_block(BlockKind::Todo),
            KeyCode::Char('h') => self.new_block(BlockKind::Habit),
            KeyCode::Char('c') => self.new_block(BlockKind::CalendarEvent),
            KeyCode::Char('N') => self.new_page(),
            KeyCode::Char(' ') => self.toggle_or_check(),
            KeyCode::Char('d') => self.delete_current(),
            KeyCode::Char('r') => self.sync_now(true),
            KeyCode::Char('1') => self.view = View::Workspace,
            KeyCode::Char('2') => self.view = View::Calendar,
            KeyCode::Char('3') => self.view = View::Habits,
            KeyCode::Esc => self.notice = None,
            _ => {}
        }
        false
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
            self.status = format!(
                "{} 年 {} 月",
                self.calendar_month.year(),
                self.calendar_month.month()
            );
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
                        buffer: page.title,
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
                        buffer: block.text,
                    });
                }
            }
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
        }
    }

    fn new_block(&mut self, kind: BlockKind) {
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
                });
                self.status = "输入页面标题，Enter 保存".to_string();
            }
            Err(err) => self.status = err.to_string(),
        }
    }

    fn toggle_or_check(&mut self) {
        let Some(block) = self.current_block() else {
            return;
        };
        let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
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
                self.status = if block.checked {
                    "已标记为未完成".to_string()
                } else {
                    "已完成 ✓".to_string()
                };
            }
            BlockKind::Habit => {
                match habit_check_in(block.due_date.as_deref(), &today, block.streak) {
                    Some(streak) => {
                        self.patch_block(
                            &block.id,
                            block.version,
                            BlockPatch {
                                checked: Some(true),
                                due_date: Some(today),
                                streak: Some(streak),
                                ..Default::default()
                            },
                        );
                        self.status = format!("打卡成功，连续 {streak} 天 🔥");
                    }
                    None => self.status = "今天已经打过卡了".to_string(),
                }
            }
            _ => self.status = "这个内容块不支持勾选".to_string(),
        }
    }

    fn delete_current(&mut self) {
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
}
