use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use totion_core::DayStat;

pub fn progress_bar(done: u32, total: u32, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if total == 0 {
        return "░".repeat(width);
    }
    let filled = ((done as f64 / total as f64) * width as f64).round() as usize;
    let filled = filled.min(width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

/// 热力图：7 行（周一..周日）× weeks 列，列优先数据。
pub fn heatmap_lines(grid: &[Vec<bool>]) -> Vec<Line<'static>> {
    let weeks = grid.len();
    if weeks == 0 {
        return Vec::new();
    }
    let mut lines = Vec::with_capacity(7);
    for day in 0..7 {
        let mut spans = Vec::with_capacity(weeks * 2);
        for column in grid.iter() {
            let on = column.get(day).copied().unwrap_or(false);
            spans.push(Span::styled(
                if on { "█" } else { "·" },
                Style::default().fg(if on { Color::Green } else { Color::DarkGray }),
            ));
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
    }
    lines
}

pub fn done_values(series: &[(String, DayStat)]) -> Vec<u64> {
    series.iter().map(|(_, stat)| stat.done as u64).collect()
}

pub fn rate_values(series: &[(String, DayStat)]) -> Vec<u64> {
    series
        .iter()
        .map(|(_, stat)| (stat.rate() * 100.0).round() as u64)
        .collect()
}
