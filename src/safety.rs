use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexProcess {
    pub kind: String,
    pub command: String,
}

pub fn ensure_codex_not_running() -> Result<()> {
    ensure_codex_not_running_with(detect_codex_processes)
}

pub fn ensure_codex_not_running_with<F>(detect: F) -> Result<()>
where
    F: FnOnce() -> Result<Vec<CodexProcess>>,
{
    let processes = detect()?;
    if processes.is_empty() {
        return Ok(());
    }

    let evidence = processes
        .iter()
        .map(|process| format!("{}: {}", process.kind, process.command))
        .collect::<Vec<_>>()
        .join("; ");
    bail!("Codex appears to be running; refusing to write. {evidence}");
}

pub fn detect_codex_processes() -> Result<Vec<CodexProcess>> {
    let proc_dir = Path::new("/proc");
    if proc_dir.exists() {
        return detect_codex_processes_from_proc(proc_dir);
    }

    detect_codex_processes_from_command()
}

fn detect_codex_processes_from_proc(proc_dir: &Path) -> Result<Vec<CodexProcess>> {
    let mut lines = Vec::new();
    for entry in fs::read_dir(proc_dir).context("failed to read /proc")? {
        let entry = entry?;
        let file_name = entry.file_name();
        if !file_name
            .to_string_lossy()
            .bytes()
            .all(|byte| byte.is_ascii_digit())
        {
            continue;
        }
        let cmdline = entry.path().join("cmdline");
        let Ok(bytes) = fs::read(cmdline) else {
            continue;
        };
        if bytes.is_empty() {
            continue;
        }
        let line = String::from_utf8_lossy(&bytes)
            .replace('\0', " ")
            .trim()
            .to_string();
        if !line.is_empty() {
            lines.push(line);
        }
    }

    Ok(detect_codex_processes_from_lines(&lines))
}

fn detect_codex_processes_from_command() -> Result<Vec<CodexProcess>> {
    let output = process_list_command()
        .output()
        .context("failed to inspect running processes")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!("failed to inspect running processes: {stderr}");
    }

    let lines = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    Ok(detect_codex_processes_from_lines(&lines))
}

#[cfg(target_os = "windows")]
fn process_list_command() -> Command {
    let mut command = Command::new("powershell.exe");
    command.args([
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        "Get-CimInstance Win32_Process | ForEach-Object { $_.CommandLine }",
    ]);
    hide_child_console(&mut command);
    command
}

#[cfg(not(target_os = "windows"))]
fn process_list_command() -> Command {
    let mut command = Command::new("ps");
    command.args(["-eo", "args="]);
    command
}

#[cfg(target_os = "windows")]
fn hide_child_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

pub fn detect_codex_processes_from_lines(lines: &[String]) -> Vec<CodexProcess> {
    lines
        .iter()
        .filter_map(|line| detect_codex_process(line))
        .collect()
}

fn detect_codex_process(line: &str) -> Option<CodexProcess> {
    let lower = line.to_ascii_lowercase();
    let kind = if lower.contains("codex desktop")
        || lower.contains("/codex/codex")
        || lower.contains("\\codex\\codex")
        || lower.ends_with("\\codex.exe")
    {
        "desktop"
    } else if lower.contains("app-server") && lower.contains("codex") {
        "app-server"
    } else if is_codex_cli(&lower) {
        "cli"
    } else {
        return None;
    };

    Some(CodexProcess {
        kind: kind.to_string(),
        command: line.to_string(),
    })
}

fn is_codex_cli(lower: &str) -> bool {
    lower == "codex"
        || lower.ends_with("/codex")
        || lower.ends_with("\\codex")
        || lower.ends_with("\\codex.cmd")
        || lower.contains(" codex ")
        || lower.contains("/.npm/bin/codex")
        || lower.contains("\\npm\\codex.cmd")
}
