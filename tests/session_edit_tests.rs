use std::fs;

use codex_session_manager::migrate::{edit_selected_sessions, ApplyOptions, SessionEdit};
use codex_session_manager::profile::CodexProfile;
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn edits_provider_and_project_for_selected_sessions_only() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    let session_dir = profile.sessions_dir().join("2026").join("05").join("06");
    fs::create_dir_all(&session_dir).unwrap();
    let selected_path = session_dir.join("thread-1.jsonl");
    let untouched_path = session_dir.join("thread-2.jsonl");
    write_rollout(&selected_path, "thread-1", "codex-auto-review", "/tmp/old");
    write_rollout(&untouched_path, "thread-2", "codex-auto-review", "/tmp/old");

    let report = edit_selected_sessions(
        &profile,
        &["thread-1".to_string()],
        &SessionEdit {
            provider: Some("cm".to_string()),
            project: Some("/tmp/new".to_string()),
        },
        &ApplyOptions {
            apply: true,
            backup: false,
            include_sessions_backup: false,
        },
    )
    .unwrap();

    assert_eq!(report.sqlite_rows, 1);
    assert_eq!(report.jsonl_files, 1);
    assert_eq!(
        read_thread(&profile.state_db_path(), "thread-1"),
        ("cm".to_string(), "/tmp/new".to_string())
    );
    assert_eq!(
        read_thread(&profile.state_db_path(), "thread-2"),
        ("codex-auto-review".to_string(), "/tmp/old".to_string())
    );
    assert!(fs::read_to_string(&selected_path)
        .unwrap()
        .contains(r#""model_provider":"cm""#));
    assert!(fs::read_to_string(&selected_path)
        .unwrap()
        .contains(r#""cwd":"/tmp/new""#));
    assert!(fs::read_to_string(&untouched_path)
        .unwrap()
        .contains(r#""model_provider":"codex-auto-review""#));
}

fn write_rollout(path: &std::path::Path, id: &str, provider: &str, cwd: &str) {
    fs::write(
        path,
        format!(
            r#"{{"type":"session_meta","payload":{{"id":"{id}","cwd":"{cwd}","source":"cli","model_provider":"{provider}"}}}}"#
        ),
    )
    .unwrap();
}

fn create_state_db(path: &std::path::Path) {
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

        INSERT INTO threads (
            id, rollout_path, created_at, updated_at, source, model_provider, cwd,
            title, sandbox_policy, approval_mode, has_user_event, archived,
            first_user_message, model, reasoning_effort, created_at_ms, updated_at_ms
        ) VALUES
        (
            'thread-1', '/tmp/thread-1.jsonl', 1770790115, 1770794029, 'cli',
            'codex-auto-review', '/tmp/old', 'Thread 1',
            'workspace-write', 'on-request', 1, 0, 'hello', 'gpt-5.5',
            'high', 1770790115043, 1770794029123
        ),
        (
            'thread-2', '/tmp/thread-2.jsonl', 1770790115, 1770794029, 'cli',
            'codex-auto-review', '/tmp/old', 'Thread 2',
            'workspace-write', 'on-request', 1, 0, 'hello', 'gpt-5.5',
            'high', 1770790115043, 1770794029122
        );
        "#,
    )
    .unwrap();
}

fn read_thread(path: &std::path::Path, id: &str) -> (String, String) {
    let conn = Connection::open(path).unwrap();
    conn.query_row(
        "SELECT model_provider, cwd FROM threads WHERE id = ?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .unwrap()
}
