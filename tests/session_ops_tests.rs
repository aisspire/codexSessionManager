use codex_session_manager::profile::CodexProfile;
use codex_session_manager::session_ops::{
    archive_sessions_with_guard, restore_sessions_with_guard, SessionApplyOptions,
};
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn archives_and_restores_selected_sessions() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    let ids = vec!["thread-1".to_string()];
    let options = SessionApplyOptions {
        apply: true,
        backup: false,
        include_sessions_backup: false,
    };

    let archive = archive_sessions_with_guard(&profile, &ids, &options, || Ok(())).unwrap();
    assert!(archive.applied);
    assert_eq!(archive.sqlite_rows, 1);
    assert_archived(&profile.state_db_path(), "thread-1", true);
    assert_archived(&profile.state_db_path(), "thread-2", false);

    let restore = restore_sessions_with_guard(&profile, &ids, &options, || Ok(())).unwrap();
    assert!(restore.applied);
    assert_eq!(restore.sqlite_rows, 1);
    assert_archived(&profile.state_db_path(), "thread-1", false);
}

#[test]
fn refuses_to_archive_when_codex_is_running() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    let options = SessionApplyOptions {
        apply: true,
        backup: false,
        include_sessions_backup: false,
    };

    let result = archive_sessions_with_guard(&profile, &["thread-1".to_string()], &options, || {
        anyhow::bail!("Codex appears to be running")
    });

    assert!(result.is_err());
    assert_archived(&profile.state_db_path(), "thread-1", false);
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
            'thread-1', '/tmp/thread-1.jsonl', 1770790115, 1770794029, 'cli', 'cm',
            '/mnt/e/code/project-a', 'Thread 1', 'workspace-write', 'on-request',
            1, 0, 'hello', 'gpt-5.5', 'high', 1770790115043, 1770794029123
        ),
        (
            'thread-2', '/tmp/thread-2.jsonl', 1770790115, 1770794029, 'cli', 'cm',
            '/mnt/e/code/project-a', 'Thread 2', 'workspace-write', 'on-request',
            1, 0, 'hello', 'gpt-5.5', 'high', 1770790115043, 1770794029122
        );
        "#,
    )
    .unwrap();
}

fn assert_archived(path: &std::path::Path, id: &str, expected: bool) {
    let conn = Connection::open(path).unwrap();
    let archived: i64 = conn
        .query_row("SELECT archived FROM threads WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(archived != 0, expected);
}
