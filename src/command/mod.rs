use crate::errors::CommandError;

use anyhow::anyhow;
use std::io::Read;
use std::process::ChildStderr;
use std::process::ChildStdout;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;
use std::time::Duration;
use wait_timeout::ChildExt;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CommandResult {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl CommandResult {
    fn new(
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

pub fn run_command_with_timeout(
    cmd: String,
    timeout: Duration,
) -> Result<CommandResult, CommandError> {
    let mut child = Command::new("sh")
        .args(["-c", &cmd])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .or_else(|_| anyhow::bail!("Command: {cmd} failed to start"))?;
    let stdout = child.stdout.take().ok_or(anyhow!(
        "Could not make stdout handle for command sub-process"
    ))?;
    let stderr = child.stderr.take().ok_or(anyhow!(
        "Could not make stderr handle for command sub-process"
    ))?;

    match child
        .wait_timeout(timeout)
        .or_else(|_| anyhow::bail!("Could not setup child timeout for command: {cmd}"))?
    {
        Some(status) => Ok(CommandResult::new(stdout, stderr, status)?),
        None => {
            child
                .kill()
                .or_else(|_| anyhow::bail!("Command: {cmd} could not be killed after timeout"))?;
            Err(CommandError::CommandTimeout(timeout.as_millis()))
        }
    }
}
