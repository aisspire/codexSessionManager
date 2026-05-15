use std::fs;
use std::path::Path;

use codex_session_manager::backup_store::{
    create_session_backup, list_session_backups, BackupTrigger,
};
use codex_session_manager::favorites::{favorite_ids, set_favorite};
use codex_session_manager::profile::CodexProfile;
use codex_session_manager::restore::{
    preview_restore_session_backup, restore_session_backup_with_guard, RestoreSessionOptions,
};
use codex_session_manager::session_list::{list_sessions, SessionListFilter};
use codex_session_manager::session_ops::{delete_sessions_with_guard, SessionApplyOptions};
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn restoring_deleted_session_recreates_jsonl_merges_index_and_syncs_sqlite() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    let rollout = profile.sessions_dir().join("thread-1.jsonl");
    write_rollout(&rollout, "thread-1", "/tmp/project", "cm");
    fs::write(
        profile.session_index_path(),
        "{\"id\":\"thread-1\",\"thread_name\":\"Restored title\",\"updated_at\":\"2026-05-06T00:00:00Z\"}\n",
    )
    .unwrap();
    set_favorite(&profile, "thread-1", true).unwrap();
    create_session_backup(&profile, "thread-1", BackupTrigger::Delete).unwrap();
    let backup_id = list_session_backups(&profile).unwrap()[0].snapshots[0]
        .backup_id
        .clone();
    fs::remove_file(&rollout).unwrap();
    fs::write(profile.session_index_path(), "").unwrap();
    delete_thread(&profile.state_db_path(), "thread-1");
    set_favorite(&profile, "thread-1", false).unwrap();

    let preview = preview_restore_session_backup(&profile, &backup_id).unwrap();
    assert_eq!(preview.session_id, "thread-1");
    assert_eq!(
        preview.restore_session_path.as_deref(),
        Some(rollout.to_str().unwrap())
    );
    assert!(!preview.overwrites_existing);

    let report = restore_session_backup_with_guard(
        &profile,
        &backup_id,
        &RestoreSessionOptions {
            apply: true,
            overwrite_existing: false,
            restore_favorite: true,
        },
        || Ok(()),
    )
    .unwrap();

    assert!(report.applied);
    assert_eq!(report.files_restored, 1);
    assert_eq!(
        fs::read_to_string(&rollout).unwrap(),
        rollout_body("thread-1", "/tmp/project", "cm")
    );
    assert_eq!(
        fs::read_to_string(profile.session_index_path())
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert_thread_exists(&profile.state_db_path(), "thread-1");
    assert!(favorite_ids(&profile).unwrap().contains("thread-1"));
}

#[test]
fn restore_deleted_session_recreates_original_sqlite_thread() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let rollout = profile.sessions_dir().join("thread-1.jsonl");
    write_rollout(&rollout, "thread-1", "/tmp/project", "cm");
    create_state_db_with_thread(&profile.state_db_path(), &rollout);
    fs::write(
        profile.session_index_path(),
        "{\"id\":\"thread-1\",\"thread_name\":\"Restored title\",\"updated_at\":\"2026-05-06T00:00:00Z\"}\n",
    )
    .unwrap();

    let delete_report = delete_sessions_with_guard(
        &profile,
        &["thread-1".to_string()],
        &SessionApplyOptions { apply: true },
        || Ok(()),
    )
    .unwrap();
    let backup_id = list_session_backups(&profile).unwrap()[0].snapshots[0]
        .backup_id
        .clone();

    assert_eq!(delete_report.sqlite_rows, 1);
    assert!(!rollout.exists());
    assert_thread_missing(&profile.state_db_path(), "thread-1");

    let report = restore_session_backup_with_guard(
        &profile,
        &backup_id,
        &RestoreSessionOptions {
            apply: true,
            overwrite_existing: false,
            restore_favorite: true,
        },
        || Ok(()),
    )
    .unwrap();

    assert_eq!(report.sqlite_rows, 1);
    assert!(rollout.exists());
    assert_thread_rollout_path(&profile.state_db_path(), "thread-1", rollout.to_str().unwrap());
    assert!(list_sessions(&profile, &SessionListFilter::default())
        .unwrap()
        .iter()
        .any(|session| session.id == "thread-1"));
}

#[test]
fn restore_uses_dated_sessions_path_when_original_path_is_unavailable() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let original = profile
        .archived_sessions_dir()
        .join("rollout-2026-05-06T20-43-11-thread-1.jsonl");
    write_rollout(&original, "thread-1", "/tmp/project", "cm");
    create_session_backup(&profile, "thread-1", BackupTrigger::Delete).unwrap();
    let backup_id = list_session_backups(&profile).unwrap()[0].snapshots[0]
        .backup_id
        .clone();
    fs::remove_file(&original).unwrap();
    fs::create_dir_all(&original).unwrap();

    let report = restore_session_backup_with_guard(
        &profile,
        &backup_id,
        &RestoreSessionOptions {
            apply: true,
            overwrite_existing: false,
            restore_favorite: false,
        },
        || Ok(()),
    )
    .unwrap();

    let fallback = profile
        .sessions_dir()
        .join("2026")
        .join("05")
        .join("06")
        .join("rollout-2026-05-06T20-43-11-thread-1.jsonl");
    assert_eq!(
        report.restored_session_path.as_deref(),
        Some(fallback.to_str().unwrap())
    );
    assert!(fallback.is_file());
}

