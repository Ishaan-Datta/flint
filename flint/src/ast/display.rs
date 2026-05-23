use std::collections::HashMap;

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
/// Returns `()` after emitting the summary to the log output.
pub(crate) fn print_duplicates_summary(
    input_deps: &HashMap<String, Vec<InputReplacement>>,
) {
    if input_deps.is_empty() {
        tracing::info!("> No duplicate dependencies found.");
        return;
    }

    tracing::info!("> Duplicate transitive dependencies found: ");

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

    tracing::info!("");

    for input in inputs {
        let header = input.bold();
        tracing::info!("{header}");

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

            tracing::info!("{line}");
        }

        tracing::info!("");
    }
}
