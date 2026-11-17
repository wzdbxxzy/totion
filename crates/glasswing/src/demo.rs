//! 演示数据：一套完整的假数据，用来把每个界面都填满。
//! 通过 `GLASSWING_DEMO=1` 启用，不连服务器、不写任何东西。

use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Utc};
use totion_core::{
    Block, BlockKind, Comment, CommentTargetKind, Habit, HabitCheckin, HabitFrequency, JournalEntry,
    JournalTarget, Page, PageKind, User,
};

use crate::store::Store;

fn at(days_ago: i64) -> chrono::DateTime<Utc> {
    let local = Local::now() - Duration::days(days_ago);
    Utc.from_utc_datetime(&local.naive_local())
}

fn user(id: &str, name: &str) -> User {
    User {
        id: id.to_string(),
        name: name.to_string(),
        public_key: format!("demo-{id}"),
        created_at: at(90),
    }
}

fn page(id: &str, title: &str, icon: &str, kind: PageKind, position: i64, date: Option<&str>) -> Page {
    Page {
        id: id.to_string(),
        title: title.to_string(),
        icon: icon.to_string(),
        kind,
        position,
        version: 1,
        updated_at: at(1),
        updated_by: "Guan".to_string(),
        deleted: false,
        date: date.map(|d| d.to_string()),
    }
}

#[allow(clippy::too_many_arguments)]
fn block(
    id: &str,
    page_id: &str,
    kind: BlockKind,
    text: &str,
    checked: bool,
    due: Option<&str>,
    time: Option<&str>,
    position: i64,
    owner: &str,
) -> Block {
    Block {
        id: id.to_string(),
        page_id: page_id.to_string(),
        kind,
        text: text.to_string(),
        checked,
        due_date: due.map(|d| d.to_string()),
        streak: 0,
        position,
        version: 1,
        updated_at: at(1),
        updated_by: if owner == "u1" { "Guan" } else { "Mira" }.to_string(),
        deleted: false,
        start_time: time.map(|t| t.to_string()),
        end_time: None,
        completed_at: if checked { Some(at(0)) } else { None },
        completed_by: if checked { Some(owner.to_string()) } else { None },
        section: None,
        owner_id: owner.to_string(),
    }
}

fn habit(id: &str, title: &str, icon: &str, freq: HabitFrequency, owner: &str, pos: i64) -> Habit {
    Habit {
        id: id.to_string(),
        title: title.to_string(),
        icon: icon.to_string(),
        note: String::new(),
        frequency: freq,
        position: pos,
        archived: false,
        version: 1,
        created_at: at(80),
        updated_at: at(2),
        updated_by: "Guan".to_string(),
        deleted: false,
        owner_id: owner.to_string(),
    }
}

fn checkin(id: &str, habit_id: &str, date: &str) -> HabitCheckin {
    HabitCheckin {
        id: id.to_string(),
        habit_id: habit_id.to_string(),
        date: date.to_string(),
        note: String::new(),
        version: 1,
        created_at: at(1),
        updated_at: at(1),
        updated_by: "Guan".to_string(),
        deleted: false,
    }
}

fn comment(
    id: &str,
    kind: CommentTargetKind,
    target: &str,
    body: &str,
    resolved: bool,
    author: &str,
    days_ago: i64,
) -> Comment {
    Comment {
        id: id.to_string(),
        target_kind: kind,
        target_id: target.to_string(),
        body: body.to_string(),
        resolved,
        author_id: if author == "Guan" { "u1" } else { "u2" }.to_string(),
        author_name: author.to_string(),
        version: 1,
        created_at: at(days_ago),
        updated_at: at(days_ago),
        deleted: false,
    }
}

fn journal(
    id: &str,
    kind: JournalTarget,
    target: &str,
    date: &str,
    body: &str,
    author: &str,
) -> JournalEntry {
    JournalEntry {
        id: id.to_string(),
        target_kind: kind,
        target_id: target.to_string(),
        date: date.to_string(),
        time: "21:30".to_string(),
        body: body.to_string(),
        author_id: if author == "Guan" { "u1" } else { "u2" }.to_string(),
        author_name: author.to_string(),
        version: 1,
        created_at: at(1),
        updated_at: at(1),
        deleted: false,
    }
}

