use std::{
    collections::HashMap,
    fs,
    path::Path,
    process::exit,
    time::Duration,
};

use tempfile::tempdir;
use tree_sitter::Parser;

use crate::{
    ast::{
        display::print_duplicates_summary,
        treesitter::{
            TextEdit,
            apply_edit,
            child_by_field_name_or_last_named,
            filter_missing_insertions,
            find_attrset_binding_by_name,
            find_flat_attrpath_binding,
            find_top_level_inputs_binding,
            insert_into_existing_attrset,
            line_start_byte_at,
            rewrite_flat_url_binding_to_attrset,
        },
    },
    command::{run_command_with_timeout, with_command_spinner},
    errors::{CommandError, WriteError, treesitter::TreesitterParseError},
    metadata::get_input_deps,
    modified_time::print_summary_message,
};

const VALIDATE_FILE_NIX_CMD: &str =
    r"nix flake metadata --no-write-lock-file {PATH}";
const CHECK_GIT_REPO_CMD: &str = r"git -C {DIR_PATH} rev-parse --show-toplevel";
const CHECK_UNSTAGED_CHANGES_CMD: &str =
    r"git -C {DIR_PATH} diff --quiet -- {FILE_NAME}";

/// Analyze flake inputs, report duplicates, and optionally rewrite the file.
///
/// # Arguments
///
/// * `fix` - If true, apply edits to the flake file.
/// * `quiet` - If true, suppress prompts and exit when duplicates are found.
/// * `timeout` - Timeout for external commands.
/// * `override_bool` - If true, skip the git dirty check when writing.
/// * `backup` - If true, save `flake.nix` as `flake.nix.bak` before writing.
/// * `flake_dir_path` - Directory that contains the target `flake.nix`.
///
/// # Returns
///
/// Returns `()` after printing a summary and optionally writing a new file.
///
/// # Errors
///
/// This function does not return errors; it logs failures and exits the
/// process with a non-zero status.
pub fn rewrite_flake_inputs(
    fix: bool,
    quiet: bool,
    timeout: Duration,
    override_bool: bool,
    backup: bool,
    flake_dir_path: &Path,
) {
    let start_time = std::time::Instant::now();

    let flake_file_path = &flake_dir_path.join("flake.nix");
    let flake_content =
        fs::read_to_string(flake_file_path).unwrap_or_else(|e| {
            tracing::error!(
                "Failed to read the flake: {flake_file_path:?}: {e}"
            );
            exit(1);
        });

    let input_deps =
        get_input_deps(flake_dir_path, timeout).unwrap_or_else(|e| {
            tracing::error!(
                "Failed to parse input dependencies of the flake: {e}"
            );
            exit(1);
        });

    print_summary_message(start_time);

    if quiet && !input_deps.is_empty() {
        exit(1);
    } else {
        print_duplicates_summary(&input_deps);
    }

    if fix {
        let converted: HashMap<String, Vec<String>> = input_deps
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().map(String::from).collect()))
            .collect();

        let new_flake_content =
            apply_flake_input_edits(&flake_content, &converted).unwrap_or_else(
                |e| {
                    tracing::error!(
                        "Failed to manipulate flake AST using Treesitter: {e}"
                    );
                    exit(1);
                },
            );

        write_new_flake_file(
            flake_dir_path,
            &new_flake_content,
            override_bool,
            quiet,
            backup,
            timeout,
        )
        .unwrap_or_else(|e| {
            tracing::error!("\nFailed to perform flake file operations: {e}");
            exit(1);
        });
    }
}

