use std::fs;

use codex_session_manager::migrate::{migrate_provider, ApplyOptions};
use codex_session_manager::profile::CodexProfile;
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn migrates_provider_and_returns_serializable_report() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    let session_dir = profile.sessions_dir().join("2026").join("05").join("06");
    fs::create_dir_all(&session_dir).unwrap();
    let rollout_path = session_dir.join("thread-1.jsonl");
    fs::write(
        &rollout_path,
        r#"{"type":"session_meta","payload":{"id":"thread-1","cwd":"/tmp/demo","source":"cli","model_provider":"codex-auto-review"}}"#,
    )
    .unwrap();

    let report = migrate_provider(
        &profile,
        "codex-auto-review",
        "cm",
        &ApplyOptions { apply: true },
    )
    .unwrap();

    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(value["sqlite_rows"], 1);
    assert_eq!(value["jsonl_files"], 1);
    assert_eq!(read_provider(&profile.state_db_path()), "cm");
    assert!(fs::read_to_string(&rollout_path)
        .unwrap()
        .contains(r#""model_provider":"cm""#));
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
        ) VALUES (
            'thread-1', '/tmp/thread-1.jsonl', 1770790115, 1770794029, 'cli',
            'codex-auto-review', '/mnt/e/code/project-a', 'Thread 1',
            'workspace-write', 'on-request', 1, 0, 'hello', 'gpt-5.5',
            'high', 1770790115043, 1770794029123
        );
        "#,
    )
    .unwrap();
}

fn read_provider(path: &std::path::Path) -> String {
    let conn = Connection::open(path).unwrap();
    conn.query_row(
        "SELECT model_provider FROM threads WHERE id = 'thread-1'",
        [],
        |row| row.get(0),
    )
    .unwrap()
}
