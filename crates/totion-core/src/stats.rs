use chrono::{Datelike, NaiveDate};

use crate::model::{HabitCheckin, HabitFrequency};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::parse_date;

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
}
