//! 数据层：把服务端快照缓存成可查询的集合，并应用增量变更。

use chrono::{Local, NaiveDate};
use totion_core::{
    Block, Comment, CommentTargetKind, Habit, HabitCheckin, HabitStats, JournalEntry,
    JournalTarget, Page, Snapshot, User,
};

#[derive(Default)]
pub struct Store {
    pub workspace_name: String,
    pub users: Vec<User>,
    pub pages: Vec<Page>,
    pub blocks: Vec<Block>,
    pub habits: Vec<Habit>,
    pub checkins: Vec<HabitCheckin>,
    pub comments: Vec<Comment>,
    pub journals: Vec<JournalEntry>,
    pub last_seq: i64,
}

impl Store {
    pub fn from_snapshot(snapshot: Snapshot) -> Self {
        Self {
            workspace_name: snapshot.workspace.name,
            users: snapshot.workspace.users,
            pages: snapshot.pages,
            blocks: snapshot.blocks,
            habits: snapshot.habits,
            checkins: snapshot.checkins,
            comments: snapshot.comments,
            journals: snapshot.journals,
            last_seq: snapshot.last_seq,
        }
    }

    pub fn upsert<T: Clone + PartialEq>(list: &mut Vec<T>, item: T, same: impl Fn(&T) -> bool) {
        match list.iter_mut().find(|existing| same(existing)) {
            Some(slot) => *slot = item,
            None => list.push(item),
        }
    }

    pub fn apply(&mut self, change: totion_core::Change) {
        use totion_core::EntityKind;
        self.last_seq = self.last_seq.max(change.seq);
        match change.entity {
            EntityKind::Page => {
                if let Ok(item) = serde_json::from_value::<Page>(change.data) {
                    let id = item.id.clone();
                    Self::upsert(&mut self.pages, item, |x| x.id == id);
                }
            }
            EntityKind::Block => {
                if let Ok(item) = serde_json::from_value::<Block>(change.data) {
                    let id = item.id.clone();
                    Self::upsert(&mut self.blocks, item, |x| x.id == id);
                }
            }
            EntityKind::Habit => {
                if let Ok(item) = serde_json::from_value::<Habit>(change.data) {
                    let id = item.id.clone();
                    Self::upsert(&mut self.habits, item, |x| x.id == id);
                }
            }
            EntityKind::HabitCheckin => {
                if let Ok(item) = serde_json::from_value::<HabitCheckin>(change.data) {
                    let id = item.id.clone();
                    Self::upsert(&mut self.checkins, item, |x| x.id == id);
                }
            }
            EntityKind::Comment => {
                if let Ok(item) = serde_json::from_value::<Comment>(change.data) {
                    let id = item.id.clone();
                    Self::upsert(&mut self.comments, item, |x| x.id == id);
                }
            }
            EntityKind::Journal => {
                if let Ok(item) = serde_json::from_value::<JournalEntry>(change.data) {
                    let id = item.id.clone();
                    Self::upsert(&mut self.journals, item, |x| x.id == id);
                }
            }
        }
    }

    pub fn user_name(&self, id: &str) -> String {
        self.users
            .iter()
            .find(|user| user.id == id)
            .map(|user| user.name.clone())
            .unwrap_or_default()
    }

    /// 某一天的待办/日程，按开始时间排序。
    pub fn day_blocks(&self, date: &str) -> Vec<Block> {
        let mut items: Vec<Block> = self
            .blocks
            .iter()
            .filter(|block| {
                !block.deleted
                    && block.due_date.as_deref() == Some(date)
                    && matches!(block.kind, totion_core::BlockKind::Todo | totion_core::BlockKind::CalendarEvent)
            })
            .cloned()
            .collect();
        items.sort_by(|a, b| {
            a.start_time
                .as_deref()
                .unwrap_or("99:99")
                .cmp(b.start_time.as_deref().unwrap_or("99:99"))
        });
        items
    }

    pub fn page_blocks(&self, page_id: &str) -> Vec<Block> {
        let mut items: Vec<Block> = self
            .blocks
            .iter()
            .filter(|block| !block.deleted && block.page_id == page_id)
            .cloned()
            .collect();
        items.sort_by_key(|block| block.position);
        items
    }

    pub fn visible_pages(&self) -> Vec<Page> {
        let mut items: Vec<Page> = self
            .pages
            .iter()
            .filter(|page| !page.deleted && page.date.is_none())
            .cloned()
            .collect();
        items.sort_by_key(|page| (page.kind as u8, page.position));
        items
    }

    pub fn active_habits(&self) -> Vec<Habit> {
        let mut items: Vec<Habit> = self
            .habits
            .iter()
            .filter(|habit| !habit.deleted && !habit.archived)
            .cloned()
            .collect();
        items.sort_by_key(|habit| habit.position);
        items
    }

    pub fn checkin(&self, habit_id: &str, date: &str) -> Option<&HabitCheckin> {
        self.checkins
            .iter()
            .find(|c| !c.deleted && c.habit_id == habit_id && c.date == date)
    }

    pub fn habit_stats(&self, habit: &Habit, today: NaiveDate) -> HabitStats {
        let mine: Vec<HabitCheckin> = self
            .checkins
            .iter()
            .filter(|c| c.habit_id == habit.id)
            .cloned()
            .collect();
        totion_core::habit_stats(habit.frequency, &habit.id, &mine, today)
    }

    pub fn comments_for(&self, kind: CommentTargetKind, id: &str) -> Vec<Comment> {
        let mut items: Vec<Comment> = self
            .comments
            .iter()
            .filter(|c| !c.deleted && c.target_kind == kind && c.target_id == id)
            .cloned()
            .collect();
        items.sort_by_key(|c| c.created_at);
        items
    }

    pub fn open_comments(&self) -> Vec<Comment> {
        let mut items: Vec<Comment> = self
            .comments
            .iter()
            .filter(|c| !c.deleted && !c.resolved)
            .cloned()
            .collect();
        items.sort_by_key(|c| std::cmp::Reverse(c.created_at));
        items
    }

    pub fn journals_for(&self, kind: JournalTarget, id: &str) -> Vec<JournalEntry> {
        let mut items: Vec<JournalEntry> = self
            .journals
            .iter()
            .filter(|j| !j.deleted && j.target_kind == kind && j.target_id == id)
            .cloned()
            .collect();
        items.sort_by_key(|j| (j.date.clone(), j.time.clone()));
        items
    }

    pub fn daily_summary(&self, date: &str) -> Option<JournalEntry> {
        let mut items = self.journals_for(JournalTarget::Daily, date);
        items.pop()
    }

    pub fn all_journals(&self) -> Vec<JournalEntry> {
        let mut items: Vec<JournalEntry> = self
            .journals
            .iter()
            .filter(|j| !j.deleted)
            .cloned()
            .collect();
        items.sort_by_key(|j| (j.date.clone(), j.time.clone()));
        items.reverse();
        items
    }

    pub fn today(&self) -> String {
        Local::now().date_naive().format("%Y-%m-%d").to_string()
    }

    pub fn day_completion(&self, date: &str) -> (usize, usize) {
        let items = self.day_blocks(date);
        let total = items.len();
        let done = items.iter().filter(|block| block.checked).count();
        (done, total)
    }
}
