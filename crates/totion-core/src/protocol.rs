use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Page,
    Block,
    Habit,
    HabitCheckin,
    Comment,
}

impl EntityKind {
    pub fn as_wire(self) -> &'static str {
        match self {
            EntityKind::Page => "page",
            EntityKind::Block => "block",
            EntityKind::Habit => "habit",
            EntityKind::HabitCheckin => "habit_checkin",
            EntityKind::Comment => "comment",
        }
    }

    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "page" => Some(EntityKind::Page),
            "block" => Some(EntityKind::Block),
            "habit" => Some(EntityKind::Habit),
            "habit_checkin" => Some(EntityKind::HabitCheckin),
            "comment" => Some(EntityKind::Comment),
            _ => None,
        }
    }

    /// v1 客户端只认识 page/block。
    pub fn is_v1(self) -> bool {
        matches!(self, EntityKind::Page | EntityKind::Block)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Change {
    pub seq: i64,
    pub entity: EntityKind,
    /// Page/Block/Habit/HabitCheckin/Comment 的完整 JSON 快照
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

pub const API_VERSION: u32 = 2;

fn default_api_version() -> u32 {
    API_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub workspace: WorkspaceInfo,
    pub pages: Vec<Page>,
    pub blocks: Vec<Block>,
    #[serde(default)]
    pub habits: Vec<Habit>,
    #[serde(default)]
    pub checkins: Vec<HabitCheckin>,
    #[serde(default)]
    pub comments: Vec<Comment>,
    pub last_seq: i64,
    #[serde(default = "default_api_version")]
    pub api_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    pub changes: Vec<Change>,
    /// 游标：本批最后一条变更的 seq；没有变更时等于请求的 since。
    pub last_seq: i64,
    #[serde(default)]
    pub has_more: bool,
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
    #[serde(default)]
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePageRequest {
    pub expected_version: i64,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub kind: Option<PageKind>,
    pub position: Option<i64>,
    pub deleted: Option<bool>,
    #[serde(default)]
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBlockRequest {
    pub page_id: Id,
    pub kind: BlockKind,
    pub text: String,
    pub due_date: Option<String>,
    pub position: i64,
    #[serde(default)]
    pub start_time: Option<String>,
    #[serde(default)]
    pub end_time: Option<String>,
    #[serde(default)]
    pub section: Option<BlockSection>,
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
    #[serde(default)]
    pub start_time: Option<String>,
    #[serde(default)]
    pub end_time: Option<String>,
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
pub struct CreateHabitRequest {
    pub title: String,
    pub icon: String,
    pub note: String,
    #[serde(default)]
    pub frequency: HabitFrequency,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateHabitRequest {
    pub expected_version: i64,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub note: Option<String>,
    pub frequency: Option<HabitFrequency>,
    pub position: Option<i64>,
    pub archived: Option<bool>,
    pub deleted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteHabitResponse {
    pub habit: Habit,
    pub seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckinRequest {
    /// YYYY-MM-DD（客户端本地日期）
    pub date: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckinRequest {
    pub expected_version: i64,
    pub note: Option<String>,
    pub deleted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteCheckinResponse {
    pub checkin: HabitCheckin,
    /// 已存在时不会产生新变更，seq 为 None。
    pub seq: Option<i64>,
    pub created: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCommentRequest {
    pub target_kind: CommentTargetKind,
    pub target_id: Id,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCommentRequest {
    pub expected_version: i64,
    pub body: Option<String>,
    pub resolved: Option<bool>,
    pub deleted: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteCommentResponse {
    pub comment: Comment,
    pub seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsureDayResponse {
    pub page: Page,
    pub blocks: Vec<Block>,
    pub last_seq: i64,
    pub created: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<serde_json::Value>,
}
