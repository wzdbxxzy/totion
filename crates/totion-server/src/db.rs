use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use totion_core::{Block, BlockKind, Page, PageKind, User};

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS users (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  public_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS sessions (
  token TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  expires_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS challenges (
  id TEXT PRIMARY KEY,
  public_key TEXT NOT NULL,
  nonce TEXT NOT NULL,
  expires_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS pages (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  icon TEXT NOT NULL,
  kind TEXT NOT NULL,
  position INTEGER NOT NULL,
  version INTEGER NOT NULL,
  updated_at TEXT NOT NULL,
  updated_by TEXT NOT NULL,
  deleted INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS blocks (
  id TEXT PRIMARY KEY,
  page_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  text TEXT NOT NULL,
  checked INTEGER NOT NULL DEFAULT 0,
  due_date TEXT,
  streak INTEGER NOT NULL DEFAULT 0,
  position INTEGER NOT NULL,
  version INTEGER NOT NULL,
  updated_at TEXT NOT NULL,
  updated_by TEXT NOT NULL,
  deleted INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS changes (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  entity TEXT NOT NULL,
  data TEXT NOT NULL,
  actor TEXT NOT NULL,
  at TEXT NOT NULL
);
"#;

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path).with_context(|| format!("打开数据库 {}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

pub fn get_meta(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO meta(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn new_token() -> String {
    use rand_core::{OsRng, RngCore};
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn now_str() -> String {
    Utc::now().to_rfc3339()
}

pub fn parse_ts(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

pub fn kind_to_str(kind: PageKind) -> &'static str {
    match kind {
        PageKind::Doc => "doc",
        PageKind::Plan => "plan",
        PageKind::Habit => "habit",
        PageKind::Todo => "todo",
        PageKind::Calendar => "calendar",
    }
}

pub fn kind_from_str(value: &str) -> PageKind {
    match value {
        "plan" => PageKind::Plan,
        "habit" => PageKind::Habit,
        "todo" => PageKind::Todo,
        "calendar" => PageKind::Calendar,
        _ => PageKind::Doc,
    }
}

pub fn block_kind_to_str(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Text => "text",
        BlockKind::Heading => "heading",
        BlockKind::Todo => "todo",
        BlockKind::Habit => "habit",
        BlockKind::CalendarEvent => "calendar_event",
        BlockKind::Divider => "divider",
    }
}

pub fn block_kind_from_str(value: &str) -> BlockKind {
    match value {
        "heading" => BlockKind::Heading,
        "todo" => BlockKind::Todo,
        "habit" => BlockKind::Habit,
        "calendar_event" => BlockKind::CalendarEvent,
        "divider" => BlockKind::Divider,
        _ => BlockKind::Text,
    }
}

pub fn row_to_user(row: &Row) -> rusqlite::Result<User> {
    Ok(User {
        id: row.get(0)?,
        name: row.get(1)?,
        public_key: row.get(2)?,
        created_at: parse_ts(&row.get::<_, String>(3)?),
    })
}

pub fn row_to_page(row: &Row) -> rusqlite::Result<Page> {
    Ok(Page {
        id: row.get(0)?,
        title: row.get(1)?,
        icon: row.get(2)?,
        kind: kind_from_str(&row.get::<_, String>(3)?),
        position: row.get(4)?,
        version: row.get(5)?,
        updated_at: parse_ts(&row.get::<_, String>(6)?),
        updated_by: row.get(7)?,
        deleted: row.get::<_, i64>(8)? != 0,
    })
}

pub fn row_to_block(row: &Row) -> rusqlite::Result<Block> {
    Ok(Block {
        id: row.get(0)?,
        page_id: row.get(1)?,
        kind: block_kind_from_str(&row.get::<_, String>(2)?),
        text: row.get(3)?,
        checked: row.get::<_, i64>(4)? != 0,
        due_date: row.get(5)?,
        streak: row.get(6)?,
        position: row.get(7)?,
        version: row.get(8)?,
        updated_at: parse_ts(&row.get::<_, String>(9)?),
        updated_by: row.get(10)?,
        deleted: row.get::<_, i64>(11)? != 0,
    })
}

pub fn list_pages(conn: &Connection) -> rusqlite::Result<Vec<Page>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, icon, kind, position, version, updated_at, updated_by, deleted
         FROM pages ORDER BY position, updated_at",
    )?;
    let rows = stmt.query_map([], row_to_page)?;
    rows.collect()
}

pub fn list_blocks(conn: &Connection) -> rusqlite::Result<Vec<Block>> {
    let mut stmt = conn.prepare(
        "SELECT id, page_id, kind, text, checked, due_date, streak, position, version,
                updated_at, updated_by, deleted
         FROM blocks ORDER BY page_id, position, updated_at",
    )?;
    let rows = stmt.query_map([], row_to_block)?;
    rows.collect()
}

pub fn list_users(conn: &Connection) -> rusqlite::Result<Vec<User>> {
    let mut stmt =
        conn.prepare("SELECT id, name, public_key, created_at FROM users ORDER BY created_at")?;
    let rows = stmt.query_map([], row_to_user)?;
    rows.collect()
}

pub fn find_page(conn: &Connection, id: &str) -> rusqlite::Result<Option<Page>> {
    conn.query_row(
        "SELECT id, title, icon, kind, position, version, updated_at, updated_by, deleted
         FROM pages WHERE id = ?1",
        params![id],
        row_to_page,
    )
    .optional()
}

pub fn find_block(conn: &Connection, id: &str) -> rusqlite::Result<Option<Block>> {
    conn.query_row(
        "SELECT id, page_id, kind, text, checked, due_date, streak, position, version,
                updated_at, updated_by, deleted
         FROM blocks WHERE id = ?1",
        params![id],
        row_to_block,
    )
    .optional()
}

pub fn insert_page(conn: &Connection, page: &Page) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO pages(id, title, icon, kind, position, version, updated_at, updated_by, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            page.id,
            page.title,
            page.icon,
            kind_to_str(page.kind),
            page.position,
            page.version,
            page.updated_at.to_rfc3339(),
            page.updated_by,
            page.deleted as i64,
        ],
    )?;
    Ok(())
}

pub fn insert_block(conn: &Connection, block: &Block) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO blocks(id, page_id, kind, text, checked, due_date, streak, position,
                            version, updated_at, updated_by, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            block.id,
            block.page_id,
            block_kind_to_str(block.kind),
            block.text,
            block.checked as i64,
            block.due_date,
            block.streak,
            block.position,
            block.version,
            block.updated_at.to_rfc3339(),
            block.updated_by,
            block.deleted as i64,
        ],
    )?;
    Ok(())
}

pub fn update_page(conn: &Connection, page: &Page) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE pages SET title = ?2, icon = ?3, kind = ?4, position = ?5, version = ?6,
                          updated_at = ?7, updated_by = ?8, deleted = ?9
         WHERE id = ?1",
        params![
            page.id,
            page.title,
            page.icon,
            kind_to_str(page.kind),
            page.position,
            page.version,
            page.updated_at.to_rfc3339(),
            page.updated_by,
            page.deleted as i64,
        ],
    )?;
    Ok(())
}

pub fn update_block(conn: &Connection, block: &Block) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE blocks SET text = ?2, checked = ?3, due_date = ?4, streak = ?5, position = ?6,
                           version = ?7, updated_at = ?8, updated_by = ?9, deleted = ?10
         WHERE id = ?1",
        params![
            block.id,
            block.text,
            block.checked as i64,
            block.due_date,
            block.streak,
            block.position,
            block.version,
            block.updated_at.to_rfc3339(),
            block.updated_by,
            block.deleted as i64,
        ],
    )?;
    Ok(())
}

/// 追加一条变更日志，返回其 seq。
pub fn push_change(
    conn: &Connection,
    entity: &str,
    data: &serde_json::Value,
    actor: &str,
    at: DateTime<Utc>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO changes(entity, data, actor, at) VALUES (?1, ?2, ?3, ?4)",
        params![entity, data.to_string(), actor, at.to_rfc3339()],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn max_seq(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM changes", [], |row| {
        row.get(0)
    })
}
