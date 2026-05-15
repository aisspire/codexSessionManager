use std::fs;

use codex_session_manager::profile::CodexProfile;
use codex_session_manager::session_list::{list_sessions, SessionListFilter};
use codex_session_manager::session_ops::{delete_sessions_with_guard, SessionApplyOptions};
use codex_session_manager::trash::TrashManifest;
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn delete_moves_rollout_files_to_tool_trash_and_deletes_threads() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    fs::create_dir_all(profile.sessions_dir()).unwrap();
    let rollout = profile.sessions_dir().join("thread-1.jsonl");
    fs::write(
        &rollout,
        r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/project","source":"cli","model_provider":"cm"}}"#,
    )
    .unwrap();
    create_state_db(&profile.state_db_path(), &rollout);
    let options = SessionApplyOptions { apply: true };

    let report =
        delete_sessions_with_guard(&profile, &["thread-1".to_string()], &options, || Ok(()))
            .unwrap();

    assert!(report.applied);
    assert_eq!(report.sqlite_rows, 1);
    assert_eq!(report.backup_manifests.len(), 1);
    assert!(std::path::Path::new(&report.backup_manifests[0]).exists());
    assert_eq!(report.trash_manifest.as_ref().unwrap().entries.len(), 1);
    assert!(!rollout.exists());
    assert_thread_missing(&profile.state_db_path(), "thread-1");
    assert!(list_sessions(&profile, &SessionListFilter::default())
        .unwrap()
        .iter()
        .all(|session| session.id != "thread-1"));

    let manifest_path = dir
        .path()
        .join(report.trash_manifest_path.as_ref().unwrap());
    let manifest: TrashManifest =
        serde_json::from_str(&fs::read_to_string(manifest_path).unwrap()).unwrap();
    assert_eq!(manifest.entries[0].session_id, "thread-1");
    assert!(std::path::Path::new(&manifest.entries[0].trashed_path).exists());
}

#[test]
fn delete_jsonl_only_session_creates_backup_and_removes_index_entry() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    fs::create_dir_all(profile.sessions_dir()).unwrap();
    let rollout = profile.sessions_dir().join("jsonl-only.jsonl");
    fs::write(
        &rollout,
        r#"{"type":"session_meta","payload":{"id":"jsonl-only","cwd":"/tmp/jsonl","source":"cli","model_provider":"cm"}}"#,
    )
    .unwrap();
    fs::write(
        profile.session_index_path(),
        "{\"id\":\"jsonl-only\",\"thread_name\":\"JSONL only\",\"updated_at\":\"2026-05-06T00:00:00Z\"}\n",
    )
    .unwrap();

    let report = delete_sessions_with_guard(
        &profile,
        &["jsonl-only".to_string()],
        &SessionApplyOptions { apply: true },
        || Ok(()),
    )
    .unwrap();

    assert!(report.applied);
    assert_eq!(report.sqlite_rows, 0);
    assert_eq!(report.index_entries, 1);
    assert_eq!(report.backup_manifests.len(), 1);
    assert!(!rollout.exists());
    assert!(fs::read_to_string(profile.session_index_path())
        .unwrap()
        .trim()
        .is_empty());
}

fn create_state_db(path: &std::path::Path, rollout: &std::path::Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(&format!(
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

            INSERT INTO threads (
                id, rollout_path, created_at, updated_at, source, model_provider, cwd,
                title, sandbox_policy, approval_mode, has_user_event, archived,
                first_user_message, model, reasoning_effort, created_at_ms, updated_at_ms
            ) VALUES (
                'thread-1', '{}', 1770790115, 1770794029, 'cli', 'cm',
                '/mnt/e/code/project-a', 'Thread 1', 'workspace-write', 'on-request',
                1, 0, 'hello', 'gpt-5.5', 'high', 1770790115043, 1770794029123
            );
            "#,
        rollout.display()
    ))
    .unwrap();
}

fn assert_thread_missing(path: &std::path::Path, id: &str) {
    let conn = Connection::open(path).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM threads WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}
