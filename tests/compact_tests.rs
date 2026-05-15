use std::fs;
use std::path::Path;

use codex_session_manager::compact::{compact_session_with_guard_and_runner, CompactOptions};
use codex_session_manager::profile::CodexProfile;
use tempfile::tempdir;

#[test]
fn compact_apply_refuses_unsupported_noninteractive_codex_compact_before_backup_or_command() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    write_rollout(&profile.sessions_dir().join("thread-1.jsonl"), "thread-1");

    let error = compact_session_with_guard_and_runner(
        &profile,
        "thread-1",
        &CompactOptions { apply: true },
        || panic!("guard should not run when compact cannot be automated"),
        || panic!("runner should not be called when compact cannot be automated"),
    )
    .unwrap_err();

    let message = format!("{error:?}");
    assert!(message.contains("Codex CLI does not expose a non-interactive compact command"));
    assert!(message.contains("/compact must be run inside an interactive Codex session"));
    assert!(!profile.codex_home.join("backups").exists());
}

#[test]
fn compact_apply_refuses_before_reading_session_project_directory() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let project = dir.path().join("project-a");
    fs::create_dir_all(&project).unwrap();
    write_rollout_with_cwd(
        &profile.sessions_dir().join("thread-1.jsonl"),
        "thread-1",
        project.to_str().unwrap(),
    );

    let error = compact_session_with_guard_and_runner(
        &profile,
        "thread-1",
        &CompactOptions { apply: true },
        || panic!("guard should not run when compact cannot be automated"),
        || panic!("runner should not be called when compact cannot be automated"),
    )
    .unwrap_err();

    assert!(format!("{error:?}").contains("interactive Codex session"));
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
        || panic!("runner should not run during dry run"),
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
