use std::{collections::HashMap, process::exit};

use unicode_width::UnicodeWidthStr;
use yansi::Paint;

use crate::metadata::InputReplacement;

/// Print a formatted summary of duplicate flake input dependencies.
///
/// # Arguments
///
/// * `input_deps` - Map of input names to their replacement entries.
///
/// # Returns
///
/// Exits the process with status 0 when there are no duplicate input
/// dependencies.
pub(crate) fn print_duplicates_summary(
    input_deps: &HashMap<String, Vec<InputReplacement>>,
) {
    if input_deps.is_empty() {
        tracing::info!("No duplicate dependencies found. \n");
        exit(0);
    }

    let mut inputs: Vec<&String> = input_deps.keys().collect();
    inputs.sort_unstable();

    tracing::trace!("{input_deps:#?}");

    let input_dep_width = input_deps
        .values()
        .flat_map(|deps| deps.iter())
        .map(|dep| dep.input_dependency.width())
        .max()
        .unwrap_or(0)
        + 4;

    let mut sections = Vec::new();
    for input in inputs {
        let mut lines = Vec::new();

        let header = input.bold();
        lines.push(format!("{header}"));

        let mut replacements =
            input_deps.get(input).expect("Checked this").clone();
        replacements.sort();

        for replacement in replacements {
            let input_dependency_entry =
                format!("[ {} ]", replacement.input_dependency);

            let line = format!(
                "{input_dependency_entry:<input_dep_width$} -> [ {} ]",
                replacement.old_dependency_target.yellow()
            );

            lines.push(line);
        }

        sections.push(lines.join("\n"));
    }

    tracing::info!(
        "Duplicate transitive dependencies found:\n\n{}",
        sections.join("\n\n")
    );

    // NB: add for cmd demo
    // tracing::info!("");
}
