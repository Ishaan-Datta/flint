use std::time::Instant;

use chrono::Local;
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;
use yansi::Paint;

use crate::modified_time::InputStatus;

/// Format an age in seconds as "ddd d, hh h, mm m", clamping negative values to
/// zero.
///
/// # Arguments
///
/// * `secs` - The age in seconds to format.
///
/// # Returns
///
/// Returns the formatted age string using days, hours, and minutes.
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

/// Log a summary message with the local finish time and elapsed duration.
///
/// # Arguments
///
/// * `start` - The instant marking the start of the operation.
pub fn print_summary_message(start: Instant) {
  let now = Local::now();
  let t24 = now.format("%H:%M:%S").to_string();
  let duration_ms = start.elapsed().as_millis() % 1000;
  let duration_s = start.elapsed().as_secs();

  let summary =
    format!("Finished at {t24} after {duration_s}.{duration_ms:03}s")
      .bold()
      .to_string();
  tracing::info!("\n{summary}\n");
}

/// Paint informational text based on the input status.
///
/// # Arguments
///
/// * `status` - The status used to select the color.
/// * `s` - The informational text to paint.
///
/// # Returns
///
/// Returns the painted string for the status.
pub fn paint_info(status: &InputStatus, s: &str) -> String {
  match status {
    InputStatus::Fresh => s.to_string(),
    InputStatus::Stale => s.yellow().to_string(),
    InputStatus::Error(_) => s.red().to_string(),
  }
}

/// Build a formatted status line with wrapping and alignment.
///
/// # Arguments
///
/// * `status` - The status used for the prefix and info color.
/// * `input_name` - The name to display in the prefix.
/// * `name_width` - The column width reserved for the input name.
/// * `info` - Optional informational text to wrap under the prefix.
///
/// # Returns
///
/// Returns the formatted status line, which may span multiple lines.
pub fn format_status_line(
  status: &InputStatus,
  input_name: &str,
  name_width: usize,
  info: Option<String>,
) -> String {
  let prefix_plain =
    format!("[{}] {input_name:<name_width$} ", status.plain_char());

  let prefix_painted =
    format!("[{}] {input_name:<name_width$} ", status.painted_char());

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
