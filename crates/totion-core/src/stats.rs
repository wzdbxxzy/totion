use std::collections::HashSet;

use chrono::{Datelike, Duration, NaiveDate};

use crate::date::{parse_date, week_range};
use crate::model::{Block, BlockKind, HabitCheckin, HabitFrequency};

/// 习惯在指定日期是否「应该做」。
/// `weekly_done` 是本周已完成次数，用于 Weekly 目标判断。
pub fn habit_due_on(frequency: HabitFrequency, date: NaiveDate, weekly_done: u32) -> bool {
    match frequency {
        HabitFrequency::Daily => true,
        HabitFrequency::Weekdays => date.weekday().num_days_from_monday() < 5,
        HabitFrequency::Weekly { times } => weekly_done < times as u32,
    }
}

/// 指定习惯在指定日期是否有有效打卡。
pub fn checkin_on<'a>(
    checkins: &'a [HabitCheckin],
    habit_id: &str,
    date: &str,
) -> Option<&'a HabitCheckin> {
    checkins
        .iter()
        .find(|item| item.habit_id == habit_id && item.date == date && !item.deleted)
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HabitStats {
    pub current_streak: u32,
    pub longest_streak: u32,
    pub total: u32,
    pub weekly_done: u32,
    pub weekly_target: Option<u8>,
    /// 最近 30 天完成率（0.0..=1.0）
    pub last_30_rate: f64,
    pub checked_today: bool,
}

pub fn habit_stats(
    frequency: HabitFrequency,
    habit_id: &str,
    checkins: &[HabitCheckin],
    today: NaiveDate,
) -> HabitStats {
    let dates: HashSet<NaiveDate> = checkins
        .iter()
        .filter(|item| item.habit_id == habit_id && !item.deleted)
        .filter_map(|item| parse_date(&item.date))
        .collect();

    let checked_today = dates.contains(&today);
    let mut cursor = if checked_today {
        today
    } else {
        today - Duration::days(1)
    };
    let mut current_streak = 0;
    while dates.contains(&cursor) {
        current_streak += 1;
        cursor -= Duration::days(1);
    }

    let mut sorted: Vec<NaiveDate> = dates.iter().copied().collect();
    sorted.sort();
    let mut longest_streak = 0;
    let mut run = 0;
    let mut previous: Option<NaiveDate> = None;
    for date in sorted {
        run = match previous {
            Some(prev) if date == prev + Duration::days(1) => run + 1,
            _ => 1,
        };
        longest_streak = longest_streak.max(run);
        previous = Some(date);
    }

    let (week_start, _) = week_range(today);
    let weekly_done = dates
        .iter()
        .filter(|date| **date >= week_start && **date <= today)
        .count() as u32;

    let window_start = today - Duration::days(29);
    let done = dates
        .iter()
        .filter(|date| **date >= window_start && **date <= today)
        .count() as u32;
    let expected = match frequency {
        HabitFrequency::Daily => 30,
        HabitFrequency::Weekdays => (0..30)
            .filter(|offset| {
                let date = window_start + Duration::days(*offset);
                date.weekday().num_days_from_monday() < 5
            })
            .count() as u32,
        HabitFrequency::Weekly { times } => ((times as u32) * 30).div_ceil(7),
    };
    let last_30_rate = if expected == 0 {
        0.0
    } else {
        (done as f64 / expected as f64).min(1.0)
    };

    HabitStats {
        current_streak,
        longest_streak,
        total: dates.len() as u32,
        weekly_done,
        weekly_target: match frequency {
            HabitFrequency::Weekly { times } => Some(times),
            _ => None,
        },
        last_30_rate,
        checked_today,
    }
}

/// 热力图网格：列优先，`grid[week][weekday]`，weekday 0 = 周一。
pub fn heatmap_grid(
    checkins: &[HabitCheckin],
    habit_id: &str,
    weeks: u32,
    today: NaiveDate,
) -> Vec<Vec<bool>> {
    let dates: HashSet<NaiveDate> = checkins
        .iter()
        .filter(|item| item.habit_id == habit_id && !item.deleted)
        .filter_map(|item| parse_date(&item.date))
        .collect();

    let (_, week_end) = week_range(today);
    let start = week_end - Duration::days(weeks as i64 * 7 - 1);
    let mut grid = vec![vec![false; 7]; weeks as usize];
    for (week, column) in grid.iter_mut().enumerate() {
        for (day, cell) in column.iter_mut().enumerate() {
            let date = start + Duration::days((week * 7 + day) as i64);
            *cell = dates.contains(&date);
        }
    }
    grid
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DayStat {
    pub total: u32,
    pub done: u32,
}

impl DayStat {
    pub fn rate(self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.done as f64 / self.total as f64
        }
    }
}

