use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use toml_edit::{value, DocumentMut};

use crate::backup_store::{self, BackupTrigger};
use crate::path_map::path_buf_for_current_os;
use crate::profile::CodexProfile;
use crate::rollout::read_rollout_meta;
use crate::safety;
use crate::settings;

const COMPACT_TIMEOUT: Duration = Duration::from_secs(120);
const COMPACT_EFFECT_TIMEOUT: Duration = Duration::from_secs(5);
const APP_SERVER_EXIT_GRACE: Duration = Duration::from_secs(2);

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
        ];
        if !self.command.is_empty() {
            lines.push(format!("command: {}", self.command.join(" ")));
        }
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
pub struct CodexAppServerInvocation {
    pub program: String,
    pub args: Vec<String>,
    pub thread_id: String,
    pub cwd: Option<String>,
    pub rollout_path: Option<String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexAppServerOutput {
    pub stdout: String,
    pub stderr: String,
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
        run_codex_app_server_compact,
    )
}

pub fn compact_session_with_local_provider_fallback(
    profile: &CodexProfile,
    session_id: &str,
    options: &CompactOptions,
) -> Result<CompactReport> {
    compact_session_with_local_provider_fallback_with_guard_and_runner(
        profile,
        session_id,
        options,
        safety::ensure_codex_not_running,
        run_codex_app_server_compact,
    )
}

