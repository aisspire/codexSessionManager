use std::fs;

use codex_session_manager::profile::CodexProfile;
use codex_session_manager::session_list::{list_sessions, ArchivedFilter, SessionListFilter};
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn lists_sessions_filtered_by_project_provider_model_and_archived_state() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    create_rollout(
        &profile.sessions_dir().join("active.jsonl"),
        "active-1",
        "/mnt/e/code/project-a",
        "cm",
    );
    create_rollout(
        &profile.sessions_dir().join("archived.jsonl"),
        "archived-1",
        "/mnt/e/code/project-b",
        "openai",
    );
    fs::write(
        profile.session_index_path(),
        concat!(
            r#"{"id":"active-1","thread_name":"Active session","updated_at":"2026-05-06T00:00:00Z"}"#,
            "\n"
        ),
    )
    .unwrap();

    let filter = SessionListFilter {
        project: Some("/mnt/e/code/project-a".to_string()),
        provider: Some("cm".to_string()),
        model: Some("gpt-5.5".to_string()),
        archived: ArchivedFilter::Active,
        ..SessionListFilter::default()
    };

    let sessions = list_sessions(&profile, &filter).unwrap();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "active-1");
    assert_eq!(
        sessions[0].project.as_deref(),
        Some("/mnt/e/code/project-a")
    );
    assert_eq!(sessions[0].provider.as_deref(), Some("cm"));
    assert_eq!(sessions[0].model.as_deref(), Some("gpt-5.5"));
    assert!(!sessions[0].archived);
    assert_eq!(sessions[0].title.as_deref(), Some("Active session"));

    let all = list_sessions(
        &profile,
        &SessionListFilter {
            archived: ArchivedFilter::All,
            ..SessionListFilter::default()
        },
    )
    .unwrap();
    assert_eq!(all.len(), 2);
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
            'active-1', '', 1770790115, 1770794029, 'cli', 'cm',
            '/mnt/e/code/project-a', 'SQLite title', 'workspace-write', 'on-request',
            1, 0, 'hello', 'gpt-5.5', 'high', 1770790115043, 1770794029123
        ),
        (
            'archived-1', '', 1770790115, 1770794029, 'vscode', 'openai',
            '/mnt/e/code/project-b', 'Archived title', 'workspace-write', 'on-request',
            1, 1, 'old', 'gpt-4.1', 'medium', 1770790115043, 1770794029122
        );
        "#,
    )
    .unwrap();
}

fn create_rollout(path: &std::path::Path, id: &str, cwd: &str, provider: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!(
            r#"{{"type":"session_meta","payload":{{"id":"{id}","cwd":"{cwd}","source":"cli","model_provider":"{provider}"}}}}"#
        ),
    )
    .unwrap();
}
