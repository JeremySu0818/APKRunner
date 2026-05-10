use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{ApkRunnerError, ApkRunnerResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub status_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl CommandOutput {
    pub fn success(stdout: impl Into<Vec<u8>>) -> Self {
        Self {
            status_code: Some(0),
            stdout: stdout.into(),
            stderr: Vec::new(),
        }
    }

    pub fn is_success(&self) -> bool {
        self.status_code == Some(0)
    }
}

pub trait ManagedChild: Send {
    fn kill(&mut self) -> ApkRunnerResult<()>;
    fn try_wait(&mut self) -> ApkRunnerResult<Option<i32>>;
}

pub trait HostCommandRunner: Send {
    fn run(
        &mut self,
        program: &Path,
        args: &[String],
        env: &[(&str, &str)],
        timeout: Duration,
    ) -> ApkRunnerResult<CommandOutput>;

    fn spawn(
        &mut self,
        program: &Path,
        args: &[String],
        env: &[(&str, &str)],
    ) -> ApkRunnerResult<Box<dyn ManagedChild>>;
}

pub trait ArtifactDownloader: Send {
    fn download(&mut self, url: &str, destination: &Path) -> ApkRunnerResult<()>;
}

#[derive(Debug, Default)]
pub struct SystemHostCommandRunner;

impl SystemHostCommandRunner {
    fn command(program: &Path, args: &[String], env: &[(&str, &str)]) -> Command {
        let mut command = Command::new(program);
        command.args(args);
        for (key, value) in env {
            command.env(key, value);
        }
        command
    }
}

impl HostCommandRunner for SystemHostCommandRunner {
    fn run(
        &mut self,
        program: &Path,
        args: &[String],
        env: &[(&str, &str)],
        timeout: Duration,
    ) -> ApkRunnerResult<CommandOutput> {
        let mut child = Self::command(program, args, env)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| ApkRunnerError::HostIoFailure {
                path: program.to_path_buf(),
                reason: source.to_string(),
            })?;
        let started = Instant::now();

        loop {
            if child
                .try_wait()
                .map_err(|source| ApkRunnerError::RuntimeBackendError(source.to_string()))?
                .is_some()
            {
                let output =
                    child
                        .wait_with_output()
                        .map_err(|source| ApkRunnerError::HostIoFailure {
                            path: program.to_path_buf(),
                            reason: source.to_string(),
                        })?;
                return Ok(CommandOutput {
                    status_code: output.status.code(),
                    stdout: output.stdout,
                    stderr: output.stderr,
                });
            }

            if started.elapsed() >= timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ApkRunnerError::RuntimeBackendError(format!(
                    "command timed out after {}ms: {}",
                    timeout.as_millis(),
                    program.display()
                )));
            }

            thread::sleep(Duration::from_millis(10));
        }
    }

    fn spawn(
        &mut self,
        program: &Path,
        args: &[String],
        env: &[(&str, &str)],
    ) -> ApkRunnerResult<Box<dyn ManagedChild>> {
        let child = Self::command(program, args, env)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|source| ApkRunnerError::HostIoFailure {
                path: program.to_path_buf(),
                reason: source.to_string(),
            })?;
        Ok(Box::new(SystemManagedChild { child }))
    }
}

pub struct SystemManagedChild {
    child: Child,
}

impl ManagedChild for SystemManagedChild {
    fn kill(&mut self) -> ApkRunnerResult<()> {
        match self.child.kill() {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == std::io::ErrorKind::InvalidInput => Ok(()),
            Err(source) => Err(ApkRunnerError::RuntimeBackendError(source.to_string())),
        }
    }

    fn try_wait(&mut self) -> ApkRunnerResult<Option<i32>> {
        self.child
            .try_wait()
            .map(|status| status.map(|status| status.code().unwrap_or(-1)))
            .map_err(|source| ApkRunnerError::RuntimeBackendError(source.to_string()))
    }
}

#[derive(Debug, Default)]
pub struct NoopArtifactDownloader;

impl ArtifactDownloader for NoopArtifactDownloader {
    fn download(&mut self, _url: &str, _destination: &Path) -> ApkRunnerResult<()> {
        Err(ApkRunnerError::RuntimeBackendError(
            "command-line tools download is not configured; provide a packaged cmdline-tools bundle"
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn command_timeout_returns_runtime_error() {
        let mut runner = SystemHostCommandRunner;
        let args = vec!["2".to_string()];
        let error = runner
            .run(
                Path::new("/bin/sleep"),
                &args,
                &[],
                Duration::from_millis(10),
            )
            .expect_err("sleep should time out");
        assert!(matches!(error, ApkRunnerError::RuntimeBackendError(_)));
    }
}
