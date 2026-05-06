use std::fs;
use std::path::Path;

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
    detect_codex_processes_from_proc(Path::new("/proc"))
}

fn detect_codex_processes_from_proc(proc_dir: &Path) -> Result<Vec<CodexProcess>> {
    if !proc_dir.exists() {
        bail!("cannot inspect running processes on this platform");
    }

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

pub fn detect_codex_processes_from_lines(lines: &[String]) -> Vec<CodexProcess> {
    lines
        .iter()
        .filter_map(|line| detect_codex_process(line))
        .collect()
}

fn detect_codex_process(line: &str) -> Option<CodexProcess> {
    let lower = line.to_ascii_lowercase();
    let kind = if lower.contains("codex desktop") || lower.contains("/codex/codex") {
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
        || lower.contains(" codex ")
        || lower.contains("/.npm/bin/codex")
}
