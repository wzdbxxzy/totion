use std::path::Path;

use anyhow::Result as AnyResult;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use totion_core::{
    Block, BlockKind, BlockSection, Comment, CommentTargetKind, EntityKind, Habit, HabitCheckin,
    HabitFrequency, JournalEntry, JournalTarget, Page, PageKind, User,
};

pub const SCHEMA_VERSION: i64 = 3;

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

const V2_TABLES: &str = r#"
CREATE UNIQUE INDEX IF NOT EXISTS idx_pages_date ON pages(date);
CREATE INDEX IF NOT EXISTS idx_blocks_page_pos ON blocks(page_id, position);
CREATE INDEX IF NOT EXISTS idx_blocks_due_date ON blocks(due_date);

CREATE TABLE IF NOT EXISTS habits (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  icon TEXT NOT NULL,
  note TEXT NOT NULL DEFAULT '',
  frequency TEXT NOT NULL,
  position INTEGER NOT NULL,
  archived INTEGER NOT NULL DEFAULT 0,
  version INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  updated_by TEXT NOT NULL,
  deleted INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS habit_checkins (
  id TEXT PRIMARY KEY,
  habit_id TEXT NOT NULL,
  date TEXT NOT NULL,
  note TEXT NOT NULL DEFAULT '',
  version INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  updated_by TEXT NOT NULL,
  deleted INTEGER NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_checkin_active
  ON habit_checkins(habit_id, date) WHERE deleted = 0;
CREATE INDEX IF NOT EXISTS idx_checkin_habit_date
  ON habit_checkins(habit_id, date);

CREATE TABLE IF NOT EXISTS comments (
  id TEXT PRIMARY KEY,
  target_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  body TEXT NOT NULL,
  resolved INTEGER NOT NULL DEFAULT 0,
  author_id TEXT NOT NULL,
  author_name TEXT NOT NULL,
  version INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_comments_target
  ON comments(target_kind, target_id, created_at);
"#;

const V3_TABLES: &str = r#"
CREATE TABLE IF NOT EXISTS journals (
  id TEXT PRIMARY KEY,
  target_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  date TEXT NOT NULL,
  time TEXT NOT NULL,
  body TEXT NOT NULL,
  author_id TEXT NOT NULL,
  author_name TEXT NOT NULL,
  version INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_journals_date ON journals(date, created_at);
CREATE INDEX IF NOT EXISTS idx_journals_target ON journals(target_kind, target_id);
"#;

pub fn open(path: &Path) -> AnyResult<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    init(&conn)?;
    Ok(conn)
}

/// 建表 + 版本迁移。可重复调用。
pub fn init(conn: &Connection) -> AnyResult<()> {
    conn.execute_batch(SCHEMA)?;
    migrate(conn)?;
    Ok(())
}

fn migrate(conn: &Connection) -> AnyResult<()> {
    let current: i64 = get_meta(conn, "schema_version")?
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    if current >= SCHEMA_VERSION {
        return Ok(());
    }

    let tx = conn.unchecked_transaction()?;
    if current < 2 {
        apply_v2(&tx)?;
        set_meta(&tx, "schema_version", "2")?;
    }
    if current < 3 {
        apply_v3(&tx)?;
        set_meta(&tx, "schema_version", "3")?;
    }
    tx.commit()?;
    Ok(())
}

fn apply_v2(conn: &Connection) -> rusqlite::Result<()> {
    let additions = [
        ("pages", "date", "ALTER TABLE pages ADD COLUMN date TEXT"),
        (
            "blocks",
            "start_time",
            "ALTER TABLE blocks ADD COLUMN start_time TEXT",
        ),
        (
            "blocks",
            "end_time",
            "ALTER TABLE blocks ADD COLUMN end_time TEXT",
        ),
        (
            "blocks",
            "completed_at",
            "ALTER TABLE blocks ADD COLUMN completed_at TEXT",
        ),
        (
            "blocks",
            "completed_by",
            "ALTER TABLE blocks ADD COLUMN completed_by TEXT",
        ),
        (
            "blocks",
            "section",
            "ALTER TABLE blocks ADD COLUMN section TEXT",
        ),
    ];
    for (table, column, ddl) in additions {
        if !column_exists(conn, table, column)? {
            conn.execute_batch(ddl)?;
        }
    }
    conn.execute_batch(V2_TABLES)?;
    Ok(())
}

fn apply_v3(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(V3_TABLES)?;
    Ok(())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
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

fn parse_opt_ts(value: Option<String>) -> Option<DateTime<Utc>> {
    value.and_then(|raw| {
        DateTime::parse_from_rfc3339(&raw)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    })
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

pub fn section_to_str(section: BlockSection) -> &'static str {
    section.as_wire()
}

fn frequency_to_json(frequency: HabitFrequency) -> String {
    serde_json::to_string(&frequency).unwrap_or_else(|_| r#"{"type":"daily"}"#.to_string())
}

fn frequency_from_json(value: &str) -> HabitFrequency {
    serde_json::from_str(value).unwrap_or_default()
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
        date: row.get(9)?,
    })
}

pub fn row_to_block(row: &Row) -> rusqlite::Result<Block> {
    let section: Option<String> = row.get(16)?;
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
        start_time: row.get(12)?,
        end_time: row.get(13)?,
        completed_at: parse_opt_ts(row.get(14)?),
        completed_by: row.get(15)?,
        section: section.and_then(|value| BlockSection::from_wire(&value)),
    })
}

pub fn row_to_habit(row: &Row) -> rusqlite::Result<Habit> {
    Ok(Habit {
        id: row.get(0)?,
        title: row.get(1)?,
        icon: row.get(2)?,
        note: row.get(3)?,
        frequency: frequency_from_json(&row.get::<_, String>(4)?),
        position: row.get(5)?,
        archived: row.get::<_, i64>(6)? != 0,
        version: row.get(7)?,
        created_at: parse_ts(&row.get::<_, String>(8)?),
        updated_at: parse_ts(&row.get::<_, String>(9)?),
        updated_by: row.get(10)?,
        deleted: row.get::<_, i64>(11)? != 0,
    })
}

pub fn row_to_checkin(row: &Row) -> rusqlite::Result<HabitCheckin> {
    Ok(HabitCheckin {
        id: row.get(0)?,
        habit_id: row.get(1)?,
        date: row.get(2)?,
        note: row.get(3)?,
        version: row.get(4)?,
        created_at: parse_ts(&row.get::<_, String>(5)?),
        updated_at: parse_ts(&row.get::<_, String>(6)?),
        updated_by: row.get(7)?,
        deleted: row.get::<_, i64>(8)? != 0,
    })
}

pub fn row_to_comment(row: &Row) -> rusqlite::Result<Comment> {
    Ok(Comment {
        id: row.get(0)?,
        target_kind: CommentTargetKind::from_wire(&row.get::<_, String>(1)?)
            .unwrap_or(CommentTargetKind::Page),
        target_id: row.get(2)?,
        body: row.get(3)?,
        resolved: row.get::<_, i64>(4)? != 0,
        author_id: row.get(5)?,
        author_name: row.get(6)?,
        version: row.get(7)?,
        created_at: parse_ts(&row.get::<_, String>(8)?),
        updated_at: parse_ts(&row.get::<_, String>(9)?),
        deleted: row.get::<_, i64>(10)? != 0,
    })
}

pub fn row_to_journal(row: &Row) -> rusqlite::Result<JournalEntry> {
    Ok(JournalEntry {
        id: row.get(0)?,
        target_kind: JournalTarget::from_wire(&row.get::<_, String>(1)?)
            .unwrap_or(JournalTarget::Daily),
        target_id: row.get(2)?,
        date: row.get(3)?,
        time: row.get(4)?,
        body: row.get(5)?,
        author_id: row.get(6)?,
        author_name: row.get(7)?,
        version: row.get(8)?,
        created_at: parse_ts(&row.get::<_, String>(9)?),
        updated_at: parse_ts(&row.get::<_, String>(10)?),
        deleted: row.get::<_, i64>(11)? != 0,
    })
}

const PAGE_COLUMNS: &str =
    "id, title, icon, kind, position, version, updated_at, updated_by, deleted, date";
const BLOCK_COLUMNS: &str = "id, page_id, kind, text, checked, due_date, streak, position, version,
    updated_at, updated_by, deleted, start_time, end_time, completed_at, completed_by, section";
const HABIT_COLUMNS: &str = "id, title, icon, note, frequency, position, archived, version,
    created_at, updated_at, updated_by, deleted";
const CHECKIN_COLUMNS: &str = "id, habit_id, date, note, version, created_at, updated_at,
    updated_by, deleted";
const COMMENT_COLUMNS: &str = "id, target_kind, target_id, body, resolved, author_id,
    author_name, version, created_at, updated_at, deleted";
const JOURNAL_COLUMNS: &str = "id, target_kind, target_id, date, time, body, author_id,
    author_name, version, created_at, updated_at, deleted";

pub fn list_pages(conn: &Connection) -> rusqlite::Result<Vec<Page>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {PAGE_COLUMNS} FROM pages ORDER BY position, updated_at"
    ))?;
    let rows = stmt.query_map([], row_to_page)?;
    rows.collect()
}

