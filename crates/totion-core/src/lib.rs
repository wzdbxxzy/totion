mod date;
mod model;
mod protocol;
mod stats;

pub use date::*;
pub use model::*;
pub use protocol::*;
pub use stats::*;

/// 旧版习惯块（BlockKind::Habit）的打卡规则。
///
/// 新版独立习惯使用 `Habit` + `HabitCheckin`，不再依赖这个函数。
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
