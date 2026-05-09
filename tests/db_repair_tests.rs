use std::fs;
use std::path::Path;

use codex_session_manager::db_repair::{
    apply_database_repairs_with_guard, preview_database_repairs, DatabaseRepairKind,
    DatabaseRepairOptions,
};
use codex_session_manager::profile::CodexProfile;
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn previews_and_applies_jsonl_only_thread_rows() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    let rollout_path = profile.sessions_dir().join("jsonl-only.jsonl");
    create_rollout(&rollout_path, "jsonl-only", "E:\\code\\jsonl-only", "cm");
    fs::write(
        profile.session_index_path(),
        r#"{"id":"jsonl-only","thread_name":"JSONL only","updated_at":"2026-05-08T00:56:06Z"}"#,
    )
    .unwrap();

    let preview = preview_database_repairs(&profile).unwrap();

    assert_eq!(preview.items.len(), 1);
    assert_eq!(preview.items[0].kind, DatabaseRepairKind::MissingThreadRow);
    assert_eq!(preview.items[0].session_id, "jsonl-only");
    assert!(preview.items[0].applicable);

    let selected = vec![preview.items[0].id.clone()];
    let applied =
        apply_database_repairs_with_guard(&profile, &DatabaseRepairOptions { selected }, || Ok(()))
            .unwrap();

    assert_eq!(applied.applied_items, 1);
    let rows = read_thread_values(&profile.state_db_path(), "jsonl-only");
    assert_eq!(
        rows.rollout_path.as_deref(),
        Some(rollout_path.to_str().unwrap())
    );
    assert_eq!(rows.cwd.as_deref(), Some("E:\\code\\jsonl-only"));
    assert_eq!(rows.provider.as_deref(), Some("cm"));
    assert!(!rows.archived);
    assert!(profile.codex_home.join("backups").exists());
}

#[test]
fn repairs_empty_or_missing_rollout_path_to_unique_jsonl_path() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    insert_thread(
        &profile.state_db_path(),
        "broken-path",
        "",
        false,
        "E:\\code\\broken",
        "cm",
    );
    let rollout_path = profile.sessions_dir().join("broken-path.jsonl");
    create_rollout(&rollout_path, "broken-path", "E:\\code\\broken", "cm");

    let preview = preview_database_repairs(&profile).unwrap();

    assert_eq!(preview.items.len(), 1);
    assert_eq!(preview.items[0].kind, DatabaseRepairKind::RepairRolloutPath);
    assert_eq!(
        preview.items[0].after.as_deref(),
        Some(rollout_path.to_str().unwrap())
    );

    apply_database_repairs_with_guard(
        &profile,
        &DatabaseRepairOptions {
            selected: vec![preview.items[0].id.clone()],
        },
        || Ok(()),
    )
    .unwrap();

    let rows = read_thread_values(&profile.state_db_path(), "broken-path");
    assert_eq!(
        rows.rollout_path.as_deref(),
        Some(rollout_path.to_str().unwrap())
    );
}

#[test]
fn synchronizes_archived_state_from_unique_rollout_directory() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    insert_thread(
        &profile.state_db_path(),
        "archived-one",
        "old.jsonl",
        false,
        "E:\\code\\archived",
        "openai",
    );
    let rollout_path = profile.archived_sessions_dir().join("archived-one.jsonl");
    create_rollout(
        &rollout_path,
        "archived-one",
        "E:\\code\\archived",
        "openai",
    );

    let preview = preview_database_repairs(&profile).unwrap();

    assert!(preview
        .items
        .iter()
        .any(|item| item.kind == DatabaseRepairKind::SyncArchivedState
            && item.session_id == "archived-one"
            && item.after.as_deref() == Some("archived")));

    let selected = preview
        .items
        .iter()
        .filter(|item| item.kind == DatabaseRepairKind::SyncArchivedState)
        .map(|item| item.id.clone())
        .collect();
    apply_database_repairs_with_guard(&profile, &DatabaseRepairOptions { selected }, || Ok(()))
        .unwrap();

    let rows = read_thread_values(&profile.state_db_path(), "archived-one");
    assert!(rows.archived);
}

#[test]
fn normalizes_wsl_mount_rollout_path_to_current_jsonl_path() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    let rollout_path = profile.sessions_dir().join("wsl-path.jsonl");
    create_rollout(&rollout_path, "wsl-path", "E:\\code\\wsl", "cm");
    insert_thread(
        &profile.state_db_path(),
        "wsl-path",
        "/mnt/e/Users/example/.codex/sessions/wsl-path.jsonl",
        false,
        "E:\\code\\wsl",
        "cm",
    );

    let preview = preview_database_repairs(&profile).unwrap();

    assert_eq!(preview.items.len(), 1);
    assert_eq!(
        preview.items[0].kind,
        DatabaseRepairKind::NormalizeRolloutPath
    );
    assert_eq!(
        preview.items[0].before.as_deref(),
        Some("/mnt/e/Users/example/.codex/sessions/wsl-path.jsonl")
    );
    assert_eq!(
        preview.items[0].after.as_deref(),
        Some(rollout_path.to_str().unwrap())
    );
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

fn insert_thread(
    db_path: &Path,
    id: &str,
    rollout_path: &str,
    archived: bool,
    cwd: &str,
    provider: &str,
) {
    let conn = Connection::open(db_path).unwrap();
    conn.execute(
        r#"
        INSERT INTO threads (
            id, rollout_path, created_at, updated_at, source, model_provider, cwd,
            title, sandbox_policy, approval_mode, has_user_event, archived,
            first_user_message, model, reasoning_effort, created_at_ms, updated_at_ms
        ) VALUES (?1, ?2, 1770790115, 1770794029, 'cli', ?5, ?4,
            '', 'workspace-write', 'on-request', 1, ?3,
            '', 'gpt-5.5', 'high', 1770790115043, 1770794029123)
        "#,
        rusqlite::params![
            id,
            rollout_path,
            if archived { 1 } else { 0 },
            cwd,
            provider
        ],
    )
    .unwrap();
}

fn create_rollout(path: &Path, id: &str, cwd: &str, provider: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let line = serde_json::json!({
        "type": "session_meta",
        "payload": {
            "id": id,
            "cwd": cwd,
            "source": "cli",
            "model_provider": provider,
        }
    });
    fs::write(path, serde_json::to_string(&line).unwrap()).unwrap();
}

struct ThreadValues {
    rollout_path: Option<String>,
    cwd: Option<String>,
    provider: Option<String>,
    archived: bool,
}

fn read_thread_values(db_path: &Path, id: &str) -> ThreadValues {
    let conn = Connection::open(db_path).unwrap();
    conn.query_row(
        "SELECT rollout_path, cwd, model_provider, archived FROM threads WHERE id = ?1",
        [id],
        |row| {
            Ok(ThreadValues {
                rollout_path: row.get(0)?,
                cwd: row.get(1)?,
                provider: row.get(2)?,
                archived: row.get::<_, i64>(3)? != 0,
            })
        },
    )
    .unwrap()
}