pub fn list_blocks(conn: &Connection) -> rusqlite::Result<Vec<Block>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {BLOCK_COLUMNS} FROM blocks ORDER BY page_id, position, updated_at"
    ))?;
    let rows = stmt.query_map([], row_to_block)?;
    rows.collect()
}

pub fn list_users(conn: &Connection) -> rusqlite::Result<Vec<User>> {
    let mut stmt =
        conn.prepare("SELECT id, name, public_key, created_at FROM users ORDER BY created_at")?;
    let rows = stmt.query_map([], row_to_user)?;
    rows.collect()
}

pub fn list_habits(conn: &Connection) -> rusqlite::Result<Vec<Habit>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {HABIT_COLUMNS} FROM habits ORDER BY position, created_at"
    ))?;
    let rows = stmt.query_map([], row_to_habit)?;
    rows.collect()
}

pub fn list_checkins(conn: &Connection) -> rusqlite::Result<Vec<HabitCheckin>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {CHECKIN_COLUMNS} FROM habit_checkins ORDER BY habit_id, date"
    ))?;
    let rows = stmt.query_map([], row_to_checkin)?;
    rows.collect()
}

pub fn list_comments(conn: &Connection) -> rusqlite::Result<Vec<Comment>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COMMENT_COLUMNS} FROM comments ORDER BY created_at"
    ))?;
    let rows = stmt.query_map([], row_to_comment)?;
    rows.collect()
}

