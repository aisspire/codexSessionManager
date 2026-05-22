use std::fs;
use std::path::Path;

use anyhow::bail;
use codex_session_manager::compact::{
    compact_session_with_guard_and_runner,
    compact_session_with_local_provider_fallback_with_guard_and_runner,
    resolve_codex_command_from_where_output, CodexAppServerOutput, CompactOptions,
};
use codex_session_manager::profile::CodexProfile;
use codex_session_manager::settings::{save_settings, AppSettings};
use tempfile::tempdir;

#[test]
fn compact_creates_backup_and_invokes_codex_app_server_compact() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let project = dir.path().join("project-a");
    fs::create_dir_all(&project).unwrap();
    write_rollout_with_cwd(
        &profile.sessions_dir().join("thread-1.jsonl"),
        "thread-1",
        project.to_str().unwrap(),
    );

    let report = compact_session_with_guard_and_runner(
        &profile,
        "thread-1",
        &CompactOptions { apply: true },
        || Ok(()),
        |invocation| {
            assert!(invocation.program.to_ascii_lowercase().contains("codex"));
            assert_eq!(invocation.args, vec!["app-server"]);
            assert_eq!(invocation.thread_id, "thread-1");
            assert_eq!(invocation.cwd.as_deref(), Some(project.to_str().unwrap()));
            assert_eq!(
                invocation.rollout_path.as_deref(),
                Some(
                    profile
                        .sessions_dir()
                        .join("thread-1.jsonl")
                        .to_str()
                        .unwrap()
                )
            );
            fs::write(
                invocation.rollout_path.as_ref().unwrap(),
                format!(
                    "{}\n{}",
                    fs::read_to_string(invocation.rollout_path.as_ref().unwrap()).unwrap(),
                    serde_json::json!({"type": "contextCompaction"})
                ),
            )
            .unwrap();
            Ok(CodexAppServerOutput {
                stdout: "thread compacted".to_string(),
                stderr: String::new(),
            })
        },
    )
    .unwrap();

    assert!(Path::new(&report.backup_manifest).exists());
    assert!(report.command[0].to_ascii_lowercase().contains("codex"));
    assert_eq!(report.command[1], "app-server");
    assert!(report.stdout.contains("thread compacted"));
}

#[test]
fn compact_uses_configured_codex_command_path() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    write_rollout(&profile.sessions_dir().join("thread-1.jsonl"), "thread-1");
    let mut settings = AppSettings::default();
    settings.codex_cli.command_path = Some(r"C:\Tools\codex.cmd".to_string());
    save_settings(&profile, &settings).unwrap();

    compact_session_with_guard_and_runner(
        &profile,
        "thread-1",
        &CompactOptions { apply: true },
        || Ok(()),
        |invocation| {
            assert_eq!(invocation.program, r"C:\Tools\codex.cmd");
            fs::write(
                invocation.rollout_path.as_ref().unwrap(),
                format!(
                    "{}\n{}",
                    fs::read_to_string(invocation.rollout_path.as_ref().unwrap()).unwrap(),
                    serde_json::json!({"type": "contextCompaction"})
                ),
            )
            .unwrap();
            Ok(CodexAppServerOutput {
                stdout: String::new(),
                stderr: String::new(),
            })
        },
    )
    .unwrap();
}

#[test]
fn configured_codex_command_path_wins() {
    let resolved =
        resolve_codex_command_from_where_output(Some(" C:\\Tools\\codex.cmd "), "").unwrap();

    assert_eq!(resolved, "C:\\Tools\\codex.cmd");
}

#[test]
fn where_resolver_prefers_codex_cmd_over_internal_node_binary() {
    let output = [
        r"C:\project\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\codex\codex.exe",
        r"C:\Users\me\AppData\Roaming\npm\codex.cmd",
    ]
    .join("\r\n");

    let resolved = resolve_codex_command_from_where_output(None, &output).unwrap();

    assert_eq!(resolved, r"C:\Users\me\AppData\Roaming\npm\codex.cmd");
}

#[test]
fn compact_refuses_when_codex_is_running_before_backup_or_app_server() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    write_rollout(&profile.sessions_dir().join("thread-1.jsonl"), "thread-1");

    let error = compact_session_with_guard_and_runner(
        &profile,
        "thread-1",
        &CompactOptions { apply: true },
        || bail!("Codex appears to be running"),
        |_| panic!("runner should not be called when guard fails"),
    )
    .unwrap_err();

    assert!(format!("{error:?}").contains("Codex appears to be running"));
    assert!(!profile.codex_home.join("backups").exists());
}

#[test]
fn compact_app_server_failure_reports_captured_output() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    write_rollout(&profile.sessions_dir().join("thread-1.jsonl"), "thread-1");

    let error = compact_session_with_guard_and_runner(
        &profile,
        "thread-1",
        &CompactOptions { apply: true },
        || Ok(()),
        |_invocation| {
            bail!("codex app-server compact failed\nstdout:\npartial\nstderr:\nlogin required")
        },
    )
    .unwrap_err();

    let message = format!("{error:?}");
    assert!(message.contains("app-server compact failed"));
    assert!(message.contains("login required"));
}

