use crate::modified_time::InputStatus;

use chrono::Local;
use std::time::Instant;
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;
use yansi::Paint;

pub fn format_age(secs: i64) -> String {
    let mut s = Vec::new();
    let mut remaining = secs.max(0);

    let days = remaining / 86_400;
    remaining %= 86_400;
    let hours = remaining / 3_600;
    remaining %= 3_600;
    let mins = remaining / 60;

    s.push(format!("{days:>3} d"));
    s.push(format!("{hours:>2} h"));
    s.push(format!("{mins:>2} m"));
    s.join(", ")
}

pub fn print_summary_message(start: Instant) {
    let now = Local::now();
    let t24 = now.format("%H:%M:%S").to_string();
    let duration = start.elapsed().as_secs();
    let summary = format!("Finished at {t24} after {duration}s")
        .bold()
        .to_string();
    tracing::info!("{summary}\n");
}

pub fn paint_info(status: &InputStatus, s: &str) -> String {
    match status {
        InputStatus::Fresh => s.to_string(),
        InputStatus::Stale => s.yellow().to_string(),
        InputStatus::Error(_) => s.red().to_string(),
    }
}

pub fn format_status_line(
    status: &InputStatus,
    input_name: &str,
    name_width: usize,
    info: Option<String>,
) -> String {
    let prefix_plain = format!("[{}] {input_name:<name_width$} ", status.plain_char());

    let prefix_painted = format!("[{}] {input_name:<name_width$} ", status.painted_char());

    let Some(info) = info else {
        return prefix_painted.trim_end().to_string();
    };

    let indent_width = prefix_plain.width();
    let indent = " ".repeat(indent_width);

    let term_width = textwrap::termwidth();
    let info_width = term_width.saturating_sub(indent_width).max(20);

    let wrapped = wrap(&info, info_width);

    wrapped
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let painted_info = paint_info(status, line.as_ref());

            if i == 0 {
                format!("{prefix_painted}{painted_info}")
            } else {
                format!("{indent}{painted_info}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