pub fn list_journals(conn: &Connection) -> rusqlite::Result<Vec<JournalEntry>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {JOURNAL_COLUMNS} FROM journals ORDER BY date, created_at"
    ))?;
    let rows = stmt.query_map([], row_to_journal)?;
    rows.collect()
}

pub fn find_page(conn: &Connection, id: &str) -> rusqlite::Result<Option<Page>> {
    conn.query_row(
        &format!("SELECT {PAGE_COLUMNS} FROM pages WHERE id = ?1"),
        params![id],
        row_to_page,
    )
    .optional()
}

pub fn find_block(conn: &Connection, id: &str) -> rusqlite::Result<Option<Block>> {
    conn.query_row(
        &format!("SELECT {BLOCK_COLUMNS} FROM blocks WHERE id = ?1"),
        params![id],
        row_to_block,
    )
    .optional()
}

pub fn find_habit(conn: &Connection, id: &str) -> rusqlite::Result<Option<Habit>> {
    conn.query_row(
        &format!("SELECT {HABIT_COLUMNS} FROM habits WHERE id = ?1"),
        params![id],
        row_to_habit,
    )
    .optional()
}

pub fn find_checkin(conn: &Connection, id: &str) -> rusqlite::Result<Option<HabitCheckin>> {
    conn.query_row(
        &format!("SELECT {CHECKIN_COLUMNS} FROM habit_checkins WHERE id = ?1"),
        params![id],
        row_to_checkin,
    )
    .optional()
}

