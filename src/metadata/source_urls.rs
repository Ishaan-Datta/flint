use crate::metadata::get_new_modified_time;

use indicatif::ProgressState;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use std::collections::HashMap;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use std::{cmp::min, fmt::Write};
use tracing;
use tracing::info_span;
use tracing_indicatif::span_ext::IndicatifSpanExt;

const URL_CMD: &str = r#"nix eval --json --impure --expr '  
builtins.mapAttrs (_: v: v.url or null) ((import ./flake.nix).inputs)  
'
"#;

/// Get the URL for each flake.nix input
pub fn get_input_urls() -> Result<HashMap<String, String>, anyhow::Error> {
    let url_output = Command::new("sh").args(["-c", URL_CMD]).output()?;
    if url_output.status.success() {
        let url_map: HashMap<String, String> = serde_json::from_slice(&url_output.stdout)?;
        // tracing::info!("Successfully fetched {} flake input urls", url_map.len());
        return Ok(url_map);
    } else {
        anyhow::bail!("Failed to get input urls: format error here...");
    }
}

pub fn get_modified_times(url_map: HashMap<String, String>) -> HashMap<String, Option<i64>> {
    let header_span = info_span!("modified_times");
    header_span.pb_set_style(
        &ProgressStyle::with_template("{spinner} {pos}/{len} [{wide_bar:.cyan/blue}] {msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    // header_span.enable_steady_tick(Duration::from_millis(50));
    header_span.pb_set_length(url_map.len() as u64);
    header_span.pb_set_message("Fetching remote modified times");
    // header_span.pb_set_finish_message("Fetched remote modified times");

    let header_enter = header_span.enter();

    let map: Vec<(String, String)> = url_map.clone().into_iter().collect();
    let modified_map: HashMap<String, Option<i64>> = map
        .par_iter()
        .map({
            let header_span = header_span.clone();

            move |(input, url)| {
                let item_span = info_span!(
                    parent: &header_span,
                    "input",
                    input = %input,
                    url = %url,
                );
                item_span.pb_set_style(&ProgressStyle::with_template("  {spinner} {msg}").unwrap());
                let ts = item_span.in_scope(|| {
                    item_span.pb_set_message(&format!("Fetching {url}"));
                    let ts = get_new_modified_time(url);
                    match ts {
                        Some(_) => {
                            item_span.pb_set_message(&format!("Fetched {url}"));
                            // sleep(Duration::from_millis(100));
                        }
                        None => {
                            item_span.pb_set_message(&format!("Failed {url}"));
                            // sleep(Duration::from_millis(100));
                        }
                    }
                    ts
                });

                header_span.pb_inc(1);

                (input.clone(), ts)
            }
        })
        .collect();

    drop(header_enter);
    drop(header_span);
    // bar.finish();
    // println!("{modified_map:#?}");
    modified_map
}

// should make a spinner on the top section, with logs streaming below it, can note the progress as wanring/yellow for something where you couldnt find the URL for the entry - red actually, that is error, yellow is need updates