pub fn build() -> Store {
    let today = Local::now().date_naive();
    let day = |offset: i64| -> String {
        (today + Duration::days(offset))
            .format("%Y-%m-%d")
            .to_string()
    };

    let users = vec![
        user("u1", "Guan"),
        user("u2", "Mira"),
        user("u3", "阿野"),
    ];

    let mut pages = vec![
        page("p-inbox", "收件箱", "◻", PageKind::Todo, 0, None),
        page("p-notes", "读书笔记", "◻", PageKind::Doc, 1, None),
        page("p-ideas", "灵感池", "◻", PageKind::Doc, 2, None),
        page("p-travel", "京都行程", "◻", PageKind::Doc, 3, None),
        page("p-work", "这周的工作", "◻", PageKind::Todo, 4, None),
    ];
    // 今日 / 昨天的日报页
    for offset in -2..=1 {
        pages.push(page(
            &format!("p-day{offset}"),
            &day(offset),
            "",
            PageKind::Doc,
            10,
            Some(&day(offset)),
        ));
    }

    let mut blocks = Vec::new();
    // 今日待办
    let todo_today = [
        ("b1", "写 Glasswing 的日历视图", true, Some("09:30")),
        ("b2", "和 Mira 对一下设计稿", false, Some("14:00")),
        ("b3", "跑步 5 公里", false, Some("19:00")),
        ("b4", "整理这周的读书笔记", false, None),
        ("b5", "给服务器做一次备份", true, Some("11:00")),
    ];
    for (index, (id, text, checked, time)) in todo_today.iter().enumerate() {
        blocks.push(block(
            id,
            "p-inbox",
            BlockKind::Todo,
            text,
            *checked,
            Some(&day(0)),
            *time,
            index as i64,
            "u1",
        ));
    }
    // 明天 / 后天
    blocks.push(block("b6", "p-inbox", BlockKind::Todo, "去邮局取快递", false, Some(&day(1)), Some("10:00"), 0, "u1"));
    blocks.push(block("b7", "p-inbox", BlockKind::Todo, "牙医复诊", false, Some(&day(2)), Some("15:30"), 1, "u1"));
    blocks.push(block("b8", "p-inbox", BlockKind::Todo, "交房租", false, Some(&day(3)), None, 2, "u2"));
    // 昨天之前
    for (index, offset) in [-1i64, -2, -4, -6, -9].iter().enumerate() {
        blocks.push(block(
            &format!("b-old{index}"),
            "p-inbox",
            BlockKind::Todo,
            ["读《禅与摩托车维修艺术》", "整理衣柜", "给妈妈打电话", "跑了 8 公里", "修好台灯"][index],
            true,
            Some(&day(*offset)),
            None,
            0,
            "u1",
        ));
    }

    // 工作区内容
    let notes = [
        ("n1", BlockKind::Heading, "第二章 · 品质", false),
        ("n2", BlockKind::Text, "良质不是主观也不是客观，而是主客体相遇的那一瞬间。", false),
        ("n3", BlockKind::Text, "作者用「维修摩托车」来讲这件事，比抽象论证好懂太多。", false),
        ("n4", BlockKind::Todo, "把这一章的观点用到接口设计上", false),
        ("n5", BlockKind::Text, "接口的「良质」大概是：用户不需要读文档也知道怎么用。", false),
    ];
    for (index, (id, kind, text, checked)) in notes.iter().enumerate() {
        blocks.push(block(id, "p-notes", *kind, text, *checked, None, None, index as i64, "u1"));
    }

    let ideas = [
        ("i1", "桌面挂件可以显示「今天的进度环」，完成时轻轻亮一下"),
        ("i2", "习惯连续天数到 7 的倍数时，换一个更亮的颜色"),
        ("i3", "周报用一句话总结：这周做了什么、下周要做什么"),
        ("i4", "把留言做成「收件箱」，处理完就归档"),
        ("i5", "日历里用圆点表示任务数量，比写文字清楚"),
    ];
    for (index, (id, text)) in ideas.iter().enumerate() {
        blocks.push(block(id, "p-ideas", BlockKind::Text, text, false, None, None, index as i64, "u1"));
    }

    let travel = [
        ("t1", BlockKind::Heading, "10 月 3 日 · 京都", false),
        ("t2", BlockKind::Todo, "清水寺（早 7 点前到）", true),
        ("t3", BlockKind::Todo, "锦市场吃早饭", false),
        ("t4", BlockKind::Todo, "鸭川散步", false),
        ("t5", BlockKind::Text, "住宿在四条河原町，走路去祇园十分钟。", false),
    ];
    for (index, (id, kind, text, checked)) in travel.iter().enumerate() {
        blocks.push(block(id, "p-travel", *kind, text, *checked, None, None, index as i64, "u2"));
    }

    blocks.push(block("w1", "p-work", BlockKind::Todo, "发布 0.4 版本", false, None, None, 0, "u1"));
    blocks.push(block("w2", "p-work", BlockKind::Todo, "写发布说明", true, None, None, 1, "u1"));
    blocks.push(block("w3", "p-work", BlockKind::Text, "下个迭代做：日历圆点、设置页、演示数据。", false, None, None, 2, "u1"));

    // 习惯
    let habits = vec![
        habit("h1", "骑车 10 公里", "◉", HabitFrequency::Daily, "u1", 0),
        habit("h2", "读书 30 分钟", "◉", HabitFrequency::Daily, "u1", 1),
        habit("h3", "喝水 2 升", "◉", HabitFrequency::Daily, "u2", 2),
        habit("h4", "健身房", "◉", HabitFrequency::Weekly { times: 3 }, "u1", 3),
        habit("h5", "早睡", "◉", HabitFrequency::Daily, "u2", 4),
        habit("h6", "写日记", "◉", HabitFrequency::Weekdays, "u1", 5),
    ];

    // 打卡：过去 70 天，按习惯给不同密度
    let mut checkins = Vec::new();
    let mut serial = 0;
    let mut seed = 7u32;
    let mut next = || {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        (seed >> 16) % 100
    };
    for (index, habit) in habits.iter().enumerate() {
        let density = [78, 62, 45, 30, 55, 70][index];
        for offset in 0..70i64 {
            let date = today - Duration::days(offset);
            if date.weekday().num_days_from_monday() >= 5 && index % 2 == 0 {
                continue;
            }
            if next() < density {
                serial += 1;
                checkins.push(checkin(
                    &format!("c{serial}"),
                    &habit.id,
                    &date.format("%Y-%m-%d").to_string(),
                ));
            }
        }
    }

    // 留言
    let comments = vec![
        comment("m1", CommentTargetKind::Block, "b2", "下午两点我有空，会议室 B？", false, "Mira", 0),
        comment("m2", CommentTargetKind::Block, "b2", "好，我把设计稿先传上去。", false, "Guan", 0),
        comment("m3", CommentTargetKind::Page, "p-notes", "这一章我也在读，可以一起讨论。", false, "阿野", 1),
        comment("m4", CommentTargetKind::Habit, "h4", "周三晚上一起去？", false, "Mira", 1),
        comment("m5", CommentTargetKind::Block, "b3", "今天下雨还跑吗", true, "Mira", 2),
        comment("m6", CommentTargetKind::Page, "p-work", "0.4 的发布说明我来写吧。", true, "Mira", 3),
        comment("m7", CommentTargetKind::Block, "b1", "日历的圆点比文字清楚多了。", false, "阿野", 0),
    ];

    // 总结
    let mut journals = vec![
        journal("j1", JournalTarget::Daily, &day(0), &day(0), "今天把日历从文字改成了圆点，整块界面一下子安静下来。晚上骑车绕了江边一圈，风很大。", "Guan"),
        journal("j2", JournalTarget::Daily, &day(-1), &day(-1), "写代码的时间被会议切碎了，明天把上午整块留给实现。", "Guan"),
        journal("j3", JournalTarget::Daily, &day(-2), &day(-2), "看完《禅与摩托车维修艺术》第二章，良质这个概念越想越有意思。", "Guan"),
        journal("j4", JournalTarget::Block, "b1", &day(0), "先做圆点版本，文字版本留作设置项。", "Guan"),
        journal("j5", JournalTarget::Habit, "h2", &day(-1), "读书半小时，读到关于「关心」的那一段。", "Guan"),
        journal("j6", JournalTarget::Daily, &day(-4), &day(-4), "把桌面挂件钉到桌面层，终于不像一个普通应用了。", "Guan"),
        journal("j7", JournalTarget::Daily, &day(-6), &day(-6), "整理了一版配色，浅蓝比深蓝更适合这张壁纸。", "Mira"),
    ];
    journals.sort_by_key(|journal| std::cmp::Reverse(journal.date.clone()));

    Store {
        workspace_name: "示例空间".to_string(),
        users,
        pages,
        blocks,
        habits,
        checkins,
        comments,
        journals,
        last_seq: 0,
    }
}

/// 今天是否是工作日（演示数据里用来给文案加一点变化）。
pub fn weekday_label(date: NaiveDate) -> &'static str {
    const NAMES: [&str; 7] = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
    NAMES[date.weekday().num_days_from_monday() as usize]
}