#[test]
fn compact_reports_failure_when_app_server_has_no_local_effect() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    write_rollout(&profile.sessions_dir().join("thread-1.jsonl"), "thread-1");

    let error = compact_session_with_guard_and_runner(
        &profile,
        "thread-1",
        &CompactOptions { apply: true },
        || Ok(()),
        |_invocation| {
            Ok(CodexAppServerOutput {
                stdout: "thread compacted".to_string(),
                stderr: String::new(),
            })
        },
    )
    .unwrap_err();

    let message = format!("{error:?}");
    assert!(message.contains("reported success but did not change"));
    assert!(message.contains("thread compacted"));
}

#[test]
fn local_provider_fallback_temporarily_switches_openai_name_to_csm_and_retries_compact() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    write_rollout(&profile.sessions_dir().join("thread-1.jsonl"), "thread-1");
    fs::write(
        profile.config_path(),
        r#"
model_provider = "openai"

[model_providers.openai]
name = "OpenAI"
base_url = "https://api.openai.com/v1"
"#,
    )
    .unwrap();

    let report = compact_session_with_local_provider_fallback_with_guard_and_runner(
        &profile,
        "thread-1",
        &CompactOptions { apply: true },
        || Ok(()),
        |invocation| {
            fs::write(
                invocation.rollout_path.as_ref().unwrap(),
                format!(
                    "{}\n{}",
                    fs::read_to_string(invocation.rollout_path.as_ref().unwrap()).unwrap(),
                    serde_json::json!({"type": "contextCompaction"})
                ),
            )
            .unwrap();
            Ok(CodexAppServerOutput {
                stdout: "local compacted".to_string(),
                stderr: String::new(),
            })
        },
    )
    .unwrap();

    let updated = fs::read_to_string(profile.config_path()).unwrap();
    assert!(updated.contains(r#"model_provider = "openai""#));
    assert!(updated.contains(r#"name = "OpenAI""#));
    assert!(updated.contains(r#"base_url = "https://api.openai.com/v1""#));
    assert!(report.stdout.contains("local compacted"));
}

#[test]
fn local_provider_fallback_restores_openai_name_when_retry_fails() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    write_rollout(&profile.sessions_dir().join("thread-1.jsonl"), "thread-1");
    fs::write(
        profile.config_path(),
        r#"
model_provider = "openai"

[model_providers.openai]
name = "OpenAI"
"#,
    )
    .unwrap();

    let error = compact_session_with_local_provider_fallback_with_guard_and_runner(
        &profile,
        "thread-1",
        &CompactOptions { apply: true },
        || Ok(()),
        |_| bail!("local compact failed"),
    )
    .unwrap_err();

    let updated = fs::read_to_string(profile.config_path()).unwrap();
    assert!(format!("{error:?}").contains("local compact failed"));
    assert!(updated.contains(r#"name = "OpenAI""#));
}

#[test]
fn local_provider_fallback_stops_when_provider_name_is_already_not_openai() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    write_rollout(&profile.sessions_dir().join("thread-1.jsonl"), "thread-1");
    fs::write(
        profile.config_path(),
        r#"
model_provider = "cm"

[model_providers.cm]
name = "CSM"
"#,
    )
    .unwrap();

    let error = compact_session_with_local_provider_fallback_with_guard_and_runner(
        &profile,
        "thread-1",
        &CompactOptions { apply: true },
        || Ok(()),
        |_| panic!("runner should not be called when provider is already local"),
    )
    .unwrap_err();

    assert!(format!("{error:?}").contains("已经是本地压缩，停止操作"));
}

#[test]
fn compact_dry_run_does_not_backup_or_invoke_command() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    write_rollout(&profile.sessions_dir().join("thread-1.jsonl"), "thread-1");

    let report = compact_session_with_guard_and_runner(
        &profile,
        "thread-1",
        &CompactOptions { apply: false },
        || panic!("guard should not run during dry run"),
        |_| panic!("runner should not run during dry run"),
    )
    .unwrap();

    assert!(!report.applied);
    assert!(report.backup_manifest.is_empty());
    assert!(!profile.codex_home.join("backups").exists());
}

fn write_rollout(path: &Path, id: &str) {
    write_rollout_with_cwd(path, id, "/tmp/project");
}

fn write_rollout_with_cwd(path: &Path, id: &str, cwd: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let line = serde_json::json!({
        "type": "session_meta",
        "payload": {
            "id": id,
            "cwd": cwd,
            "source": "cli",
            "model_provider": "cm",
        }
    });
    fs::write(path, serde_json::to_string(&line).unwrap()).unwrap();
}
