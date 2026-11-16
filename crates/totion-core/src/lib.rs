use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type Id = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    pub id: Id,
    pub name: String,
    pub public_key: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PageKind {
    Doc,
    Plan,
    Habit,
    Todo,
    Calendar,
}

impl PageKind {
    pub const ALL: [PageKind; 5] = [
        PageKind::Doc,
        PageKind::Plan,
        PageKind::Habit,
        PageKind::Todo,
        PageKind::Calendar,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PageKind::Doc => "文档",
            PageKind::Plan => "计划",
            PageKind::Habit => "习惯",
            PageKind::Todo => "待办",
            PageKind::Calendar => "日历",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            PageKind::Doc => "📄",
            PageKind::Plan => "🎯",
            PageKind::Habit => "🌱",
            PageKind::Todo => "✅",
            PageKind::Calendar => "📅",
        }
    }

    pub fn next(self) -> Self {
        match self {
            PageKind::Doc => PageKind::Plan,
            PageKind::Plan => PageKind::Habit,
            PageKind::Habit => PageKind::Todo,
            PageKind::Todo => PageKind::Calendar,
            PageKind::Calendar => PageKind::Doc,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Page {
    pub id: Id,
    pub title: String,
    pub icon: String,
    pub kind: PageKind,
    pub position: i64,
    pub version: i64,
    pub updated_at: DateTime<Utc>,
    pub updated_by: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum BlockKind {
    Text,
    Heading,
    Todo,
    Habit,
    CalendarEvent,
    Divider,
}

impl BlockKind {
    pub fn icon(self) -> &'static str {
        match self {
            BlockKind::Text => "·",
            BlockKind::Heading => "H",
            BlockKind::Todo => "[ ]",
            BlockKind::Habit => "🌱",
            BlockKind::CalendarEvent => "📅",
            BlockKind::Divider => "—",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Block {
    pub id: Id,
    pub page_id: Id,
    pub kind: BlockKind,
    pub text: String,
    pub checked: bool,
    /// 截止日期 / 事件日期 / 习惯最近打卡日期，格式 YYYY-MM-DD
    pub due_date: Option<String>,
    /// 习惯连续打卡天数
    pub streak: i64,
    pub position: i64,
    pub version: i64,
    pub updated_at: DateTime<Utc>,
    pub updated_by: String,
    pub deleted: bool,
}

impl Block {
    pub fn is_done(&self) -> bool {
        self.checked
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Page,
    Block,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Change {
    pub seq: i64,
    pub entity: EntityKind,
    /// Page 或 Block 的完整 JSON 快照
    pub data: serde_json::Value,
    /// 操作者用户名
    pub actor: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub name: String,
    pub users: Vec<User>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub workspace: WorkspaceInfo,
    pub pages: Vec<Page>,
    pub blocks: Vec<Block>,
    pub last_seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    pub changes: Vec<Change>,
    pub last_seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub invite_code: String,
    pub name: String,
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
    pub workspace_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeRequest {
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeResponse {
    pub challenge_id: String,
    pub nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRequest {
    pub challenge_id: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePageRequest {
    pub title: String,
    pub icon: String,
    pub kind: PageKind,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePageRequest {
    pub expected_version: i64,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub kind: Option<PageKind>,
    pub position: Option<i64>,
    pub deleted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBlockRequest {
    pub page_id: Id,
    pub kind: BlockKind,
    pub text: String,
    pub due_date: Option<String>,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateBlockRequest {
    pub expected_version: i64,
    pub text: Option<String>,
    pub checked: Option<bool>,
    pub due_date: Option<String>,
    pub streak: Option<i64>,
    pub position: Option<i64>,
    pub deleted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritePageResponse {
    pub page: Page,
    pub seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteBlockResponse {
    pub block: Block,
    pub seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<serde_json::Value>,
}

/// 打卡后的新连续天数。今天已打卡返回 None；昨天打过卡则 +1，否则从 1 重新开始。
pub fn habit_check_in(last_date: Option<&str>, today: &str, old_streak: i64) -> Option<i64> {
    let yesterday = chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d")
        .ok()
        .map(|d| {
            (d - chrono::Duration::days(1))
                .format("%Y-%m-%d")
                .to_string()
        });
    match last_date {
        Some(last) if last == today => None,
        Some(last) if Some(last) == yesterday.as_deref() => Some(old_streak + 1),
        _ => Some(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn habit_streak_increments_after_yesterday() {
        assert_eq!(habit_check_in(Some("2026-11-15"), "2026-11-16", 4), Some(5));
        assert_eq!(habit_check_in(Some("2026-11-10"), "2026-11-16", 4), Some(1));
        assert_eq!(habit_check_in(Some("2026-11-16"), "2026-11-16", 4), None);
        assert_eq!(habit_check_in(None, "2026-11-16", 0), Some(1));
    }
}