pub fn find_comment(conn: &Connection, id: &str) -> rusqlite::Result<Option<Comment>> {
    conn.query_row(
        &format!("SELECT {COMMENT_COLUMNS} FROM comments WHERE id = ?1"),
        params![id],
        row_to_comment,
    )
    .optional()
}

pub fn find_journal(conn: &Connection, id: &str) -> rusqlite::Result<Option<JournalEntry>> {
    conn.query_row(
        &format!("SELECT {JOURNAL_COLUMNS} FROM journals WHERE id = ?1"),
        params![id],
        row_to_journal,
    )
    .optional()
}

pub fn find_page_by_date(conn: &Connection, date: &str) -> rusqlite::Result<Option<Page>> {
    conn.query_row(
        &format!("SELECT {PAGE_COLUMNS} FROM pages WHERE date = ?1"),
        params![date],
        row_to_page,
    )
    .optional()
}

pub fn find_section_block(
    conn: &Connection,
    page_id: &str,
    section: BlockSection,
) -> rusqlite::Result<Option<Block>> {
    conn.query_row(
        &format!(
            "SELECT {BLOCK_COLUMNS} FROM blocks
             WHERE page_id = ?1 AND section = ?2
             ORDER BY deleted ASC, updated_at DESC LIMIT 1"
        ),
        params![page_id, section.as_wire()],
        row_to_block,
    )
    .optional()
}

/// 查某个习惯某天的有效打卡（未删除）。
pub fn find_checkin_by_date(
    conn: &Connection,
    habit_id: &str,
    date: &str,
) -> rusqlite::Result<Option<HabitCheckin>> {
    conn.query_row(
        &format!(
            "SELECT {CHECKIN_COLUMNS} FROM habit_checkins
             WHERE habit_id = ?1 AND date = ?2 AND deleted = 0"
        ),
        params![habit_id, date],
        row_to_checkin,
    )
    .optional()
}

pub fn insert_page(conn: &Connection, page: &Page) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO pages(id, title, icon, kind, position, version, updated_at, updated_by,
                           deleted, date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
            page.date,
        ],
    )?;
    Ok(())
}

pub fn insert_block(conn: &Connection, block: &Block) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO blocks(id, page_id, kind, text, checked, due_date, streak, position,
                            version, updated_at, updated_by, deleted, start_time, end_time,
                            completed_at, completed_by, section)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
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
            block.start_time,
            block.end_time,
            block.completed_at.map(|value| value.to_rfc3339()),
            block.completed_by,
            block.section.map(section_to_str),
        ],
    )?;
    Ok(())
}

pub fn insert_habit(conn: &Connection, habit: &Habit) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO habits(id, title, icon, note, frequency, position, archived, version,
                            created_at, updated_at, updated_by, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            habit.id,
            habit.title,
            habit.icon,
            habit.note,
            frequency_to_json(habit.frequency),
            habit.position,
            habit.archived as i64,
            habit.version,
            habit.created_at.to_rfc3339(),
            habit.updated_at.to_rfc3339(),
            habit.updated_by,
            habit.deleted as i64,
        ],
    )?;
    Ok(())
}

pub fn insert_checkin(conn: &Connection, checkin: &HabitCheckin) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO habit_checkins(id, habit_id, date, note, version, created_at, updated_at,
                                    updated_by, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            checkin.id,
            checkin.habit_id,
            checkin.date,
            checkin.note,
            checkin.version,
            checkin.created_at.to_rfc3339(),
            checkin.updated_at.to_rfc3339(),
            checkin.updated_by,
            checkin.deleted as i64,
        ],
    )?;
    Ok(())
}

