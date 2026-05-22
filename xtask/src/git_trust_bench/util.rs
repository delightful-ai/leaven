use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

use super::Result;

pub(super) fn git<const N: usize>(cwd: &Path, args: [&str; N]) -> GitCommand {
    let mut command = Command::new("git");
    command.current_dir(cwd).args(args);
    GitCommand(command)
}

pub(super) fn git_no_cwd<const N: usize>(args: [&str; N]) -> GitCommand {
    let mut command = Command::new("git");
    command.args(args);
    GitCommand(command)
}

pub(super) fn git_output<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<String> {
    let output = git(cwd, args).output_checked()?;
    Ok(String::from_utf8(output.stdout)?)
}

pub(super) fn git_success<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<bool> {
    Ok(git(cwd, args).output()?.status.success())
}

pub(super) struct GitCommand(Command);

impl GitCommand {
    pub(super) fn arg<T: AsRef<OsStr>>(&mut self, arg: T) -> &mut Self {
        self.0.arg(arg);
        self
    }

    pub(super) fn status_checked(&mut self) -> Result<()> {
        let output = self.output_checked()?;
        drop(output);
        Ok(())
    }

    pub(super) fn output(&mut self) -> Result<Output> {
        Ok(self.with_envs().output()?)
    }

    pub(super) fn output_checked(&mut self) -> Result<Output> {
        let output = self.output()?;
        if !output.status.success() {
            return Err(format!(
                "command {:?} failed: {}",
                self.0,
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        Ok(output)
    }

    fn with_envs(&mut self) -> &mut Command {
        self.0
            .env("GIT_AUTHOR_NAME", "Leaven Benchmark")
            .env("GIT_AUTHOR_EMAIL", "leaven@example.invalid")
            .env("GIT_COMMITTER_NAME", "Leaven Benchmark")
            .env("GIT_COMMITTER_EMAIL", "leaven@example.invalid")
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
    }
}

pub(super) fn run_command(command: &mut Command, verbose: bool) -> Result<()> {
    if verbose {
        println!("running: {command:?}");
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "command {command:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}

pub(super) fn du_kib(path: &Path) -> Result<u64> {
    let output = Command::new("du").arg("-sk").arg(path).output()?;
    if !output.status.success() {
        return Err(format!("du failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    let stdout = String::from_utf8(output.stdout)?;
    let first = stdout
        .split_whitespace()
        .next()
        .ok_or("du output was empty")?;
    Ok(first.parse()?)
}

pub(super) fn repo_root() -> Result<PathBuf> {
    for mut command in [
        {
            let mut command = Command::new("jj");
            command.arg("root");
            command
        },
        {
            let mut command = Command::new("git");
            command.args(["rev-parse", "--show-toplevel"]);
            command
        },
    ] {
        let Ok(output) = command.output() else {
            continue;
        };
        if output.status.success() {
            return Ok(PathBuf::from(String::from_utf8(output.stdout)?.trim()));
        }
    }
    Err("could not determine repository root with jj or git".into())
}

pub(super) fn logical_cpus() -> usize {
    thread::available_parallelism().map_or(1, usize::from)
}

pub(super) fn half_parallelism() -> usize {
    (logical_cpus() / 2).max(1)
}

pub(super) fn seconds(duration: Duration) -> f64 {
    duration.as_secs_f64()
}

pub(super) fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let mut count = 0.0;
    let mut sum = 0.0;
    for value in values {
        count += 1.0;
        sum += value;
    }
    if count == 0.0 { 0.0 } else { sum / count }
}

pub(super) fn usize_to_f64(value: usize) -> f64 {
    value
        .to_string()
        .parse()
        .expect("usize decimal representation parses as f64")
}

pub(super) fn u64_to_f64(value: u64) -> f64 {
    value
        .to_string()
        .parse()
        .expect("u64 decimal representation parses as f64")
}
