use std::sync::{Arc, Mutex};

use axum::{
    extract::{Extension, Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::json;
use totion_core::{
    valid_date, valid_time, ApiError, AuthResponse, Block, BlockKind, BlockSection,
    ChallengeRequest, ChallengeResponse, Change, CheckinRequest, Comment, CommentTargetKind,
    CreateBlockRequest, CreateCommentRequest, CreateHabitRequest, CreateInviteRequest,
    CreateJournalRequest, CreatePageRequest, EnsureDayResponse, EntityKind, Habit, HabitCheckin,
    Invite, InviteListResponse, JournalEntry, JournalTarget, Page, PageKind, RegisterRequest,
    Snapshot, SyncResponse, UpdateBlockRequest, UpdateCheckinRequest, UpdateCommentRequest,
    UpdateHabitRequest, UpdateInviteRequest, UpdateJournalRequest, UpdatePageRequest, User,
    VerifyRequest, WorkspaceInfo, WriteBlockResponse, WriteCheckinResponse, WriteCommentResponse,
    WriteHabitResponse, WriteInviteResponse, WriteJournalResponse, WritePageResponse, API_VERSION,
};

use crate::{auth, db};

pub type Db = Arc<Mutex<Connection>>;

pub fn router(db: Db) -> Router {
    let protected = Router::new()
        .route("/api/snapshot", get(snapshot))
        .route("/api/sync", get(sync))
        .route("/api/pages", post(create_page))
        .route("/api/pages/{id}", patch(update_page))
        .route("/api/blocks", post(create_block))
        .route("/api/blocks/{id}", patch(update_block))
        .route("/api/habits", post(create_habit))
        .route("/api/habits/{id}", patch(update_habit))
        .route("/api/habits/{id}/checkins", post(check_in_habit))
        .route("/api/checkins/{id}", patch(update_checkin))
        .route("/api/comments", post(create_comment))
        .route("/api/comments/{id}", patch(update_comment))
        .route("/api/journals", post(create_journal))
        .route("/api/journals/{id}", patch(update_journal))
        .route("/api/invites", get(list_invites).post(create_invite))
        .route("/api/invites/{id}", patch(update_invite))
        .route("/api/days/{date}/ensure", post(ensure_day))
        .route_layer(axum::middleware::from_fn_with_state(
            db.clone(),
            require_auth,
        ));

    Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/register", post(register))
        .route("/api/auth/challenge", post(challenge))
        .route("/api/auth/verify", post(verify))
        .merge(protected)
        .with_state(db)
}

#[derive(Debug)]
pub struct AppError {
    status: StatusCode,
    error: &'static str,
    message: String,
    current: Option<serde_json::Value>,
}

impl AppError {
    pub fn new(status: StatusCode, error: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            error,
            message: message.into(),
            current: None,
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    pub fn conflict(message: impl Into<String>, current: serde_json::Value) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            error: "conflict",
            message: message.into(),
            current: Some(current),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = ApiError {
            error: self.error.to_string(),
            message: self.message,
            current: self.current,
        };
        (self.status, Json(body)).into_response()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::internal(format!("数据库错误: {err}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::internal(format!("序列化错误: {err}"))
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::internal(err.to_string())
    }
}

async fn require_auth(
    State(db): State<Db>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::unauthorized("缺少 Authorization: Bearer <token>"))?;

    let user = {
        let conn = db.lock().unwrap();
        auth::user_for_token(&conn, token)
    };
    let user = user.ok_or_else(|| AppError::unauthorized("登录已过期，请重新登录"))?;
    req.extensions_mut().insert(user);
    Ok(next.run(req).await)
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "service": "totion-server",
        "api_version": API_VERSION,
        "features": ["calendar_time", "habits", "checkins", "daily", "comments", "sync_cursor"]
    }))
}