/// 某天所有待办/日程的完成情况。
pub fn block_day_stats(blocks: &[Block], date: &str) -> DayStat {
    let mut stat = DayStat::default();
    for block in blocks.iter().filter(|block| {
        !block.deleted
            && block.due_date.as_deref() == Some(date)
            && matches!(block.kind, BlockKind::Todo | BlockKind::CalendarEvent)
    }) {
        stat.total += 1;
        if block.checked {
            stat.done += 1;
        }
    }
    stat
}

/// 最近 `days` 天的完成序列，按时间升序。
pub fn completion_series(blocks: &[Block], days: u32, end: NaiveDate) -> Vec<(String, DayStat)> {
    (0..days)
        .map(|offset| {
            let date = end - Duration::days((days - 1 - offset) as i64);
            let key = date.format("%Y-%m-%d").to_string();
            (key.clone(), block_day_stats(blocks, &key))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::parse_date;
    use crate::model::HabitCheckin;
    use chrono::Utc;

    fn checkin(habit_id: &str, date: &str) -> HabitCheckin {
        HabitCheckin {
            id: format!("c-{habit_id}-{date}"),
            habit_id: habit_id.to_string(),
            date: date.to_string(),
            note: String::new(),
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            updated_by: "Guan".to_string(),
            deleted: false,
        }
    }

    fn block(date: &str, checked: bool) -> Block {
        Block {
            id: format!("b-{date}-{checked}"),
            page_id: "p".to_string(),
            kind: BlockKind::Todo,
            text: "task".to_string(),
            checked,
            due_date: Some(date.to_string()),
            streak: 0,
            position: 0,
            version: 1,
            updated_at: Utc::now(),
            updated_by: "Guan".to_string(),
            deleted: false,
            start_time: None,
            end_time: None,
            completed_at: None,
            completed_by: None,
            section: None,
            owner_id: String::new(),
        }
    }

    #[test]
    fn frequency_schedule() {
        let monday = parse_date("2026-11-16").unwrap();
        let saturday = parse_date("2026-11-21").unwrap();
        assert!(habit_due_on(HabitFrequency::Daily, saturday, 0));
        assert!(habit_due_on(HabitFrequency::Weekdays, monday, 0));
        assert!(!habit_due_on(HabitFrequency::Weekdays, saturday, 0));
        assert!(habit_due_on(HabitFrequency::Weekly { times: 3 }, monday, 2));
        assert!(!habit_due_on(
            HabitFrequency::Weekly { times: 3 },
            monday,
            3
        ));
    }

    #[test]
    fn computes_streaks_and_rates() {
        let today = parse_date("2026-11-16").unwrap();
        let checkins = vec![
            checkin("h1", "2026-11-16"),
            checkin("h1", "2026-11-15"),
            checkin("h1", "2026-11-14"),
            checkin("h1", "2026-11-10"),
            checkin("h2", "2026-11-16"),
        ];
        let stats = habit_stats(HabitFrequency::Daily, "h1", &checkins, today);
        assert_eq!(stats.current_streak, 3);
        assert_eq!(stats.longest_streak, 3);
        assert_eq!(stats.total, 4);
        assert_eq!(stats.weekly_done, 1);
        assert!(stats.checked_today);
        assert!(stats.last_30_rate > 0.0);

        let stats2 = habit_stats(HabitFrequency::Daily, "h2", &checkins, today);
        assert_eq!(stats2.current_streak, 1);
        assert_eq!(stats2.total, 1);
    }

    #[test]
    fn builds_heatmap_and_series() {
        let today = parse_date("2026-11-16").unwrap();
        let checkins = vec![checkin("h1", "2026-11-16")];
        let grid = heatmap_grid(&checkins, "h1", 2, today);
        assert_eq!(grid.len(), 2);
        assert_eq!(grid[0].len(), 7);
        // 2026-11-16 是本周一，位于最后一列第 0 行。
        assert!(grid[1][0]);

        let blocks = vec![block("2026-11-16", true), block("2026-11-16", false)];
        let stat = block_day_stats(&blocks, "2026-11-16");
        assert_eq!(stat, DayStat { total: 2, done: 1 });
        assert!((stat.rate() - 0.5).abs() < f64::EPSILON);

        let series = completion_series(&blocks, 3, today);
        assert_eq!(series.len(), 3);
        assert_eq!(series[2].0, "2026-11-16");
        assert_eq!(series[2].1.done, 1);
    }
}