#[test]
fn restore_creates_preflight_backup_before_overwriting_existing_jsonl() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let rollout = profile.sessions_dir().join("thread-1.jsonl");
    write_rollout(&rollout, "thread-1", "/tmp/original", "cm");
    create_session_backup(&profile, "thread-1", BackupTrigger::Delete).unwrap();
    let backup_id = list_session_backups(&profile).unwrap()[0].snapshots[0]
        .backup_id
        .clone();
    write_rollout(&rollout, "thread-1", "/tmp/current", "openai");

    let report = restore_session_backup_with_guard(
        &profile,
        &backup_id,
        &RestoreSessionOptions {
            apply: true,
            overwrite_existing: true,
            restore_favorite: false,
        },
        || Ok(()),
    )
    .unwrap();

    assert!(report.preflight_backup_manifest.is_some());
    assert!(fs::read_to_string(&rollout)
        .unwrap()
        .contains("/tmp/original"));
    let snapshots = list_session_backups(&profile).unwrap()[0].snapshots.clone();
    assert!(snapshots
        .iter()
        .any(|snapshot| snapshot.trigger == BackupTrigger::RestorePreflight));
}

#[test]
fn restore_refuses_to_apply_when_codex_is_running() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let rollout = profile.sessions_dir().join("thread-1.jsonl");
    write_rollout(&rollout, "thread-1", "/tmp/project", "cm");
    create_session_backup(&profile, "thread-1", BackupTrigger::Delete).unwrap();
    let backup_id = list_session_backups(&profile).unwrap()[0].snapshots[0]
        .backup_id
        .clone();
    fs::remove_file(&rollout).unwrap();

    let error = restore_session_backup_with_guard(
        &profile,
        &backup_id,
        &RestoreSessionOptions {
            apply: true,
            overwrite_existing: false,
            restore_favorite: false,
        },
        || anyhow::bail!("Codex appears to be running"),
    )
    .unwrap_err();

    assert!(error.to_string().contains("Codex appears to be running"));
    assert!(!rollout.exists());
}

fn write_rollout(path: &Path, id: &str, cwd: &str, provider: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, rollout_body(id, cwd, provider)).unwrap();
}

fn rollout_body(id: &str, cwd: &str, provider: &str) -> String {
    serde_json::to_string(&serde_json::json!({
        "type": "session_meta",
        "payload": {
            "id": id,
            "cwd": cwd,
            "source": "cli",
            "model_provider": provider,
        }
    }))
    .unwrap()
}

fn create_state_db(path: &Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE threads (
            id TEXT PRIMARY KEY,
            rollout_path TEXT,
            created_at INTEGER,
            updated_at INTEGER,
            source TEXT,
            model_provider TEXT,
            cwd TEXT,
            title TEXT,
            sandbox_policy TEXT,
            approval_mode TEXT,
            tokens_used INTEGER NOT NULL DEFAULT 0,
            has_user_event INTEGER NOT NULL DEFAULT 0,
            archived INTEGER NOT NULL DEFAULT 0,
            first_user_message TEXT,
            model TEXT,
            reasoning_effort TEXT,
            created_at_ms INTEGER,
            updated_at_ms INTEGER
        );
        "#,
    )
    .unwrap();
}

fn create_state_db_with_thread(path: &Path, rollout: &Path) {
    create_state_db(path);
    let conn = Connection::open(path).unwrap();
    conn.execute(
        r#"
        INSERT INTO threads (
            id, rollout_path, created_at, updated_at, source, model_provider, cwd,
            title, sandbox_policy, approval_mode, has_user_event, archived,
            first_user_message, model, reasoning_effort, created_at_ms, updated_at_ms
        ) VALUES (
            'thread-1', ?1, 1770790115, 1770794029, 'cli', 'cm',
            '/tmp/project', 'SQLite title', 'workspace-write', 'on-request',
            1, 0, 'hello', 'gpt-5.5', 'high', 1770790115043, 1770794029123
        )
        "#,
        [rollout.to_str().unwrap()],
    )
    .unwrap();
}

fn delete_thread(path: &Path, id: &str) {
    let conn = Connection::open(path).unwrap();
    conn.execute("DELETE FROM threads WHERE id = ?1", [id])
        .unwrap();
}

fn assert_thread_missing(path: &Path, id: &str) {
    let conn = Connection::open(path).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM threads WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}

fn assert_thread_rollout_path(path: &Path, id: &str, expected: &str) {
    let conn = Connection::open(path).unwrap();
    let rollout_path: String = conn
        .query_row("SELECT rollout_path FROM threads WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(rollout_path, expected);
}

fn assert_thread_exists(path: &Path, id: &str) {
    let conn = Connection::open(path).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM threads WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
}
