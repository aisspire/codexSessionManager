use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

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
    let current_uid = read_proc_uid(&proc_dir.join("self").join("status"))?;
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
        let process_dir = entry.path();
        if !proc_entry_may_belong_to_uid(&process_dir, current_uid)? {
            continue;
        }

        let status_path = process_dir.join("status");
        let process_uid = match fs::read_to_string(&status_path) {
            Ok(status) => parse_proc_uid(&status).with_context(|| {
                format!("process status has no real UID {}", status_path.display())
            })?,
            Err(error) if should_skip_proc_read_error(&error, true) => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to read process status {}", status_path.display())
                })
            }
        };
        if process_uid != current_uid {
            continue;
        }

        let cmdline = process_dir.join("cmdline");
        let bytes = match fs::read(&cmdline) {
            Ok(bytes) => bytes,
            Err(error) if should_skip_proc_read_error(&error, true) => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to read process command line {}", cmdline.display())
                })
            }
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

fn read_proc_uid(path: &Path) -> Result<u32> {
    let status = fs::read_to_string(path)
        .with_context(|| format!("failed to read current process status {}", path.display()))?;
    parse_proc_uid(&status)
        .with_context(|| format!("current process status has no real UID: {}", path.display()))
}

fn parse_proc_uid(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:")?.split_whitespace().next())
        .and_then(|value| value.parse().ok())
}

fn proc_entry_may_belong_to_uid(process_dir: &Path, current_uid: u32) -> Result<bool> {
    #[cfg(unix)]
    {
        match fs::metadata(process_dir) {
            Ok(metadata) => return Ok(metadata.uid() == current_uid),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "failed to inspect process ownership {}",
                        process_dir.display()
                    )
                })
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = (process_dir, current_uid);
        Ok(true)
    }
}

fn should_skip_proc_read_error(error: &std::io::Error, same_uid: bool) -> bool {
    error.kind() == std::io::ErrorKind::NotFound
        || (!same_uid && error.kind() == std::io::ErrorKind::PermissionDenied)
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

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn write_process(proc_dir: &Path, pid: &str, uid: u32, command: &str) {
        let process_dir = proc_dir.join(pid);
        fs::create_dir_all(&process_dir).unwrap();
        fs::write(
            process_dir.join("status"),
            format!("Name:\ttest\nUid:\t{uid}\t{uid}\t{uid}\t{uid}\n"),
        )
        .unwrap();
        fs::write(process_dir.join("cmdline"), command.replace(' ', "\0")).unwrap();
    }

    #[test]
    fn proc_detection_reads_same_uid_processes_and_skips_other_uids() {
        let directory = tempdir().unwrap();
        let proc_dir = directory.path();
        fs::create_dir_all(proc_dir.join("self")).unwrap();
        fs::write(
            proc_dir.join("self/status"),
            "Name:\thelper\nUid:\t1000\t1000\t1000\t1000\n",
        )
        .unwrap();
        write_process(proc_dir, "101", 1000, "codex app-server");
        write_process(proc_dir, "102", 2000, "codex app-server");

        let processes = detect_codex_processes_from_proc(proc_dir).unwrap();

        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].kind, "app-server");
    }

    #[test]
    fn proc_detection_ignores_processes_that_disappear_before_reading() {
        let directory = tempdir().unwrap();
        let proc_dir = directory.path();
        fs::create_dir_all(proc_dir.join("self")).unwrap();
        fs::write(
            proc_dir.join("self/status"),
            "Uid:\t1000\t1000\t1000\t1000\n",
        )
        .unwrap();
        fs::create_dir(proc_dir.join("101")).unwrap();

        assert!(detect_codex_processes_from_proc(proc_dir)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn proc_permission_errors_are_fail_closed_for_the_current_uid() {
        let permission_denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);

        assert!(!should_skip_proc_read_error(&permission_denied, true));
        assert!(should_skip_proc_read_error(&permission_denied, false));
    }
}
