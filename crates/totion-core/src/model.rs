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
    /// 日报页归属日期（YYYY-MM-DD）。普通页面为 None。
    #[serde(default)]
    pub date: Option<String>,
}

impl Page {
    pub fn is_daily(&self) -> bool {
        self.date.is_some()
    }
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

/// 日报页固定的三个记录区。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum BlockSection {
    Summary,
    Reflection,
    Lessons,
}

impl BlockSection {
    pub const ALL: [BlockSection; 3] = [
        BlockSection::Summary,
        BlockSection::Reflection,
        BlockSection::Lessons,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BlockSection::Summary => "总结",
            BlockSection::Reflection => "感悟",
            BlockSection::Lessons => "经验",
        }
    }

    pub fn as_wire(self) -> &'static str {
        match self {
            BlockSection::Summary => "summary",
            BlockSection::Reflection => "reflection",
            BlockSection::Lessons => "lessons",
        }
    }

    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "summary" => Some(BlockSection::Summary),
            "reflection" => Some(BlockSection::Reflection),
            "lessons" => Some(BlockSection::Lessons),
            _ => None,
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
    /// 截止日期 / 事件日期 / 旧版习惯最近打卡日期，格式 YYYY-MM-DD
    pub due_date: Option<String>,
    /// 旧版习惯连续打卡天数（新习惯实体不再使用）
    pub streak: i64,
    pub position: i64,
    pub version: i64,
    pub updated_at: DateTime<Utc>,
    pub updated_by: String,
    pub deleted: bool,
    /// 安排时间 HH:MM
    #[serde(default)]
    pub start_time: Option<String>,
    #[serde(default)]
    pub end_time: Option<String>,
    #[serde(default)]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub completed_by: Option<String>,
    /// 日报页的记录区类型
    #[serde(default)]
    pub section: Option<BlockSection>,
    /// 创建者（归属成员）id；旧数据为空字符串
    #[serde(default)]
    pub owner_id: Id,
}

impl Block {
    pub fn is_done(&self) -> bool {
        self.checked
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HabitFrequency {
    #[default]
    Daily,
    Weekdays,
    Weekly {
        times: u8,
    },
}

impl HabitFrequency {
    pub fn label(self) -> String {
        match self {
            HabitFrequency::Daily => "每天".to_string(),
            HabitFrequency::Weekdays => "工作日".to_string(),
            HabitFrequency::Weekly { times } => format!("每周 {times} 次"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Habit {
    pub id: Id,
    pub title: String,
    pub icon: String,
    pub note: String,
    #[serde(default)]
    pub frequency: HabitFrequency,
    pub position: i64,
    pub archived: bool,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub updated_by: String,
    pub deleted: bool,
    /// 创建者（归属成员）id；旧数据为空字符串
    #[serde(default)]
    pub owner_id: Id,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HabitCheckin {
    pub id: Id,
    pub habit_id: Id,
    /// 本地日期 YYYY-MM-DD
    pub date: String,
    pub note: String,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub updated_by: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CommentTargetKind {
    Page,
    Block,
    Habit,
}

impl CommentTargetKind {
    pub fn label(self) -> &'static str {
        match self {
            CommentTargetKind::Page => "页面",
            CommentTargetKind::Block => "内容块",
            CommentTargetKind::Habit => "习惯",
        }
    }

    pub fn as_wire(self) -> &'static str {
        match self {
            CommentTargetKind::Page => "page",
            CommentTargetKind::Block => "block",
            CommentTargetKind::Habit => "habit",
        }
    }

    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "page" => Some(CommentTargetKind::Page),
            "block" => Some(CommentTargetKind::Block),
            "habit" => Some(CommentTargetKind::Habit),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Comment {
    pub id: Id,
    pub target_kind: CommentTargetKind,
    pub target_id: Id,
    pub body: String,
    pub resolved: bool,
    pub author_id: Id,
    pub author_name: String,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted: bool,
}

/// 总结挂在什么对象上。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum JournalTarget {
    /// 当天整体总结，target_id 存日期
    Daily,
    Block,
    Habit,
}

impl JournalTarget {
    pub fn label(self) -> &'static str {
        match self {
            JournalTarget::Daily => "当日",
            JournalTarget::Block => "待办",
            JournalTarget::Habit => "习惯",
        }
    }

    pub fn as_wire(self) -> &'static str {
        match self {
            JournalTarget::Daily => "daily",
            JournalTarget::Block => "block",
            JournalTarget::Habit => "habit",
        }
    }

    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "daily" => Some(JournalTarget::Daily),
            "block" => Some(JournalTarget::Block),
            "habit" => Some(JournalTarget::Habit),
            _ => None,
        }
    }
}

/// 一条总结：对某个待办/习惯/当天的记录，带日期与时间。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JournalEntry {
    pub id: Id,
    pub target_kind: JournalTarget,
    /// Daily 时是日期字符串；Block/Habit 时是对象 id
    pub target_id: Id,
    pub date: String,
    pub time: String,
    pub body: String,
    pub author_id: Id,
    pub author_name: String,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted: bool,
}

/// 一个邀请码。同一个空间的多个邀请码都能用来注册。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Invite {
    pub id: Id,
    pub code: String,
    pub note: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_uses: Option<u32>,
    pub used_count: u32,
    pub revoked: bool,
}

impl Invite {
    pub fn is_active(&self, now: DateTime<Utc>) -> bool {
        !self.revoked
            && self.expires_at.map(|expire| expire > now).unwrap_or(true)
            && self
                .max_uses
                .map(|max| self.used_count < max)
                .unwrap_or(true)
    }

    pub fn status_label(&self, now: DateTime<Utc>) -> &'static str {
        if self.revoked {
            "已撤销"
        } else if self.expires_at.map(|expire| expire <= now).unwrap_or(false) {
            "已过期"
        } else if self
            .max_uses
            .map(|max| self.used_count >= max)
            .unwrap_or(false)
        {
            "已用尽"
        } else {
            "有效"
        }
    }
}
