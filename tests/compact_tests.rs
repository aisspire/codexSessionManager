use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use anyhow::bail;
use codex_session_manager::compact::{
    compact_session_with_guard_and_runner, CodexCliOutput, CompactOptions,
};
use codex_session_manager::profile::CodexProfile;
use tempfile::tempdir;

#[test]
fn compact_creates_backup_and_invokes_codex_exec_resume_compact() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    write_rollout(&profile.sessions_dir().join("thread-1.jsonl"), "thread-1");
    let seen = Rc::new(RefCell::new(None));
    let seen_runner = Rc::clone(&seen);

    let report = compact_session_with_guard_and_runner(
        &profile,
        "thread-1",
        &CompactOptions { apply: true },
        || Ok(()),
        move |invocation| {
            *seen_runner.borrow_mut() = Some(invocation.clone());
            Ok(CodexCliOutput {
                exit_code: Some(0),
                stdout: "compacted".to_string(),
                stderr: String::new(),
                timed_out: false,
            })
        },
    )
    .unwrap();

    let invocation = seen.borrow().clone().unwrap();
    assert_eq!(invocation.program, "codex");
    assert_eq!(
        invocation.args,
        vec![
            "exec",
            "resume",
            "--skip-git-repo-check",
            "thread-1",
            "/compact"
        ]
    );
    assert!(invocation
        .env
        .iter()
        .any(|(key, value)| key == "CI" && value == "1"));
    assert!(Path::new(&report.backup_manifest).exists());
    assert!(report.stdout.contains("compacted"));
}

#[test]
fn compact_refuses_when_codex_is_running_before_backup_or_command() {
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
fn compact_reports_cli_failure_output() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    write_rollout(&profile.sessions_dir().join("thread-1.jsonl"), "thread-1");

    let error = compact_session_with_guard_and_runner(
        &profile,
        "thread-1",
        &CompactOptions { apply: true },
        || Ok(()),
        |_| {
            Ok(CodexCliOutput {
                exit_code: Some(1),
                stdout: "update available".to_string(),
                stderr: "login required".to_string(),
                timed_out: false,
            })
        },
    )
    .unwrap_err();

    let message = format!("{error:?}");
    assert!(message.contains("codex compact command failed"));
    assert!(message.contains("update available"));
    assert!(message.contains("login required"));
}

#[test]
fn compact_reports_timeout_as_possible_interactive_prompt() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    write_rollout(&profile.sessions_dir().join("thread-1.jsonl"), "thread-1");

    let error = compact_session_with_guard_and_runner(
        &profile,
        "thread-1",
        &CompactOptions { apply: true },
        || Ok(()),
        |_| {
            Ok(CodexCliOutput {
                exit_code: None,
                stdout: String::new(),
                stderr: "checking for updates".to_string(),
                timed_out: true,
            })
        },
    )
    .unwrap_err();

    let message = format!("{error:?}");
    assert!(message.contains("timed out"));
    assert!(message.contains("interactive prompt"));
    assert!(message.contains("checking for updates"));
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
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!(
            r#"{{"type":"session_meta","payload":{{"id":"{id}","cwd":"/tmp/project","source":"cli","model_provider":"cm"}}}}"#
        ),
    )
    .unwrap();
}
