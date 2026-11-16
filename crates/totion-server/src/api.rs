use std::sync::{Arc, Mutex};

use axum::{
    extract::{Extension, Path, Query, Request, State},
    http::StatusCode,
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
    ApiError, AuthResponse, Block, ChallengeRequest, ChallengeResponse, Change, CreateBlockRequest,
    CreatePageRequest, EntityKind, Page, RegisterRequest, Snapshot, SyncResponse,
    UpdateBlockRequest, UpdatePageRequest, User, VerifyRequest, WorkspaceInfo, WriteBlockResponse,
    WritePageResponse,
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
    Json(json!({ "status": "ok", "service": "totion-server" }))
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

    let invite = db::get_meta(&conn, "invite_code")?.unwrap_or_default();
    if req.invite_code.trim() != invite {
        return Err(AppError::forbidden("邀请码不正确"));
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
        last_seq,
    }))
}

#[derive(Debug, Deserialize)]
struct SyncQuery {
    since: Option<i64>,
}

async fn sync(
    State(db): State<Db>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<SyncResponse>, AppError> {
    let conn = db.lock().unwrap();
    let since = query.since.unwrap_or(0);

    let mut stmt = conn.prepare(
        "SELECT seq, entity, data, actor, at FROM changes
         WHERE seq > ?1 ORDER BY seq LIMIT 1000",
    )?;
    let rows = stmt.query_map(params![since], |row| {
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
        changes.push(Change {
            seq,
            entity: if entity == "page" {
                EntityKind::Page
            } else {
                EntityKind::Block
            },
            data: serde_json::from_str(&data)?,
            actor,
            at: db::parse_ts(&at),
        });
    }

    let last_seq = db::max_seq(&conn)?;
    Ok(Json(SyncResponse { changes, last_seq }))
}

async fn create_page(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Json(req): Json<CreatePageRequest>,
) -> Result<Json<WritePageResponse>, AppError> {
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
    };
    db::insert_page(&conn, &page)?;
    let seq = db::push_change(
        &conn,
        "page",
        &serde_json::to_value(&page)?,
        &user.name,
        now,
    )?;

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
    page.version += 1;
    page.updated_at = Utc::now();
    page.updated_by = user.name.clone();

    db::update_page(&conn, &page)?;
    let seq = db::push_change(
        &conn,
        "page",
        &serde_json::to_value(&page)?,
        &user.name,
        page.updated_at,
    )?;

    Ok(Json(WritePageResponse { page, seq }))
}

async fn create_block(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Json(req): Json<CreateBlockRequest>,
) -> Result<Json<WriteBlockResponse>, AppError> {
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
    };
    db::insert_block(&conn, &block)?;
    let seq = db::push_change(
        &conn,
        "block",
        &serde_json::to_value(&block)?,
        &user.name,
        now,
    )?;

    Ok(Json(WriteBlockResponse { block, seq }))
}

async fn update_block(
    State(db): State<Db>,
    Extension(user): Extension<User>,
    Path(id): Path<String>,
    Json(req): Json<UpdateBlockRequest>,
) -> Result<Json<WriteBlockResponse>, AppError> {
    let conn = db.lock().unwrap();
    let mut block =
        db::find_block(&conn, &id)?.ok_or_else(|| AppError::not_found("内容块不存在"))?;

    if block.version != req.expected_version {
        return Err(AppError::conflict(
            "该内容块已被另一方修改，请刷新后重试",
            serde_json::to_value(&block)?,
        ));
    }

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
    block.version += 1;
    block.updated_at = Utc::now();
    block.updated_by = user.name.clone();

    db::update_block(&conn, &block)?;
    let seq = db::push_change(
        &conn,
        "block",
        &serde_json::to_value(&block)?,
        &user.name,
        block.updated_at,
    )?;

    Ok(Json(WriteBlockResponse { block, seq }))
}
