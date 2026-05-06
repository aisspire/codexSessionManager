use codex_session_manager::state_db::StateDb;
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn reads_threads_with_integer_timestamps_from_current_schema() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("state.sqlite");
    let conn = Connection::open(&db_path).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE threads (
            id TEXT PRIMARY KEY,
            rollout_path TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            source TEXT NOT NULL,
            model_provider TEXT NOT NULL,
            cwd TEXT NOT NULL,
            title TEXT NOT NULL,
            sandbox_policy TEXT NOT NULL,
            approval_mode TEXT NOT NULL,
            tokens_used INTEGER NOT NULL DEFAULT 0,
            has_user_event INTEGER NOT NULL DEFAULT 0,
            archived INTEGER NOT NULL DEFAULT 0,
            first_user_message TEXT NOT NULL DEFAULT '',
            model TEXT,
            reasoning_effort TEXT,
            created_at_ms INTEGER,
            updated_at_ms INTEGER
        );

        INSERT INTO threads (
            id,
            rollout_path,
            created_at,
            updated_at,
            source,
            model_provider,
            cwd,
            title,
            sandbox_policy,
            approval_mode,
            has_user_event,
            archived,
            first_user_message,
            model,
            reasoning_effort,
            created_at_ms,
            updated_at_ms
        ) VALUES (
            'thread-1',
            '/mnt/c/Users/14139/.codex/sessions/thread-1.jsonl',
            1770790115,
            1770794029,
            'cli',
            'cm',
            '/mnt/e/code/demo',
            'Demo',
            'workspace-write',
            'on-request',
            1,
            0,
            'hello',
            'gpt-5.5',
            'high',
            1770790115043,
            1770794029123
        );
        "#,
    )
    .unwrap();

    let db = StateDb::open(&db_path).unwrap();
    let threads = db.read_threads().unwrap();

    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].id, "thread-1");
    assert_eq!(
        threads[0].created_at.as_deref(),
        Some("2026-02-11T06:08:35Z")
    );
    assert_eq!(
        threads[0].updated_at.as_deref(),
        Some("2026-02-11T07:13:49Z")
    );
    assert_eq!(threads[0].created_at_ms, Some(1770790115043));
    assert_eq!(threads[0].updated_at_ms, Some(1770794029123));
}