async fn register(
    State(db): State<Db>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(AppError::bad_request("名字不能为空"));
    }
    let key_bytes = hex::decode(req.public_key.trim())
        .map_err(|_| AppError::bad_request("public_key 必须是 hex 编码"))?;
    if key_bytes.len() != 32 {
        return Err(AppError::bad_request(
            "public_key 必须是 32 字节 Ed25519 公钥",
        ));
    }

    let conn = db.lock().unwrap();
    auth::cleanup_expired(&conn);

    let code = req.invite_code.trim();
    let invite = db::find_invite_by_code(&conn, code)?;
    let valid = invite
        .as_ref()
        .map(|item| item.is_active(Utc::now()))
        .unwrap_or(false);
    if !valid {
        return Err(AppError::forbidden("邀请码无效、已过期或已用尽"));
    }

    if let Some(existing) = auth::find_user_by_key(&conn, req.public_key.trim()) {
        return Err(AppError::conflict(
            "该密钥已注册，请直接登录",
            json!({ "user": existing }),
        ));
    }

    let user = User {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_string(),
        public_key: req.public_key.trim().to_string(),
        created_at: Utc::now(),
    };
    conn.execute(
        "INSERT INTO users(id, name, public_key, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            user.id,
            user.name,
            user.public_key,
            user.created_at.to_rfc3339()
        ],
    )?;
    db::increment_invite_use(&conn, code)?;

    let token = auth::create_session(&conn, &user.id)?;
    let workspace_name =
        db::get_meta(&conn, "workspace_name")?.unwrap_or_else(|| "totion 共享空间".to_string());

    Ok(Json(AuthResponse {
        token,
        user,
        workspace_name,
    }))
}

async fn challenge(
    State(db): State<Db>,
    Json(req): Json<ChallengeRequest>,
) -> Result<Json<ChallengeResponse>, AppError> {
    let conn = db.lock().unwrap();
    auth::cleanup_expired(&conn);

    if auth::find_user_by_key(&conn, req.public_key.trim()).is_none() {
        return Err(AppError::not_found(
            "该密钥尚未加入空间，请先使用邀请码注册",
        ));
    }

    let challenge_id = uuid::Uuid::new_v4().to_string();
    let nonce = auth::random_hex(32);
    let expires_at = (Utc::now() + chrono::Duration::seconds(auth::CHALLENGE_SECS)).to_rfc3339();
    conn.execute(
        "INSERT INTO challenges(id, public_key, nonce, expires_at) VALUES (?1, ?2, ?3, ?4)",
        params![challenge_id, req.public_key.trim(), nonce, expires_at],
    )?;

    Ok(Json(ChallengeResponse {
        challenge_id,
        nonce,
    }))
}