pub fn insert_comment(conn: &Connection, comment: &Comment) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO comments(id, target_kind, target_id, body, resolved, author_id,
                              author_name, version, created_at, updated_at, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            comment.id,
            comment.target_kind.as_wire(),
            comment.target_id,
            comment.body,
            comment.resolved as i64,
            comment.author_id,
            comment.author_name,
            comment.version,
            comment.created_at.to_rfc3339(),
            comment.updated_at.to_rfc3339(),
            comment.deleted as i64,
        ],
    )?;
    Ok(())
}

pub fn insert_journal(conn: &Connection, journal: &JournalEntry) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO journals(id, target_kind, target_id, date, time, body, author_id,
                              author_name, version, created_at, updated_at, deleted)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            journal.id,
            journal.target_kind.as_wire(),
            journal.target_id,
            journal.date,
            journal.time,
            journal.body,
            journal.author_id,
            journal.author_name,
            journal.version,
            journal.created_at.to_rfc3339(),
            journal.updated_at.to_rfc3339(),
            journal.deleted as i64,
        ],
    )?;
    Ok(())
}

pub fn update_page(conn: &Connection, page: &Page) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE pages SET title = ?2, icon = ?3, kind = ?4, position = ?5, version = ?6,
                          updated_at = ?7, updated_by = ?8, deleted = ?9, date = ?10
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
            page.date,
        ],
    )?;
    Ok(())
}

pub fn update_block(conn: &Connection, block: &Block) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE blocks SET text = ?2, checked = ?3, due_date = ?4, streak = ?5, position = ?6,
                           version = ?7, updated_at = ?8, updated_by = ?9, deleted = ?10,
                           start_time = ?11, end_time = ?12, completed_at = ?13,
                           completed_by = ?14, section = ?15
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
            block.start_time,
            block.end_time,
            block.completed_at.map(|value| value.to_rfc3339()),
            block.completed_by,
            block.section.map(section_to_str),
        ],
    )?;
    Ok(())
}

pub fn update_habit(conn: &Connection, habit: &Habit) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE habits SET title = ?2, icon = ?3, note = ?4, frequency = ?5, position = ?6,
                           archived = ?7, version = ?8, updated_at = ?9, updated_by = ?10,
                           deleted = ?11
         WHERE id = ?1",
        params![
            habit.id,
            habit.title,
            habit.icon,
            habit.note,
            frequency_to_json(habit.frequency),
            habit.position,
            habit.archived as i64,
            habit.version,
            habit.updated_at.to_rfc3339(),
            habit.updated_by,
            habit.deleted as i64,
        ],
    )?;
    Ok(())
}

pub fn update_checkin(conn: &Connection, checkin: &HabitCheckin) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE habit_checkins SET note = ?2, version = ?3, updated_at = ?4, updated_by = ?5,
                                   deleted = ?6
         WHERE id = ?1",
        params![
            checkin.id,
            checkin.note,
            checkin.version,
            checkin.updated_at.to_rfc3339(),
            checkin.updated_by,
            checkin.deleted as i64,
        ],
    )?;
    Ok(())
}

pub fn update_comment(conn: &Connection, comment: &Comment) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE comments SET body = ?2, resolved = ?3, version = ?4, updated_at = ?5,
                             deleted = ?6
         WHERE id = ?1",
        params![
            comment.id,
            comment.body,
            comment.resolved as i64,
            comment.version,
            comment.updated_at.to_rfc3339(),
            comment.deleted as i64,
        ],
    )?;
    Ok(())
}

