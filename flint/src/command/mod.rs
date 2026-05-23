use std::{
    io::Read,
    process::{ChildStderr, ChildStdout, Command, ExitStatus, Stdio},
    time::Duration,
};

use anyhow::anyhow;
use wait_timeout::ChildExt;

use crate::errors::CommandError;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct CommandResult {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl CommandResult {
    /// Collect stdout and stderr from a completed child process.
    ///
    /// # Arguments
    ///
    /// * `stdout` - The child stdout handle to read.
    /// * `stderr` - The child stderr handle to read.
    /// * `status` - The exit status returned by the child.
    ///
    /// # Returns
    ///
    /// Returns a `CommandResult` containing the exit status and captured
    /// output.
    ///
    /// # Errors
    ///
    /// Returns an error if reading stdout or stderr fails.
    pub(crate) fn new(
        mut stdout: ChildStdout,
        mut stderr: ChildStderr,
        status: ExitStatus,
    ) -> Result<Self, anyhow::Error> {
        let mut stdout_buf = Vec::<u8>::new();
        let mut stderr_buf = Vec::<u8>::new();

        stdout.read_to_end(&mut stdout_buf)?;
        stderr.read_to_end(&mut stderr_buf)?;

        Ok(Self {
            stdout: stdout_buf,
            stderr: stderr_buf,
            status,
        })
    }
}

/// Run a shell command with a timeout and capture its output.
///
/// # Arguments
///
/// * `cmd` - The shell command to execute.
/// * `timeout` - The maximum duration to wait for the command to finish.
///
/// # Returns
///
/// Returns a `CommandResult` with the exit status and captured output when the
/// command completes before the timeout.
///
/// # Errors
///
/// Returns a `CommandError` if the command fails to start, stdout or stderr
/// pipes cannot be captured, output collection fails, the timeout could not be
/// configured, or the command exceeds the timeout.
pub(crate) fn run_command_with_timeout(
    cmd: &str,
    timeout: Duration,
) -> Result<CommandResult, CommandError> {
    tracing::trace!("Running command: {cmd} with timeout: {timeout:?}");

    let mut child = Command::new("sh")
        .args(["-c", cmd])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .or_else(|_| anyhow::bail!("Command: {cmd} failed to start"))?;

    let stdout = child.stdout.take().ok_or_else(|| {
        anyhow!("Could not make stdout handle for command sub-process")
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        anyhow!("Could not make stderr handle for command sub-process")
    })?;

    if let Some(status) = child.wait_timeout(timeout).or_else(|_| {
        anyhow::bail!("Could not setup child timeout for command: {cmd}")
    })? {
        Ok(CommandResult::new(stdout, stderr, status)?)
    } else {
        child.kill().or_else(|_| {
            anyhow::bail!("Command: {cmd} could not be killed after timeout")
        })?;
        Err(CommandError::CommandTimeout(timeout.as_millis()))
    }
}

/// Run a command with a spinner and a timeout, returning its result.
///
/// # Arguments
///
/// * `$progress_msg` - The message to display next to the spinner.
/// * `$cmd` - The shell command to execute.
/// * `$timeout` - The maximum duration to wait for the command to finish.
///
/// # Returns
///
/// Returns a `CommandResult` with the exit status and captured output when the
/// command completes before the timeout.
///
/// # Errors
///
/// Returns a `CommandError` if the command fails to start, if the timeout
/// could not be configured, or if the command exceeds the timeout.
///
/// # Panics
///
/// Panics if the spinner progress template is invalid.
macro_rules! with_command_spinner {
    ($progress_msg:expr, $cmd:expr, $timeout:expr $(,)?) => {{
        use indicatif::ProgressStyle;
        use tracing_indicatif::span_ext::IndicatifSpanExt;

        use crate::command::run_command_with_timeout;

        let header_span = tracing::info_span!("run_command");

        header_span.pb_set_style(
            &ProgressStyle::with_template("{spinner} {msg}")
                .expect("valid progress template")
                .tick_chars("⠋⠙⠹⠸⢰⣠⣄⡆⡇⡏⠏ "),
        );

        header_span.pb_set_message($progress_msg);

        let header_enter = header_span.enter();

        let result = run_command_with_timeout($cmd, $timeout);

        drop(header_enter);
        drop(header_span);

        result
    }};
}

pub(crate) use with_command_spinner;