async fn verify(
    State(db): State<Db>,
    Json(req): Json<VerifyRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let conn = db.lock().unwrap();
    auth::cleanup_expired(&conn);

    let found: Option<(String, String)> = conn
        .query_row(
            "SELECT public_key, nonce FROM challenges WHERE id = ?1 AND expires_at > ?2",
            params![req.challenge_id, db::now_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    let (public_key, nonce) = found.ok_or_else(|| AppError::bad_request("挑战不存在或已过期"))?;

    if !auth::verify_signature(&public_key, nonce.as_bytes(), &req.signature) {
        return Err(AppError::unauthorized("签名校验失败"));
    }
    conn.execute(
        "DELETE FROM challenges WHERE id = ?1",
        params![req.challenge_id],
    )?;

    let user = auth::find_user_by_key(&conn, &public_key)
        .ok_or_else(|| AppError::not_found("用户不存在"))?;
    let token = auth::create_session(&conn, &user.id)?;
    let workspace_name =
        db::get_meta(&conn, "workspace_name")?.unwrap_or_else(|| "totion 共享空间".to_string());

    Ok(Json(AuthResponse {
        token,
        user,
        workspace_name,
    }))
}

async fn snapshot(State(db): State<Db>) -> Result<Json<Snapshot>, AppError> {
    let conn = db.lock().unwrap();
    let pages = db::list_pages(&conn)?;
    let blocks = db::list_blocks(&conn)?;
    let users = db::list_users(&conn)?;
    let habits = db::list_habits(&conn)?;
    let checkins = db::list_checkins(&conn)?;
    let comments = db::list_comments(&conn)?;
    let journals = db::list_journals(&conn)?;
    let last_seq = db::max_seq(&conn)?;
    let workspace_name =
        db::get_meta(&conn, "workspace_name")?.unwrap_or_else(|| "totion 共享空间".to_string());

    Ok(Json(Snapshot {
        workspace: WorkspaceInfo {
            name: workspace_name,
            users,
        },
        pages,
        blocks,
        habits,
        checkins,
        comments,
        journals,
        last_seq,
        api_version: API_VERSION,
    }))
}

#[derive(Debug, Deserialize)]
struct SyncQuery {
    since: Option<i64>,
    limit: Option<u32>,
}

async fn sync(
    State(db): State<Db>,
    headers: HeaderMap,
    Query(query): Query<SyncQuery>,
) -> Result<Json<SyncResponse>, AppError> {
    let conn = db.lock().unwrap();
    let since = query.since.unwrap_or(0);
    let limit = query.limit.unwrap_or(500).clamp(1, 1000) as i64;
    let protocol: u32 = headers
        .get("x-totion-protocol")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    let v1_only = protocol < 2;

    let (sql, next_sql) = if v1_only {
        (
            "SELECT seq, entity, data, actor, at FROM changes
             WHERE seq > ?1 AND entity IN ('page','block') ORDER BY seq LIMIT ?2",
            "SELECT 1 FROM changes
             WHERE seq > ?1 AND entity IN ('page','block') LIMIT 1",
        )
    } else {
        (
            "SELECT seq, entity, data, actor, at FROM changes
             WHERE seq > ?1 ORDER BY seq LIMIT ?2",
            "SELECT 1 FROM changes WHERE seq > ?1 LIMIT 1",
        )
    };

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![since, limit], |row| {
        let seq: i64 = row.get(0)?;
        let entity: String = row.get(1)?;
        let data: String = row.get(2)?;
        let actor: String = row.get(3)?;
        let at: String = row.get(4)?;
        Ok((seq, entity, data, actor, at))
    })?;

    let mut changes = Vec::new();
    for row in rows {
        let (seq, entity, data, actor, at) = row?;
        let Some(kind) = EntityKind::from_wire(&entity) else {
            tracing::warn!("sync 跳过未知实体: {entity}");
            continue;
        };
        if v1_only && !kind.is_v1() {
            continue;
        }
        changes.push(Change {
            seq,
            entity: kind,
            data: serde_json::from_str(&data)?,
            actor,
            at: db::parse_ts(&at),
        });
    }

    let next_seq = changes.last().map(|change| change.seq).unwrap_or(since);
    let has_more = conn
        .query_row(next_sql, params![next_seq], |row| row.get::<_, i64>(0))
        .optional()?
        .is_some();

    Ok(Json(SyncResponse {
        changes,
        last_seq: next_seq,
        has_more,
    }))
}

async fn create_page(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Json(req): Json<CreatePageRequest>,
) -> Result<Json<WritePageResponse>, AppError> {
    if let Some(date) = req.date.as_deref() {
        if !valid_date(date) {
            return Err(AppError::bad_request("date 必须是 YYYY-MM-DD"));
        }
    }
    let conn = db.lock().unwrap();
    let now = Utc::now();
    let page = Page {
        id: uuid::Uuid::new_v4().to_string(),
        title: req.title,
        icon: req.icon,
        kind: req.kind,
        position: req.position,
        version: 1,
        updated_at: now,
        updated_by: user.name.clone(),
        deleted: false,
        date: req.date,
    };
    let seq = db::insert_page_and_change(&conn, &page, &user.name)?;

    Ok(Json(WritePageResponse { page, seq }))
}

async fn update_page(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
    Json(req): Json<UpdatePageRequest>,
) -> Result<Json<WritePageResponse>, AppError> {
    let conn = db.lock().unwrap();
    let mut page = db::find_page(&conn, &id)?.ok_or_else(|| AppError::not_found("页面不存在"))?;

    if page.version != req.expected_version {
        return Err(AppError::conflict(
            "该页面已被另一方修改，请刷新后重试",
            serde_json::to_value(&page)?,
        ));
    }

    if let Some(title) = req.title {
        page.title = title;
    }
    if let Some(icon) = req.icon {
        page.icon = icon;
    }
    if let Some(kind) = req.kind {
        page.kind = kind;
    }
    if let Some(position) = req.position {
        page.position = position;
    }
    if let Some(deleted) = req.deleted {
        page.deleted = deleted;
    }
    if let Some(date) = req.date {
        if !date.is_empty() && !valid_date(&date) {
            return Err(AppError::bad_request("date 必须是 YYYY-MM-DD"));
        }
        page.date = if date.is_empty() { None } else { Some(date) };
    }
    page.version += 1;
    page.updated_at = Utc::now();
    page.updated_by = user.name.clone();

    let seq = db::update_page_and_change(&conn, &page, &user.name)?;

    Ok(Json(WritePageResponse { page, seq }))
}

fn validate_times(start: Option<&str>, end: Option<&str>) -> Result<(), AppError> {
    if let Some(value) = start.filter(|value| !value.is_empty()) {
        if !valid_time(value) {
            return Err(AppError::bad_request("开始时间格式应为 HH:MM"));
        }
    }
    if let Some(value) = end.filter(|value| !value.is_empty()) {
        if !valid_time(value) {
            return Err(AppError::bad_request("结束时间格式应为 HH:MM"));
        }
    }
    Ok(())
}

async fn create_block(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Json(req): Json<CreateBlockRequest>,
) -> Result<Json<WriteBlockResponse>, AppError> {
    validate_times(req.start_time.as_deref(), req.end_time.as_deref())?;
    let conn = db.lock().unwrap();
    if db::find_page(&conn, &req.page_id)?.is_none() {
        return Err(AppError::not_found("目标页面不存在"));
    }
    let now = Utc::now();
    let block = Block {
        id: uuid::Uuid::new_v4().to_string(),
        page_id: req.page_id,
        kind: req.kind,
        text: req.text,
        checked: false,
        due_date: req.due_date,
        streak: 0,
        position: req.position,
        version: 1,
        updated_at: now,
        updated_by: user.name.clone(),
        deleted: false,
        start_time: req.start_time.filter(|value| !value.is_empty()),
        end_time: req.end_time.filter(|value| !value.is_empty()),
        completed_at: None,
        completed_by: None,
        section: req.section,
    };
    let seq = db::insert_block_and_change(&conn, &block, &user.name)?;

    Ok(Json(WriteBlockResponse { block, seq }))
}

async fn update_block(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
    Json(req): Json<UpdateBlockRequest>,
) -> Result<Json<WriteBlockResponse>, AppError> {
    validate_times(req.start_time.as_deref(), req.end_time.as_deref())?;
    let conn = db.lock().unwrap();
    let mut block =
        db::find_block(&conn, &id)?.ok_or_else(|| AppError::not_found("内容块不存在"))?;

    if block.version != req.expected_version {
        return Err(AppError::conflict(
            "该内容块已被另一方修改，请刷新后重试",
            serde_json::to_value(&block)?,
        ));
    }

    let was_checked = block.checked;
    if let Some(text) = req.text {
        block.text = text;
    }
    if let Some(checked) = req.checked {
        block.checked = checked;
    }
    if let Some(due_date) = req.due_date {
        block.due_date = if due_date.is_empty() {
            None
        } else {
            Some(due_date)
        };
    }
    if let Some(streak) = req.streak {
        block.streak = streak;
    }
    if let Some(position) = req.position {
        block.position = position;
    }
    if let Some(deleted) = req.deleted {
        block.deleted = deleted;
    }
    if let Some(start_time) = req.start_time {
        block.start_time = if start_time.is_empty() {
            None
        } else {
            Some(start_time)
        };
    }
    if let Some(end_time) = req.end_time {
        block.end_time = if end_time.is_empty() {
            None
        } else {
            Some(end_time)
        };
    }

    let now = Utc::now();
    if !was_checked && block.checked {
        block.completed_at = Some(now);
        block.completed_by = Some(user.name.clone());
    } else if was_checked && !block.checked {
        block.completed_at = None;
        block.completed_by = None;
    }
    block.version += 1;
    block.updated_at = now;
    block.updated_by = user.name.clone();

    let seq = db::update_block_and_change(&conn, &block, &user.name)?;

    Ok(Json(WriteBlockResponse { block, seq }))
}

async fn create_habit(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Json(req): Json<CreateHabitRequest>,
) -> Result<Json<WriteHabitResponse>, AppError> {
    if req.title.trim().is_empty() {
        return Err(AppError::bad_request("习惯名称不能为空"));
    }
    let conn = db.lock().unwrap();
    let now = Utc::now();
    let habit = Habit {
        id: uuid::Uuid::new_v4().to_string(),
        title: req.title.trim().to_string(),
        icon: if req.icon.is_empty() {
            "🌱".to_string()
        } else {
            req.icon
        },
        note: req.note,
        frequency: req.frequency,
        position: req.position,
        archived: false,
        version: 1,
        created_at: now,
        updated_at: now,
        updated_by: user.name.clone(),
        deleted: false,
    };
    let seq = db::insert_habit_and_change(&conn, &habit, &user.name)?;
    Ok(Json(WriteHabitResponse { habit, seq }))
}

async fn update_habit(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
    Json(req): Json<UpdateHabitRequest>,
) -> Result<Json<WriteHabitResponse>, AppError> {
    let conn = db.lock().unwrap();
    let mut habit = db::find_habit(&conn, &id)?.ok_or_else(|| AppError::not_found("习惯不存在"))?;

    if habit.version != req.expected_version {
        return Err(AppError::conflict(
            "该习惯已被另一方修改，请刷新后重试",
            serde_json::to_value(&habit)?,
        ));
    }

    if let Some(title) = req.title {
        if title.trim().is_empty() {
            return Err(AppError::bad_request("习惯名称不能为空"));
        }
        habit.title = title.trim().to_string();
    }
    if let Some(icon) = req.icon {
        habit.icon = icon;
    }
    if let Some(note) = req.note {
        habit.note = note;
    }
    if let Some(frequency) = req.frequency {
        habit.frequency = frequency;
    }
    if let Some(position) = req.position {
        habit.position = position;
    }
    if let Some(archived) = req.archived {
        habit.archived = archived;
    }
    if let Some(deleted) = req.deleted {
        habit.deleted = deleted;
    }
    habit.version += 1;
    habit.updated_at = Utc::now();
    habit.updated_by = user.name.clone();

    let seq = db::update_habit_and_change(&conn, &habit, &user.name)?;
    Ok(Json(WriteHabitResponse { habit, seq }))
}

async fn check_in_habit(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
    Json(req): Json<CheckinRequest>,
) -> Result<Json<WriteCheckinResponse>, AppError> {
    if !valid_date(&req.date) {
        return Err(AppError::bad_request("date 必须是 YYYY-MM-DD"));
    }
    let conn = db.lock().unwrap();
    let habit = db::find_habit(&conn, &id)?
        .filter(|habit| !habit.deleted)
        .ok_or_else(|| AppError::not_found("习惯不存在"))?;

    if let Some(existing) = db::find_checkin_by_date(&conn, &habit.id, &req.date)? {
        return Ok(Json(WriteCheckinResponse {
            checkin: existing,
            seq: None,
            created: false,
        }));
    }

    let now = Utc::now();
    let checkin = HabitCheckin {
        id: uuid::Uuid::new_v4().to_string(),
        habit_id: habit.id,
        date: req.date,
        note: req.note.unwrap_or_default(),
        version: 1,
        created_at: now,
        updated_at: now,
        updated_by: user.name.clone(),
        deleted: false,
    };
    match db::insert_checkin_and_change(&conn, &checkin, &user.name) {
        Ok(seq) => Ok(Json(WriteCheckinResponse {
            checkin,
            seq: Some(seq),
            created: true,
        })),
        Err(err) => {
            // 并发/重复打卡：唯一索引兜底，返回已有记录。
            if let Some(existing) =
                db::find_checkin_by_date(&conn, &checkin.habit_id, &checkin.date)?
            {
                Ok(Json(WriteCheckinResponse {
                    checkin: existing,
                    seq: None,
                    created: false,
                }))
            } else {
                Err(AppError::internal(err.to_string()))
            }
        }
    }
}

async fn update_checkin(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
    Json(req): Json<UpdateCheckinRequest>,
) -> Result<Json<WriteCheckinResponse>, AppError> {
    let conn = db.lock().unwrap();
    let mut checkin =
        db::find_checkin(&conn, &id)?.ok_or_else(|| AppError::not_found("打卡记录不存在"))?;

    if checkin.version != req.expected_version {
        return Err(AppError::conflict(
            "该打卡记录已被另一方修改，请刷新后重试",
            serde_json::to_value(&checkin)?,
        ));
    }

    if let Some(note) = req.note {
        checkin.note = note;
    }
    if let Some(deleted) = req.deleted {
        checkin.deleted = deleted;
    }
    checkin.version += 1;
    checkin.updated_at = Utc::now();
    checkin.updated_by = user.name.clone();

    let seq = db::update_checkin_and_change(&conn, &checkin, &user.name)?;
    Ok(Json(WriteCheckinResponse {
        checkin,
        seq: Some(seq),
        created: false,
    }))
}

fn target_exists(conn: &Connection, kind: CommentTargetKind, id: &str) -> rusqlite::Result<bool> {
    match kind {
        CommentTargetKind::Page => Ok(db::find_page(conn, id)?
            .map(|page| !page.deleted)
            .unwrap_or(false)),
        CommentTargetKind::Block => Ok(db::find_block(conn, id)?
            .map(|block| !block.deleted)
            .unwrap_or(false)),
        CommentTargetKind::Habit => Ok(db::find_habit(conn, id)?
            .map(|habit| !habit.deleted)
            .unwrap_or(false)),
    }
}

async fn create_comment(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Json(req): Json<CreateCommentRequest>,
) -> Result<Json<WriteCommentResponse>, AppError> {
    let body = req.body.trim();
    if body.is_empty() {
        return Err(AppError::bad_request("评论内容不能为空"));
    }
    if body.chars().count() > 4000 {
        return Err(AppError::bad_request("评论不能超过 4000 字"));
    }
    let conn = db.lock().unwrap();
    if !target_exists(&conn, req.target_kind, &req.target_id)? {
        return Err(AppError::not_found("评论目标不存在"));
    }

    let now = Utc::now();
    let comment = Comment {
        id: uuid::Uuid::new_v4().to_string(),
        target_kind: req.target_kind,
        target_id: req.target_id,
        body: body.to_string(),
        resolved: false,
        author_id: user.id.clone(),
        author_name: user.name.clone(),
        version: 1,
        created_at: now,
        updated_at: now,
        deleted: false,
    };
    let seq = db::insert_comment_and_change(&conn, &comment, &user.name)?;
    Ok(Json(WriteCommentResponse { comment, seq }))
}

async fn update_comment(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
    Json(req): Json<UpdateCommentRequest>,
) -> Result<Json<WriteCommentResponse>, AppError> {
    let conn = db.lock().unwrap();
    let mut comment =
        db::find_comment(&conn, &id)?.ok_or_else(|| AppError::not_found("评论不存在"))?;

    if comment.version != req.expected_version {
        return Err(AppError::conflict(
            "该评论已被另一方修改，请刷新后重试",
            serde_json::to_value(&comment)?,
        ));
    }

    if let Some(body) = req.body {
        let body = body.trim();
        if body.is_empty() {
            return Err(AppError::bad_request("评论内容不能为空"));
        }
        comment.body = body.to_string();
    }
    if let Some(resolved) = req.resolved {
        comment.resolved = resolved;
    }
    if let Some(deleted) = req.deleted {
        comment.deleted = deleted;
    }
    comment.version += 1;
    comment.updated_at = Utc::now();

    let seq = db::update_comment_and_change(&conn, &comment, &user.name)?;
    Ok(Json(WriteCommentResponse { comment, seq }))
}

fn journal_target_exists(
    conn: &Connection,
    kind: JournalTarget,
    id: &str,
) -> rusqlite::Result<bool> {
    match kind {
        JournalTarget::Daily => Ok(true),
        JournalTarget::Block => Ok(db::find_block(conn, id)?
            .map(|block| !block.deleted)
            .unwrap_or(false)),
        JournalTarget::Habit => Ok(db::find_habit(conn, id)?
            .map(|habit| !habit.deleted)
            .unwrap_or(false)),
    }
}

async fn list_invites(State(db): State<Db>) -> Result<Json<InviteListResponse>, AppError> {
    let conn = db.lock().unwrap();
    Ok(Json(InviteListResponse {
        invites: db::list_invites(&conn)?,
    }))
}

async fn create_invite(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Json(req): Json<CreateInviteRequest>,
) -> Result<Json<WriteInviteResponse>, AppError> {
    if req.max_uses == Some(0) {
        return Err(AppError::bad_request("可用次数必须大于 0"));
    }
    let now = Utc::now();
    let note = if req.note.trim().is_empty() {
        format!("由 {} 创建", user.name)
    } else {
        req.note.trim().to_string()
    };
    let expires_at = req
        .expires_in_days
        .map(|days| now + chrono::Duration::days(days as i64));

    let conn = db.lock().unwrap();
    for _ in 0..8 {
        let candidate = Invite {
            id: uuid::Uuid::new_v4().to_string(),
            code: auth::generate_invite_code(),
            note: note.clone(),
            created_by: user.name.clone(),
            created_at: now,
            expires_at,
            max_uses: req.max_uses,
            used_count: 0,
            revoked: false,
        };
        match db::insert_invite(&conn, &candidate) {
            Ok(()) => return Ok(Json(WriteInviteResponse { invite: candidate })),
            Err(err) if err.to_string().contains("UNIQUE") => continue,
            Err(err) => return Err(AppError::from(err)),
        }
    }
    Err(AppError::internal("生成邀请码失败，请重试"))
}

async fn update_invite(
    State(db): State<Db>,
    Path(id): Path<String>,
    Json(req): Json<UpdateInviteRequest>,
) -> Result<Json<WriteInviteResponse>, AppError> {
    let conn = db.lock().unwrap();
    let mut invite =
        db::find_invite(&conn, &id)?.ok_or_else(|| AppError::not_found("邀请码不存在"))?;
    if let Some(revoked) = req.revoked {
        invite.revoked = revoked;
    }
    db::update_invite(&conn, &invite)?;
    Ok(Json(WriteInviteResponse { invite }))
}

async fn create_journal(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Json(req): Json<CreateJournalRequest>,
) -> Result<Json<WriteJournalResponse>, AppError> {
    if !valid_date(&req.date) {
        return Err(AppError::bad_request("date 必须是 YYYY-MM-DD"));
    }
    if !valid_time(&req.time) {
        return Err(AppError::bad_request("time 必须是 HH:MM"));
    }
    let body = req.body.trim();
    if body.is_empty() {
        return Err(AppError::bad_request("总结内容不能为空"));
    }
    if body.chars().count() > 8000 {
        return Err(AppError::bad_request("总结不能超过 8000 字"));
    }

    let target_id = if req.target_kind == JournalTarget::Daily && req.target_id.is_empty() {
        req.date.clone()
    } else {
        req.target_id.clone()
    };
    if req.target_kind == JournalTarget::Daily && !valid_date(&target_id) {
        return Err(AppError::bad_request("当日总结的 target_id 必须是日期"));
    }

    let conn = db.lock().unwrap();
    if !journal_target_exists(&conn, req.target_kind, &target_id)? {
        return Err(AppError::not_found("总结目标不存在"));
    }

    let now = Utc::now();
    let journal = JournalEntry {
        id: uuid::Uuid::new_v4().to_string(),
        target_kind: req.target_kind,
        target_id,
        date: req.date,
        time: req.time,
        body: body.to_string(),
        author_id: user.id.clone(),
        author_name: user.name.clone(),
        version: 1,
        created_at: now,
        updated_at: now,
        deleted: false,
    };
    let seq = db::insert_journal_and_change(&conn, &journal, &user.name)?;
    Ok(Json(WriteJournalResponse { journal, seq }))
}

async fn update_journal(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
    Json(req): Json<UpdateJournalRequest>,
) -> Result<Json<WriteJournalResponse>, AppError> {
    let conn = db.lock().unwrap();
    let mut journal =
        db::find_journal(&conn, &id)?.ok_or_else(|| AppError::not_found("总结不存在"))?;

    if journal.version != req.expected_version {
        return Err(AppError::conflict(
            "该总结已被其他人修改，请刷新后重试",
            serde_json::to_value(&journal)?,
        ));
    }

    if let Some(body) = req.body {
        let body = body.trim();
        if body.is_empty() {
            return Err(AppError::bad_request("总结内容不能为空"));
        }
        journal.body = body.to_string();
    }
    if let Some(deleted) = req.deleted {
        journal.deleted = deleted;
    }
    journal.version += 1;
    journal.updated_at = Utc::now();

    let seq = db::update_journal_and_change(&conn, &journal, &user.name)?;
    Ok(Json(WriteJournalResponse { journal, seq }))
}

async fn ensure_day(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Path(date): Path<String>,
) -> Result<Json<EnsureDayResponse>, AppError> {
    if !valid_date(&date) {
        return Err(AppError::bad_request("date 必须是 YYYY-MM-DD"));
    }
    let conn = db.lock().unwrap();
    let now = Utc::now();
    let tx = conn.unchecked_transaction()?;

    let mut created = false;
    let page = match db::find_page_by_date(&tx, &date)? {
        Some(mut page) => {
            if page.deleted {
                page.deleted = false;
                page.version += 1;
                page.updated_at = now;
                page.updated_by = user.name.clone();
                db::update_page(&tx, &page)?;
                let data = serde_json::to_value(&page)?;
                db::push_change(&tx, EntityKind::Page.as_wire(), &data, &user.name, now)?;
            }
            page
        }
        None => {
            created = true;
            let page = Page {
                id: uuid::Uuid::new_v4().to_string(),
                title: format!("{date} 总结"),
                icon: "📔".to_string(),
                kind: PageKind::Doc,
                position: 0,
                version: 1,
                updated_at: now,
                updated_by: user.name.clone(),
                deleted: false,
                date: Some(date.clone()),
            };
            db::insert_page(&tx, &page)?;
            let data = serde_json::to_value(&page)?;
            db::push_change(&tx, EntityKind::Page.as_wire(), &data, &user.name, now)?;
            page
        }
    };

    let mut blocks = Vec::new();
    for (index, section) in BlockSection::ALL.iter().enumerate() {
        let block = match db::find_section_block(&tx, &page.id, *section)? {
            Some(mut block) => {
                if block.deleted {
                    block.deleted = false;
                    block.version += 1;
                    block.updated_at = now;
                    block.updated_by = user.name.clone();
                    db::update_block(&tx, &block)?;
                    let data = serde_json::to_value(&block)?;
                    db::push_change(&tx, EntityKind::Block.as_wire(), &data, &user.name, now)?;
                }
                block
            }
            None => {
                let block = Block {
                    id: uuid::Uuid::new_v4().to_string(),
                    page_id: page.id.clone(),
                    kind: BlockKind::Text,
                    text: String::new(),
                    checked: false,
                    due_date: None,
                    streak: 0,
                    position: index as i64,
                    version: 1,
                    updated_at: now,
                    updated_by: user.name.clone(),
                    deleted: false,
                    start_time: None,
                    end_time: None,
                    completed_at: None,
                    completed_by: None,
                    section: Some(*section),
                };
                db::insert_block(&tx, &block)?;
                let data = serde_json::to_value(&block)?;
                db::push_change(&tx, EntityKind::Block.as_wire(), &data, &user.name, now)?;
                block
            }
        };
        blocks.push(block);
    }

    let last_seq = db::max_seq(&tx)?;
    tx.commit()?;

    Ok(Json(EnsureDayResponse {
        page,
        blocks,
        last_seq,
        created,
    }))
}
