use chrono::{Datelike, Local, NaiveDate, Weekday};

/// 客户端本地日期键 YYYY-MM-DD。
pub fn today_key() -> String {
    Local::now().date_naive().format("%Y-%m-%d").to_string()
}

pub fn local_today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

pub fn valid_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10 && bytes[4] == b'-' && bytes[7] == b'-' && parse_date(value).is_some()
}

pub fn valid_time(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 5 || bytes[2] != b':' {
        return false;
    }
    if !bytes[..2].iter().all(u8::is_ascii_digit) || !bytes[3..].iter().all(u8::is_ascii_digit) {
        return false;
    }
    let hour: u32 = value[..2].parse().unwrap_or(99);
    let minute: u32 = value[3..].parse().unwrap_or(99);
    hour < 24 && minute < 60
}

pub fn weekday(date: NaiveDate) -> Weekday {
    date.weekday()
}

/// 返回本周（周一开始）的日期范围。
pub fn week_range(date: NaiveDate) -> (NaiveDate, NaiveDate) {
    let start = date - chrono::Duration::days(date.weekday().num_days_from_monday() as i64);
    (start, start + chrono::Duration::days(6))
}

/// 把 "YYYY-MM-DD" 加 n 天，解析失败返回 None。
pub fn add_days(value: &str, days: i64) -> Option<String> {
    parse_date(value).map(|date| {
        (date + chrono::Duration::days(days))
            .format("%Y-%m-%d")
            .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_dates_and_times() {
        assert!(valid_date("2026-11-16"));
        assert!(!valid_date("2026-11-31"));
        assert!(!valid_date("16/11/2026"));
        assert!(valid_time("09:05"));
        assert!(!valid_time("9:05"));
        assert!(!valid_time("24:00"));
    }

    #[test]
    fn computes_week_range() {
        let (start, end) = week_range(parse_date("2026-11-16").unwrap());
        assert_eq!(start.format("%Y-%m-%d").to_string(), "2026-11-16");
        assert_eq!(end.format("%Y-%m-%d").to_string(), "2026-11-22");
        assert_eq!(add_days("2026-11-16", 1).as_deref(), Some("2026-11-17"));
    }
}