pub fn compact_session_with_local_provider_fallback_with_guard_and_runner<G, R>(
    profile: &CodexProfile,
    session_id: &str,
    options: &CompactOptions,
    guard: G,
    runner: R,
) -> Result<CompactReport>
where
    G: FnOnce() -> Result<()>,
    R: FnOnce(&CodexAppServerInvocation) -> Result<CodexAppServerOutput>,
{
    if options.apply {
        guard()?;
    }
    let restore = if options.apply {
        Some(switch_openai_provider_name_to_local(profile)?)
    } else {
        None
    };
    let result =
        compact_session_with_guard_and_runner(profile, session_id, options, || Ok(()), runner);
    if let Some(restore) = restore {
        restore
            .restore()
            .context("failed to restore provider name after local compact attempt")?;
    }
    result
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
    R: FnOnce(&CodexAppServerInvocation) -> Result<CodexAppServerOutput>,
{
    let configured = settings::load_settings(profile)?
        .codex_cli
        .command_path
        .unwrap_or_default();
    let invocation = CodexAppServerInvocation {
        program: resolve_codex_command(Some(configured.as_str()))?,
        args: vec!["app-server".to_string()],
        thread_id: session_id.to_string(),
        cwd: None,
        rollout_path: None,
        timeout: COMPACT_TIMEOUT,
    };
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
    let invocation = CodexAppServerInvocation {
        cwd: compact_current_dir(&local_session_path)?,
        rollout_path: Some(local_session_path.display().to_string()),
        ..invocation
    };
    let manifest =
        backup_store::create_session_backup(profile, session_id, BackupTrigger::Compact)?;
    report.backup_manifest = backup_manifest_path(&manifest)?;

    let before_compact = session_file_fingerprint(&local_session_path)?;
    let output = runner(&invocation)?;
    if let Err(error) =
        wait_for_session_file_change(&local_session_path, &before_compact, COMPACT_EFFECT_TIMEOUT)
    {
        bail!(
            "codex app-server reported success but did not change local session JSONL: {}\nstdout:\n{}\nstderr:\n{}\n{error}",
            local_session_path.display(),
            output.stdout,
            output.stderr
        );
    }
    report.stdout = output.stdout;
    report.stderr = output.stderr;
    Ok(report)
}

struct ProviderNameRestore {
    config_path: std::path::PathBuf,
    provider: String,
    original_name: String,
}

impl ProviderNameRestore {
    fn restore(self) -> Result<()> {
        write_provider_name(&self.config_path, &self.provider, &self.original_name)
    }
}

fn switch_openai_provider_name_to_local(profile: &CodexProfile) -> Result<ProviderNameRestore> {
    let config_path = profile.config_path();
    let text = fs::read_to_string(&config_path).with_context(|| {
        format!(
            "failed to read Codex config.toml: {}",
            config_path.display()
        )
    })?;
    let mut doc = text.parse::<DocumentMut>().with_context(|| {
        format!(
            "failed to parse Codex config.toml: {}",
            config_path.display()
        )
    })?;
    let provider = doc["model_provider"]
        .as_str()
        .map(str::trim)
        .filter(|provider| !provider.is_empty())
        .context("config.toml is missing model_provider")?
        .to_string();
    let provider_name = doc["model_providers"][&provider]["name"]
        .as_str()
        .map(str::trim)
        .context("config.toml is missing model_providers.<model_provider>.name")?;
    if !provider_name.eq_ignore_ascii_case("OpenAI") {
        bail!("已经是本地压缩，停止操作");
    }
    let original_name = provider_name.to_string();

    doc["model_providers"][&provider]["name"] = value("CSM");
    fs::write(&config_path, doc.to_string()).with_context(|| {
        format!(
            "failed to write Codex config.toml: {}",
            config_path.display()
        )
    })?;
    Ok(ProviderNameRestore {
        config_path,
        provider,
        original_name,
    })
}

fn write_provider_name(config_path: &Path, provider: &str, name: &str) -> Result<()> {
    let text = fs::read_to_string(config_path).with_context(|| {
        format!(
            "failed to read Codex config.toml: {}",
            config_path.display()
        )
    })?;
    let mut doc = text.parse::<DocumentMut>().with_context(|| {
        format!(
            "failed to parse Codex config.toml: {}",
            config_path.display()
        )
    })?;
    doc["model_providers"][provider]["name"] = value(name);
    fs::write(config_path, doc.to_string()).with_context(|| {
        format!(
            "failed to write Codex config.toml: {}",
            config_path.display()
        )
    })
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

fn run_codex_app_server_compact(
    invocation: &CodexAppServerInvocation,
) -> Result<CodexAppServerOutput> {
    let mut command = Command::new(&invocation.program);
    command
        .args(&invocation.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = &invocation.cwd {
        command.current_dir(cwd);
    }

    let mut child = command.spawn().with_context(|| {
        format!(
            "failed to start codex app-server command: {} {}. Set Codex CLI command path in settings or ensure `where.exe codex` finds codex.cmd.",
            invocation.program,
            invocation.args.join(" ")
        )
    })?;

    let mut stdin = child
        .stdin
        .take()
        .context("failed to capture codex app-server stdin")?;
    let stdout = child
        .stdout
        .take()
        .context("failed to capture codex app-server stdout")?;
    let stderr = child
        .stderr
        .take()
        .context("failed to capture codex app-server stderr")?;
    let (line_rx, stdout_handle) = spawn_stdout_reader(stdout);
    let stderr_handle = thread::spawn(move || read_pipe(stderr));
    let deadline = Instant::now() + invocation.timeout;
    let mut raw_stdout = String::new();

    let result =
        run_app_server_protocol(invocation, &mut stdin, &line_rx, &mut raw_stdout, deadline);
    drop(stdin);
    let stderr = finish_child(child, stdout_handle, stderr_handle)?;

    match result {
        Ok(()) => Ok(CodexAppServerOutput {
            stdout: raw_stdout,
            stderr,
        }),
        Err(error) => bail!(
            "codex app-server compact failed: {error}\nstdout:\n{raw_stdout}\nstderr:\n{stderr}"
        ),
    }
}

fn run_app_server_protocol(
    invocation: &CodexAppServerInvocation,
    stdin: &mut impl Write,
    line_rx: &Receiver<std::io::Result<String>>,
    raw_stdout: &mut String,
    deadline: Instant,
) -> Result<()> {
    send_json(
        stdin,
        &json!({
            "id": 1,
            "method": "initialize",
            "params": {
                "clientInfo": {
                    "name": "codex-session-manager",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": true
                }
            }
        }),
    )?;
    wait_for_response(line_rx, raw_stdout, 1, deadline)?;
    send_json(stdin, &json!({"method": "initialized"}))?;

    let resume_result = wait_for_response_after_send(
        stdin,
        line_rx,
        raw_stdout,
        deadline,
        &json!({
            "id": 2,
            "method": "thread/resume",
            "params": {
                "threadId": invocation.thread_id,
                "cwd": invocation.cwd,
                "path": invocation.rollout_path,
                "persistExtendedHistory": false
            }
        }),
        2,
    )?;

    let compact_thread_id = resume_result
        .get("thread")
        .and_then(|thread| thread.get("id"))
        .and_then(Value::as_str)
        .unwrap_or(&invocation.thread_id)
        .to_string();

    wait_for_response_after_send(
        stdin,
        line_rx,
        raw_stdout,
        deadline,
        &json!({
            "id": 3,
            "method": "thread/compact/start",
            "params": {
                "threadId": compact_thread_id
            }
        }),
        3,
    )?;
    wait_for_compaction(line_rx, raw_stdout, &compact_thread_id, deadline)
}

fn wait_for_response_after_send(
    stdin: &mut impl Write,
    line_rx: &Receiver<std::io::Result<String>>,
    raw_stdout: &mut String,
    deadline: Instant,
    request: &Value,
    id: i64,
) -> Result<Value> {
    send_json(stdin, request)?;
    wait_for_response(line_rx, raw_stdout, id, deadline)
}

fn send_json(stdin: &mut impl Write, value: &Value) -> Result<()> {
    serde_json::to_writer(&mut *stdin, value)
        .context("failed to serialize codex app-server request")?;
    stdin
        .write_all(b"\n")
        .context("failed to write codex app-server request")?;
    stdin
        .flush()
        .context("failed to flush codex app-server request")
}

fn wait_for_response(
    line_rx: &Receiver<std::io::Result<String>>,
    raw_stdout: &mut String,
    id: i64,
    deadline: Instant,
) -> Result<Value> {
    loop {
        let value = recv_json_line(line_rx, raw_stdout, deadline)?;
        if value.get("id").and_then(Value::as_i64) == Some(id) {
            if let Some(error) = value.get("error") {
                bail!("request {id} returned error: {error}");
            }
            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
        }
        if value.get("method").and_then(Value::as_str) == Some("error") {
            bail!("app-server error notification: {value}");
        }
    }
}

fn wait_for_compaction(
    line_rx: &Receiver<std::io::Result<String>>,
    raw_stdout: &mut String,
    thread_id: &str,
    deadline: Instant,
) -> Result<()> {
    loop {
        let value = recv_json_line(line_rx, raw_stdout, deadline)?;
        if value.get("method").and_then(Value::as_str) == Some("thread/compacted")
            && value
                .get("params")
                .and_then(|params| params.get("threadId"))
                .and_then(Value::as_str)
                == Some(thread_id)
        {
            return Ok(());
        }
        if value.get("method").and_then(Value::as_str) == Some("contextCompacted")
            && value
                .get("params")
                .and_then(|params| params.get("threadId"))
                .and_then(Value::as_str)
                == Some(thread_id)
        {
            return Ok(());
        }
        if is_completed_context_compaction(&value, thread_id) {
            return Ok(());
        }
        if value.get("method").and_then(Value::as_str) == Some("error") {
            bail!("app-server error notification: {value}");
        }
    }
}

fn recv_json_line(
    line_rx: &Receiver<std::io::Result<String>>,
    raw_stdout: &mut String,
    deadline: Instant,
) -> Result<Value> {
    let now = Instant::now();
    if now >= deadline {
        bail!("timed out waiting for codex app-server response");
    }
    let line = line_rx
        .recv_timeout(deadline.saturating_duration_since(now))
        .context("timed out waiting for codex app-server response")?
        .context("failed to read codex app-server stdout")?;
    raw_stdout.push_str(&line);
    serde_json::from_str(line.trim())
        .with_context(|| format!("failed to parse codex app-server JSON line: {line}"))
}

fn contains_context_compaction(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            matches!(
                map.get("type").and_then(Value::as_str),
                Some("contextCompaction" | "compaction")
            ) || map.values().any(contains_context_compaction)
        }
        Value::Array(items) => items.iter().any(contains_context_compaction),
        _ => false,
    }
}

fn is_completed_context_compaction(value: &Value, thread_id: &str) -> bool {
    if value.get("method").and_then(Value::as_str) != Some("item/completed") {
        return false;
    }
    let Some(params) = value.get("params") else {
        return false;
    };
    if params
        .get("threadId")
        .and_then(Value::as_str)
        .is_some_and(|value| value != thread_id)
    {
        return false;
    }
    contains_context_compaction(params)
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use super::wait_for_compaction;

    #[test]
    fn wait_for_compaction_ignores_started_context_compaction_until_completed() {
        let (tx, rx) = mpsc::channel();
        tx.send(Ok(r#"{"method":"item/started","params":{"threadId":"thread-1","item":{"type":"contextCompaction","id":"item-1"}}}"#.to_string()))
            .unwrap();
        tx.send(Ok(r#"{"method":"item/completed","params":{"threadId":"thread-1","item":{"type":"contextCompaction","id":"item-1"}}}"#.to_string()))
            .unwrap();
        let mut raw_stdout = String::new();

        wait_for_compaction(
            &rx,
            &mut raw_stdout,
            "thread-1",
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();

        assert!(raw_stdout.contains("item/started"));
        assert!(raw_stdout.contains("item/completed"));
    }

    #[test]
    fn wait_for_compaction_accepts_thread_compacted_notification() {
        let (tx, rx) = mpsc::channel();
        tx.send(Ok(
            r#"{"method":"thread/compacted","params":{"threadId":"thread-1"}}"#.to_string(),
        ))
        .unwrap();
        let mut raw_stdout = String::new();

        wait_for_compaction(
            &rx,
            &mut raw_stdout,
            "thread-1",
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();

        assert!(raw_stdout.contains("thread/compacted"));
    }

    #[test]
    fn wait_for_compaction_ignores_completed_context_compaction_for_other_thread() {
        let (tx, rx) = mpsc::channel();
        tx.send(Ok(r#"{"method":"item/completed","params":{"threadId":"thread-2","item":{"type":"contextCompaction","id":"item-1"}}}"#.to_string()))
            .unwrap();
        tx.send(Ok(
            r#"{"method":"thread/compacted","params":{"threadId":"thread-1"}}"#.to_string(),
        ))
        .unwrap();
        let mut raw_stdout = String::new();

        wait_for_compaction(
            &rx,
            &mut raw_stdout,
            "thread-1",
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();

        assert!(raw_stdout.contains("thread-2"));
        assert!(raw_stdout.contains("thread-1"));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionFileFingerprint {
    len: u64,
    modified: Option<SystemTime>,
}

fn session_file_fingerprint(path: &Path) -> Result<SessionFileFingerprint> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to stat local session JSONL: {}", path.display()))?;
    Ok(SessionFileFingerprint {
        len: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn wait_for_session_file_change(
    path: &Path,
    before: &SessionFileFingerprint,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if session_file_fingerprint(path)? != *before {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for compact side effect");
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn spawn_stdout_reader<R: Read + Send + 'static>(
    reader: R,
) -> (
    Receiver<std::io::Result<String>>,
    thread::JoinHandle<std::io::Result<()>>,
) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        loop {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line)?;
            if bytes == 0 {
                break;
            }
            if tx.send(Ok(line)).is_err() {
                break;
            }
        }
        Ok(())
    });
    (rx, handle)
}

fn finish_child(
    mut child: Child,
    stdout_handle: thread::JoinHandle<std::io::Result<()>>,
    stderr_handle: thread::JoinHandle<std::io::Result<String>>,
) -> Result<String> {
    let exit_deadline = Instant::now() + APP_SERVER_EXIT_GRACE;
    while Instant::now() < exit_deadline {
        if child.try_wait()?.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    if child.try_wait()?.is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
    let _ = stdout_handle
        .join()
        .unwrap_or_else(|_| Err(std::io::Error::other("stdout reader panicked")));
    stderr_handle
        .join()
        .unwrap_or_else(|_| Err(std::io::Error::other("stderr reader panicked")))
        .context("failed to read codex app-server stderr")
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