pub fn update_journal(conn: &Connection, journal: &JournalEntry) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE journals SET body = ?2, version = ?3, updated_at = ?4, deleted = ?5
         WHERE id = ?1",
        params![
            journal.id,
            journal.body,
            journal.version,
            journal.updated_at.to_rfc3339(),
            journal.deleted as i64,
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

fn write_with_change<T>(
    conn: &Connection,
    entity: EntityKind,
    value: &T,
    actor: &str,
    at: DateTime<Utc>,
    write: impl FnOnce(&Connection, &T) -> rusqlite::Result<()>,
) -> AnyResult<i64>
where
    T: serde::Serialize,
{
    let tx = conn.unchecked_transaction()?;
    write(&tx, value)?;
    let data = serde_json::to_value(value)?;
    let seq = push_change(&tx, entity.as_wire(), &data, actor, at)?;
    tx.commit()?;
    Ok(seq)
}

pub fn insert_page_and_change(conn: &Connection, page: &Page, actor: &str) -> AnyResult<i64> {
    write_with_change(
        conn,
        EntityKind::Page,
        page,
        actor,
        page.updated_at,
        insert_page,
    )
}

pub fn update_page_and_change(conn: &Connection, page: &Page, actor: &str) -> AnyResult<i64> {
    write_with_change(
        conn,
        EntityKind::Page,
        page,
        actor,
        page.updated_at,
        update_page,
    )
}

pub fn insert_block_and_change(conn: &Connection, block: &Block, actor: &str) -> AnyResult<i64> {
    write_with_change(
        conn,
        EntityKind::Block,
        block,
        actor,
        block.updated_at,
        insert_block,
    )
}

pub fn update_block_and_change(conn: &Connection, block: &Block, actor: &str) -> AnyResult<i64> {
    write_with_change(
        conn,
        EntityKind::Block,
        block,
        actor,
        block.updated_at,
        update_block,
    )
}

pub fn insert_habit_and_change(conn: &Connection, habit: &Habit, actor: &str) -> AnyResult<i64> {
    write_with_change(
        conn,
        EntityKind::Habit,
        habit,
        actor,
        habit.updated_at,
        insert_habit,
    )
}

pub fn update_habit_and_change(conn: &Connection, habit: &Habit, actor: &str) -> AnyResult<i64> {
    write_with_change(
        conn,
        EntityKind::Habit,
        habit,
        actor,
        habit.updated_at,
        update_habit,
    )
}

pub fn insert_checkin_and_change(
    conn: &Connection,
    checkin: &HabitCheckin,
    actor: &str,
) -> AnyResult<i64> {
    write_with_change(
        conn,
        EntityKind::HabitCheckin,
        checkin,
        actor,
        checkin.updated_at,
        insert_checkin,
    )
}

pub fn update_checkin_and_change(
    conn: &Connection,
    checkin: &HabitCheckin,
    actor: &str,
) -> AnyResult<i64> {
    write_with_change(
        conn,
        EntityKind::HabitCheckin,
        checkin,
        actor,
        checkin.updated_at,
        update_checkin,
    )
}

pub fn insert_comment_and_change(
    conn: &Connection,
    comment: &Comment,
    actor: &str,
) -> AnyResult<i64> {
    write_with_change(
        conn,
        EntityKind::Comment,
        comment,
        actor,
        comment.updated_at,
        insert_comment,
    )
}

pub fn update_comment_and_change(
    conn: &Connection,
    comment: &Comment,
    actor: &str,
) -> AnyResult<i64> {
    write_with_change(
        conn,
        EntityKind::Comment,
        comment,
        actor,
        comment.updated_at,
        update_comment,
    )
}

pub fn insert_journal_and_change(
    conn: &Connection,
    journal: &JournalEntry,
    actor: &str,
) -> AnyResult<i64> {
    write_with_change(
        conn,
        EntityKind::Journal,
        journal,
        actor,
        journal.updated_at,
        insert_journal,
    )
}

pub fn update_journal_and_change(
    conn: &Connection,
    journal: &JournalEntry,
    actor: &str,
) -> AnyResult<i64> {
    write_with_change(
        conn,
        EntityKind::Journal,
        journal,
        actor,
        journal.updated_at,
        update_journal,
    )
}
