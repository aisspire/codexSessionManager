use std::fs;

use codex_session_manager::favorites::set_favorite;
use codex_session_manager::profile::CodexProfile;
use codex_session_manager::session_list::{
    list_sessions, ArchivedFilter, SessionListFilter, SessionScope,
};
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
    assert!(!sessions[0].favorite);
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

#[test]
fn lists_sessions_present_only_in_rollout_and_session_index() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    let rollout_path = profile
        .sessions_dir()
        .join("2026")
        .join("05")
        .join("08")
        .join("rollout-2026-05-08T00-56-06-index-only.jsonl");
    create_rollout(&rollout_path, "index-only", "/mnt/e/code/jsonl-only", "cm");
    fs::write(
        profile.session_index_path(),
        concat!(
            r#"{"id":"active-1","thread_name":"Active session","updated_at":"2026-05-06T00:00:00Z"}"#,
            "\n",
            r#"{"id":"index-only","thread_name":"Index only session","updated_at":"2026-05-08T00:56:06Z"}"#,
            "\n",
        ),
    )
    .unwrap();

    let sessions = list_sessions(
        &profile,
        &SessionListFilter {
            search: Some("jsonl-only".to_string()),
            archived: ArchivedFilter::All,
            ..SessionListFilter::default()
        },
    )
    .unwrap();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "index-only");
    assert_eq!(sessions[0].title.as_deref(), Some("Index only session"));
    assert_eq!(
        sessions[0].project.as_deref(),
        Some("/mnt/e/code/jsonl-only")
    );
    assert_eq!(sessions[0].provider.as_deref(), Some("cm"));
    assert_eq!(sessions[0].source.as_deref(), Some("cli"));
    assert!(!sessions[0].favorite);
    assert_eq!(
        sessions[0].rollout_path.as_deref(),
        Some(rollout_path.to_str().unwrap())
    );
    assert!(sessions[0].in_session_index);
    assert!(!sessions[0].archived);
}

#[test]
fn lists_sessions_from_archived_rollout_directory_as_archived() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    let active_rollout_path = profile.sessions_dir().join("active-1.jsonl");
    create_rollout(
        &active_rollout_path,
        "active-1",
        "/mnt/e/code/project-a",
        "cm",
    );
    let archived_rollout_path = profile
        .archived_sessions_dir()
        .join("rollout-2026-05-06T20-43-11-archived-1.jsonl");
    create_rollout(
        &archived_rollout_path,
        "archived-1",
        "/mnt/e/code/project-b",
        "openai",
    );
    set_archived(&profile.state_db_path(), "archived-1", false);

    let sessions = list_sessions(
        &profile,
        &SessionListFilter {
            archived: ArchivedFilter::Archived,
            search: Some("Archived title".to_string()),
            ..SessionListFilter::default()
        },
    )
    .unwrap();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "archived-1");
    assert!(sessions[0].archived);
    assert_eq!(
        sessions[0].rollout_path.as_deref(),
        Some(archived_rollout_path.to_str().unwrap())
    );
}

#[test]
fn favorite_scope_returns_only_favorite_sessions() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    set_favorite(&profile, "archived-1", true).unwrap();

    let sessions = list_sessions(
        &profile,
        &SessionListFilter {
            archived: SessionScope::Favorite,
            ..SessionListFilter::default()
        },
    )
    .unwrap();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "archived-1");
    assert!(sessions[0].favorite);
}

#[test]
fn jsonl_only_sessions_can_be_marked_favorite() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    let rollout_path = profile.sessions_dir().join("jsonl-only.jsonl");
    create_rollout(&rollout_path, "jsonl-only", "/mnt/e/code/jsonl-only", "cm");
    set_favorite(&profile, "jsonl-only", true).unwrap();

    let sessions = list_sessions(
        &profile,
        &SessionListFilter {
            archived: SessionScope::Favorite,
            ..SessionListFilter::default()
        },
    )
    .unwrap();

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "jsonl-only");
    assert!(sessions[0].favorite);
    assert_eq!(
        sessions[0].rollout_path.as_deref(),
        Some(rollout_path.to_str().unwrap())
    );
}

#[test]
fn title_priority_is_index_then_sqlite_then_first_user_message() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());

    fs::write(
        profile.session_index_path(),
        concat!(
            r#"{"id":"active-1","thread_name":"Index title","updated_at":"2026-05-06T00:00:00Z"}"#,
            "\n"
        ),
    )
    .unwrap();
    let with_index = list_sessions(
        &profile,
        &SessionListFilter {
            archived: ArchivedFilter::All,
            search: Some("active-1".to_string()),
            ..SessionListFilter::default()
        },
    )
    .unwrap();
    assert_eq!(with_index[0].title.as_deref(), Some("Index title"));

    fs::write(profile.session_index_path(), "").unwrap();
    let with_sqlite = list_sessions(
        &profile,
        &SessionListFilter {
            archived: ArchivedFilter::All,
            search: Some("active-1".to_string()),
            ..SessionListFilter::default()
        },
    )
    .unwrap();
    assert_eq!(with_sqlite[0].title.as_deref(), Some("SQLite title"));

    let conn = Connection::open(profile.state_db_path()).unwrap();
    conn.execute("UPDATE threads SET title = NULL WHERE id = 'active-1'", [])
        .unwrap();
    let with_first_message = list_sessions(
        &profile,
        &SessionListFilter {
            archived: ArchivedFilter::All,
            search: Some("active-1".to_string()),
            ..SessionListFilter::default()
        },
    )
    .unwrap();
    assert_eq!(with_first_message[0].title.as_deref(), Some("hello"));
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

fn set_archived(path: &std::path::Path, id: &str, archived: bool) {
    let conn = Connection::open(path).unwrap();
    conn.execute(
        "UPDATE threads SET archived = ?1 WHERE id = ?2",
        rusqlite::params![if archived { 1 } else { 0 }, id],
    )
    .unwrap();
}