/// Write a validated flake to disk, optionally backing up and checking for
/// local edits.
///
/// # Arguments
///
/// * `new_flake_dir_path` - Directory that contains the target `flake.nix`.
/// * `new_flake_content` - Full contents for the new flake file.
/// * `override_bool` - If true, skip the git dirty check.
/// * `quiet` - If true, avoid prompts and exit on dirty files.
/// * `backup` - If true, rename the original flake to `flake.nix.bak`.
/// * `timeout` - Timeout for external commands.
///
/// # Returns
///
/// Returns `Ok(())` after the new flake has been validated and written.
///
/// # Errors
///
/// Returns an error on IO failures, validation command failures, dirty-file
/// checks, or user aborts.
pub fn write_new_flake_file(
    new_flake_dir_path: &Path,
    new_flake_content: &str,
    override_bool: bool,
    quiet: bool,
    backup: bool,
    timeout: Duration,
) -> Result<(), WriteError> {
    let temp_flake_dir = tempdir()?;
    let temp_flake_path = temp_flake_dir.path().join("flake.nix");
    std::fs::write(&temp_flake_path, new_flake_content)?;

    let cmd = VALIDATE_FILE_NIX_CMD
        .replace("{PATH}", &temp_flake_path.display().to_string());
    let output = with_command_spinner!(
        "Validating the flake file edits",
        &cmd,
        timeout
    )?;

    if !output.status.success() {
        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
        let code = output.status.code().unwrap_or(1);
        return Err(WriteError::CommandError(CommandError::NonZeroExitCode(
            code, stderr_str, stdout_str,
        )));
    }

    tracing::info!("Flake edits were successfully validated");

    let original_flake_path = new_flake_dir_path.join("flake.nix");

    if backup {
        let old_flake_backup_path = new_flake_dir_path.join("flake.nix.bak");
        fs::copy(&original_flake_path, &old_flake_backup_path)?;
        tracing::info!(
            "\nMade backup of original flake at: {}",
            old_flake_backup_path.display()
        );
    }

    tracing::info!("");

    if !override_bool {
        let status =
            run_git_file_status(&original_flake_path, "flake.nix", timeout)?;
        handle_dirty_file_status(status, &original_flake_path, quiet)?;
    }

    // Copy the new flake to the current directory so the temp file and
    // destination are on the same mount, and final replacement can be an
    // atomic rename
    let new_temp_flake_path = new_flake_dir_path.join("temp.nix");
    fs::copy(&temp_flake_path, &new_temp_flake_path)?;
    fs::rename(&new_temp_flake_path, &original_flake_path)?;
    tracing::info!(
        "Successfully wrote new flake to path: {}\n",
        original_flake_path.display()
    );
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitFileStatus {
    NotRepo,
    Clean,
    Dirty,
}

/// Run git commands to determine if the target file has unstaged changes.
///
/// # Arguments
///
/// * `file_path` - Path to the file to inspect.
/// * `file_name` - File name used in the git diff check.
/// * `timeout` - Timeout for git commands.
///
/// # Returns
///
/// Returns `Ok(GitFileStatus::Clean)` when the file is clean, `Dirty` when
/// it has unstaged changes, and `NotRepo` when the file is not in a git repo.
///
/// # Errors
///
/// Returns an error if git commands fail.
///
/// # Panics
///
/// Panics if `file_path` has no parent directory.
pub fn run_git_file_status(
    file_path: &Path,
    file_name: &str,
    timeout: Duration,
) -> Result<GitFileStatus, WriteError> {
    let cmd = CHECK_GIT_REPO_CMD.replace(
        "{DIR_PATH}",
        &file_path
            .parent()
            .expect("Should have parent")
            .display()
            .to_string(),
    );
    let output = run_command_with_timeout(&cmd, timeout)?;

    if output.status.success() {
        tracing::debug!("Detected file: {file_name} is in a git repository");
        let cmd = CHECK_UNSTAGED_CHANGES_CMD
            .replace(
                "{DIR_PATH}",
                &file_path
                    .parent()
                    .expect("Should have parent")
                    .display()
                    .to_string(),
            )
            .replace("{FILE_NAME}", file_name);
        let output = with_command_spinner!(
            "Checking if the existing file: {file_name} has unstaged changes",
            &cmd,
            timeout
        )?;
        match output.status.code() {
            Some(0) => {
                tracing::debug!("Detected file does not have unstaged changes");
                return Ok(GitFileStatus::Clean);
            },
            Some(1) => {
                tracing::debug!("Detected file has unstaged changes");
                return Ok(GitFileStatus::Dirty);
            },
            _ => {
                let stdout_str =
                    String::from_utf8_lossy(&output.stdout).to_string();
                let stderr_str =
                    String::from_utf8_lossy(&output.stderr).to_string();
                let code = output.status.code().unwrap_or(1);
                Err(WriteError::CommandError(CommandError::NonZeroExitCode(
                    code, stderr_str, stdout_str,
                )))?
            },
        };
    }

    tracing::debug!("Did not detect file: {file_name} is in a git repository");
    Ok(GitFileStatus::NotRepo)
}

/// Handle a dirty file status by optionally prompting or aborting.
///
/// # Arguments
///
/// * `status` - Result of the git status checks for the target file.
/// * `flake_path` - Path to the `flake.nix` file to inspect.
/// * `quiet` - If true, do not prompt; exit on dirty files.
///
/// # Returns
///
/// Returns `Ok(())` when the file is clean or the user confirms overwriting.
///
/// # Errors
///
/// Returns an error if the user rejects overwriting.
pub(crate) fn handle_dirty_file_status(
    status: GitFileStatus,
    flake_path: &Path,
    quiet: bool,
) -> Result<(), WriteError> {
    if status != GitFileStatus::Dirty {
        return Ok(());
    }

    // Exit if user input is required but quiet mode is enabled
    if quiet {
        exit(1);
    }

    let confirmation = inquire::Confirm::new(&format!(
        "The file at path: {} has changes that are not commited in git, \
         override the changes?",
        flake_path.display()
    ))
    .with_default(false)
    .prompt()?;

    if !confirmation {
        return Err(WriteError::AbortUserInput);
    }

    tracing::info!("");
    Ok(())
}

/// Apply input dependency edits to a flake file using a Treesitter AST.
///
/// # Arguments
///
/// * `flake_file_content` - Full contents of the flake file to edit.
/// * `input_dep_edits` - Map of input name to dependency lines to insert.
///
/// # Returns
///
/// Returns the updated flake file contents after applying all edits.
///
/// # Errors
///
/// Returns a parse error if the flake cannot be parsed, the inputs attribute
/// is missing or malformed, or incremental reparse fails.
pub fn apply_flake_input_edits(
    flake_file_content: &str,
    input_dep_edits: &HashMap<String, Vec<String>>,
) -> std::result::Result<String, TreesitterParseError> {
    tracing::trace!("{input_dep_edits:#?}");

    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_nix::LANGUAGE.into())
        .map_err(|_| TreesitterParseError::LanguageLoad)?;

    let mut tree = parser
        .parse(flake_file_content, None)
        .ok_or(TreesitterParseError::ParseFailed)?;
    let root = tree.root_node();
    if root.has_error() {
        return Err(TreesitterParseError::SyntaxError);
    }

    let inputs_binding =
        find_top_level_inputs_binding(root, flake_file_content)
            .ok_or(TreesitterParseError::MissingTopLevelInputs)?;

    let inputs_rhs = child_by_field_name_or_last_named(inputs_binding, "value")
        .ok_or(TreesitterParseError::InputsMissingRhs)?;

    if inputs_rhs.kind() != "attrset_expression"
        && inputs_rhs.kind() != "attr_set"
    {
        return Err(TreesitterParseError::InputsNotAttrset(
            inputs_rhs.kind().to_string(),
        ));
    }

    // Collect edits first, apply from back to front.
    let mut edits = Vec::<TextEdit>::new();

    for (input_name, lines) in input_dep_edits {
        if let Some(binding) = find_attrset_binding_by_name(
            inputs_rhs,
            flake_file_content,
            input_name,
        ) {
            let rhs = child_by_field_name_or_last_named(binding, "value")
                .ok_or_else(|| {
                    TreesitterParseError::BindingMissingRhs(input_name.clone())
                })?;

            let already_nested =
                rhs.kind() == "attrset_expression" || rhs.kind() == "attr_set";

            if already_nested {
                let missing =
                    filter_missing_insertions(rhs, flake_file_content, lines);
                if missing.is_empty() {
                    continue;
                }

                let edit = insert_into_existing_attrset(
                    rhs,
                    flake_file_content,
                    &missing,
                )
                .map_err(|_| {
                    TreesitterParseError::AttrsetMissingClosingBrace
                })?;
                edits.push(edit);
            }
        } else if let Some(flat_binding) = find_flat_attrpath_binding(
            inputs_rhs,
            flake_file_content,
            input_name,
            "url",
        ) {
            let replacement = rewrite_flat_url_binding_to_attrset(
                flat_binding,
                flake_file_content,
                input_name,
                lines,
            )
            .map_err(|_| TreesitterParseError::FlatBindingMissingRhs)?;

            edits.push(TextEdit {
                start_byte:   line_start_byte_at(
                    flake_file_content,
                    flat_binding.start_byte(),
                ),
                old_end_byte: flat_binding.end_byte(),
                new_text:     replacement,
            });
        }
    }

    edits.sort_by_key(|e| e.start_byte);
    edits.reverse();

    let mut out = flake_file_content.to_string();
    for e in edits {
        apply_edit(&mut out, &mut tree, &mut parser, &e)
            .map_err(|_| TreesitterParseError::IncrementalReparseFailed)?;
    }

    Ok(out)
}
