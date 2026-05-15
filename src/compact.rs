use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::backup_store::{self, BackupTrigger};
use crate::path_map::path_buf_for_current_os;
use crate::profile::CodexProfile;
use crate::rollout::read_rollout_meta;
use crate::safety;
use crate::settings;

const COMPACT_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactOptions {
    pub apply: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactReport {
    pub action: String,
    pub applied: bool,
    pub session_id: String,
    pub backup_manifest: String,
    pub command: Vec<String>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CompactReport {
    pub fn to_text(&self) -> String {
        let mut lines = vec![
            format!("action: {}", self.action),
            format!("mode: {}", if self.applied { "applied" } else { "dry-run" }),
            format!("session id: {}", self.session_id),
            format!("command: {}", self.command.join(" ")),
        ];
        if !self.backup_manifest.is_empty() {
            lines.push(format!("backup manifest: {}", self.backup_manifest));
        }
        if let Some(exit_code) = self.exit_code {
            lines.push(format!("exit code: {exit_code}"));
        }
        lines.join("\n")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexCliInvocation {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub current_dir: Option<String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexCliOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

pub fn compact_session(
    profile: &CodexProfile,
    session_id: &str,
    options: &CompactOptions,
) -> Result<CompactReport> {
    compact_session_with_guard_and_runner(
        profile,
        session_id,
        options,
        safety::ensure_codex_not_running,
        run_codex_cli,
    )
}

pub fn compact_session_with_guard_and_runner<G, R>(
    profile: &CodexProfile,
    session_id: &str,
    options: &CompactOptions,
    guard: G,
    runner: R,
) -> Result<CompactReport>
where
    G: FnOnce() -> Result<()>,
    R: FnOnce(&CodexCliInvocation) -> Result<CodexCliOutput>,
{
    let configured = settings::load_settings(profile)?
        .codex_cli
        .command_path
        .unwrap_or_default();
    let mut invocation = compact_invocation(
        session_id,
        &resolve_codex_command(Some(configured.as_str()))?,
    );
    let mut report = CompactReport {
        action: "compact session context".to_string(),
        applied: options.apply,
        session_id: session_id.to_string(),
        backup_manifest: String::new(),
        command: std::iter::once(invocation.program.clone())
            .chain(invocation.args.clone())
            .collect(),
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
    };

    if !options.apply {
        return Ok(report);
    }

    guard()?;
    let local_session_path = backup_store::locate_unique_local_session(profile, session_id)?
        .with_context(|| format!("cannot compact {session_id}: local JSONL file was not found"))?;
    if let Some(current_dir) = compact_current_dir(&local_session_path)? {
        set_compact_current_dir(&mut invocation, current_dir);
        report.command = std::iter::once(invocation.program.clone())
            .chain(invocation.args.clone())
            .collect();
    }

    compact_after_preflight(profile, session_id, invocation, runner, report)
}

fn compact_after_preflight<R>(
    profile: &CodexProfile,
    session_id: &str,
    invocation: CodexCliInvocation,
    runner: R,
    mut report: CompactReport,
) -> Result<CompactReport>
where
    R: FnOnce(&CodexCliInvocation) -> Result<CodexCliOutput>,
{
    let manifest =
        backup_store::create_session_backup(profile, session_id, BackupTrigger::Compact)?;
    report.backup_manifest = backup_manifest_path(&manifest)?;

    let output = runner(&invocation)?;
    report.exit_code = output.exit_code;
    report.stdout = output.stdout;
    report.stderr = output.stderr;

    if output.timed_out {
        bail!(
            "codex compact command timed out after {} seconds; possible interactive prompt, update notice, login prompt, or network hang.\nstdout:\n{}\nstderr:\n{}",
            invocation.timeout.as_secs(),
            report.stdout,
            report.stderr
        );
    }

    if report.exit_code != Some(0) {
        bail!(
            "codex compact command failed with exit code {:?}.\nstdout:\n{}\nstderr:\n{}",
            report.exit_code,
            report.stdout,
            report.stderr
        );
    }

    Ok(report)
}

fn compact_invocation(session_id: &str, program: &str) -> CodexCliInvocation {
    CodexCliInvocation {
        program: program.to_string(),
        args: vec![
            "exec".to_string(),
            "resume".to_string(),
            "--skip-git-repo-check".to_string(),
            session_id.to_string(),
            "/compact".to_string(),
        ],
        env: vec![
            ("CI".to_string(), "1".to_string()),
            ("NO_COLOR".to_string(), "1".to_string()),
            ("CODEX_DISABLE_UPDATE_CHECK".to_string(), "1".to_string()),
            (
                "OPENAI_CODEX_DISABLE_UPDATE_CHECK".to_string(),
                "1".to_string(),
            ),
        ],
        timeout: COMPACT_TIMEOUT,
        current_dir: None,
    }
}

fn compact_current_dir(local_session_path: &std::path::Path) -> Result<Option<String>> {
    let Some(meta) = read_rollout_meta(local_session_path)? else {
        return Ok(None);
    };
    let Some(cwd) = meta
        .cwd
        .as_deref()
        .map(str::trim)
        .filter(|cwd| !cwd.is_empty())
    else {
        return Ok(None);
    };
    let path = path_buf_for_current_os(cwd);
    if path.is_dir() {
        Ok(Some(path.display().to_string()))
    } else {
        Ok(None)
    }
}

fn set_compact_current_dir(invocation: &mut CodexCliInvocation, current_dir: String) {
    invocation.current_dir = Some(current_dir.clone());
    if invocation.args.first().is_some_and(|arg| arg == "exec")
        && !invocation
            .args
            .iter()
            .any(|arg| arg == "-C" || arg == "--cd")
    {
        invocation
            .args
            .splice(1..1, ["-C".to_string(), current_dir]);
    }
}

pub fn resolve_codex_command(configured: Option<&str>) -> Result<String> {
    let configured = configured.and_then(non_empty_trimmed);
    if let Some(path) = configured {
        return Ok(path.to_string());
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = Command::new("where.exe").arg("codex").output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return resolve_codex_command_from_where_output(None, &stdout);
            }
        }
    }

    Ok("codex".to_string())
}

pub fn resolve_codex_command_from_where_output(
    configured: Option<&str>,
    stdout: &str,
) -> Result<String> {
    if let Some(path) = configured.and_then(non_empty_trimmed) {
        return Ok(path.to_string());
    }

    let paths = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    if let Some(path) = paths
        .iter()
        .find(|path| path.to_ascii_lowercase().ends_with("codex.cmd"))
    {
        return Ok((*path).to_string());
    }

    if let Some(path) = paths
        .iter()
        .find(|path| !path.to_ascii_lowercase().contains("node_modules"))
    {
        return Ok((*path).to_string());
    }

    Ok("codex".to_string())
}

fn non_empty_trimmed(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn run_codex_cli(invocation: &CodexCliInvocation) -> Result<CodexCliOutput> {
    let mut command = Command::new(&invocation.program);
    command
        .args(&invocation.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &invocation.env {
        command.env(key, value);
    }
    if let Some(current_dir) = &invocation.current_dir {
        command.current_dir(current_dir);
    }

    let mut child = command.spawn().with_context(|| {
        format!(
            "failed to start codex compact command: {} {}. Set Codex CLI command path in settings or ensure `where.exe codex` finds codex.cmd.",
            invocation.program,
            invocation.args.join(" ")
        )
    })?;

    let stdout = child
        .stdout
        .take()
        .context("failed to capture codex stdout")?;
    let stderr = child
        .stderr
        .take()
        .context("failed to capture codex stderr")?;
    let stdout_handle = thread::spawn(move || read_pipe(stdout));
    let stderr_handle = thread::spawn(move || read_pipe(stderr));

    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if started.elapsed() >= invocation.timeout {
            timed_out = true;
            let _ = child.kill();
            break child.wait().ok();
        }
        thread::sleep(Duration::from_millis(100));
    };

    let stdout = stdout_handle
        .join()
        .unwrap_or_else(|_| Err(std::io::Error::other("stdout reader panicked")))
        .context("failed to read codex stdout")?;
    let stderr = stderr_handle
        .join()
        .unwrap_or_else(|_| Err(std::io::Error::other("stderr reader panicked")))
        .context("failed to read codex stderr")?;

    Ok(CodexCliOutput {
        exit_code: status.and_then(|status| status.code()),
        stdout,
        stderr,
        timed_out,
    })
}

fn read_pipe<R: Read>(mut reader: R) -> std::io::Result<String> {
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    Ok(text)
}

fn backup_manifest_path(manifest: &backup_store::SessionBackupManifest) -> Result<String> {
    let session_path = manifest
        .backup_session_path
        .as_deref()
        .context("compact backup did not include a copied session JSONL path")?;
    Ok(std::path::PathBuf::from(session_path)
        .parent()
        .context("backup session path has no parent")?
        .join("manifest.json")
        .display()
        .to_string())
}
