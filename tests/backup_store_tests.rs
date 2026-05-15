use std::fs;
use std::path::Path;

use codex_session_manager::backup_store::{
    create_session_backup, delete_backup_snapshot_with_confirmation, enforce_backup_retention,
    list_session_backups, BackupTrigger,
};
use codex_session_manager::favorites::set_favorite;
use codex_session_manager::profile::CodexProfile;
use codex_session_manager::settings::{save_settings, AppSettings};
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn creating_delete_backup_copies_session_jsonl_and_writes_manifest() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let rollout = profile.sessions_dir().join("thread-1.jsonl");
    write_rollout(&rollout, "thread-1", "/tmp/project", "cm");
    create_state_db(&profile.state_db_path(), &rollout);
    write_index(&profile, "thread-1", "Index title");
    set_favorite(&profile, "thread-1", true).unwrap();

    let manifest = create_session_backup(&profile, "thread-1", BackupTrigger::Delete).unwrap();

    assert_eq!(manifest.session_id, "thread-1");
    assert_eq!(manifest.trigger, BackupTrigger::Delete);
    assert_eq!(
        manifest.original_session_path.as_deref(),
        Some(rollout.to_str().unwrap())
    );
    assert_eq!(manifest.index_entries.len(), 1);
    assert_eq!(manifest.sqlite_thread.as_ref().unwrap().id, "thread-1");
    assert!(manifest.favorite);
    assert!(manifest.local_session_existed_at_backup_time);
    assert!(manifest.size_bytes > 0);
    let copied = manifest.backup_session_path.as_ref().unwrap();
    assert_eq!(
        fs::read_to_string(copied).unwrap(),
        fs::read_to_string(rollout).unwrap()
    );
}

#[test]
fn creating_backup_for_jsonl_only_session_works() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let rollout = profile.sessions_dir().join("jsonl-only.jsonl");
    write_rollout(&rollout, "jsonl-only", "/tmp/jsonl", "cm");

    let manifest = create_session_backup(&profile, "jsonl-only", BackupTrigger::Delete).unwrap();

    assert_eq!(manifest.session_id, "jsonl-only");
    assert!(manifest.sqlite_thread.is_none());
    assert!(Path::new(manifest.backup_session_path.as_ref().unwrap()).exists());
}

#[test]
fn delete_or_edit_backup_without_local_jsonl_returns_clear_error() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();

    let error = create_session_backup(&profile, "missing", BackupTrigger::Delete).unwrap_err();

    assert!(error.to_string().contains("local JSONL"));
}

#[test]
fn listing_backups_groups_by_session_and_sorts_snapshots_newest_first() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let rollout = profile.sessions_dir().join("thread-1.jsonl");
    write_rollout(&rollout, "thread-1", "/tmp/project", "cm");

    let first = create_session_backup(&profile, "thread-1", BackupTrigger::Manual).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let second = create_session_backup(&profile, "thread-1", BackupTrigger::Delete).unwrap();

    let rows = list_session_backups(&profile).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].session_id, "thread-1");
    assert_eq!(rows[0].snapshots.len(), 2);
    assert!(rows[0].local_exists);
    assert_eq!(rows[0].snapshots[0].created_at_unix, second.created_at_unix);
    assert_eq!(rows[0].snapshots[1].created_at_unix, first.created_at_unix);
}

#[test]
fn delete_backups_are_grouped_under_trash_when_latest_snapshot_is_delete() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let rollout = profile.sessions_dir().join("thread-1.jsonl");
    write_rollout(&rollout, "thread-1", "/tmp/project", "cm");

    create_session_backup(&profile, "thread-1", BackupTrigger::Manual).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    create_session_backup(&profile, "thread-1", BackupTrigger::Delete).unwrap();

    let rows = list_session_backups(&profile).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].group.as_deref(), Some("回收站"));
}

#[test]
fn listing_backup_marks_local_exists_false_when_jsonl_is_missing() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let rollout = profile.sessions_dir().join("thread-1.jsonl");
    write_rollout(&rollout, "thread-1", "/tmp/project", "cm");
    create_session_backup(&profile, "thread-1", BackupTrigger::Delete).unwrap();
    fs::remove_file(&rollout).unwrap();

    let rows = list_session_backups(&profile).unwrap();

    assert_eq!(rows.len(), 1);
    assert!(!rows[0].local_exists);
}

#[test]
fn max_count_prunes_oldest_non_protected_backups() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let mut settings = AppSettings::default();
    settings.backup.max_count = Some(1);
    save_settings(&profile, &settings).unwrap();
    let rollout = profile.sessions_dir().join("thread-1.jsonl");
    write_rollout(&rollout, "thread-1", "/tmp/project", "cm");

    create_session_backup(&profile, "thread-1", BackupTrigger::Manual).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    create_session_backup(&profile, "thread-1", BackupTrigger::Manual).unwrap();

    let rows = list_session_backups(&profile).unwrap();
    assert_eq!(rows[0].snapshots.len(), 1);
}

#[test]
fn max_age_days_prunes_expired_backups() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let rollout = profile.sessions_dir().join("thread-1.jsonl");
    write_rollout(&rollout, "thread-1", "/tmp/project", "cm");
    let manifest = create_session_backup(&profile, "thread-1", BackupTrigger::Manual).unwrap();
    rewrite_manifest_created_at(&manifest, 1);
    let mut settings = AppSettings::default();
    settings.backup.max_age_days = Some(1);
    save_settings(&profile, &settings).unwrap();

    let report = enforce_backup_retention(&profile).unwrap();

    assert_eq!(report.deleted_backup_dirs.len(), 1);
    assert!(list_session_backups(&profile).unwrap().is_empty());
}

#[test]
fn max_bytes_prunes_until_under_limit() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let rollout = profile.sessions_dir().join("thread-1.jsonl");
    write_rollout(&rollout, "thread-1", "/tmp/project", "cm");
    create_session_backup(&profile, "thread-1", BackupTrigger::Manual).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    create_session_backup(&profile, "thread-1", BackupTrigger::Manual).unwrap();
    let total_before = backup_total_size(&profile);
    let mut settings = AppSettings::default();
    settings.backup.max_bytes = Some(total_before - 1);
    save_settings(&profile, &settings).unwrap();

    let report = enforce_backup_retention(&profile).unwrap();

    assert!(!report.deleted_backup_dirs.is_empty());
    assert!(report.total_bytes_after < report.total_bytes_before);
}

#[test]
fn automatic_pruning_skips_unique_archive_when_configured() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let rollout = profile.sessions_dir().join("thread-1.jsonl");
    write_rollout(&rollout, "thread-1", "/tmp/project", "cm");
    create_session_backup(&profile, "thread-1", BackupTrigger::Manual).unwrap();
    fs::remove_file(&rollout).unwrap();
    let mut settings = AppSettings::default();
    settings.backup.max_count = Some(0);
    settings.backup.skip_unique_archive_on_auto_prune = true;
    save_settings(&profile, &settings).unwrap();

    let report = enforce_backup_retention(&profile).unwrap();

    assert!(report.deleted_backup_dirs.is_empty());
    assert_eq!(report.skipped_unique_archives, vec!["thread-1"]);
    assert!(!report.warnings.is_empty());
    assert_eq!(
        list_session_backups(&profile).unwrap()[0].snapshots.len(),
        1
    );
}

#[test]
fn manual_deletion_of_last_archive_requires_confirmation() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let rollout = profile.sessions_dir().join("thread-1.jsonl");
    write_rollout(&rollout, "thread-1", "/tmp/project", "cm");
    create_session_backup(&profile, "thread-1", BackupTrigger::Manual).unwrap();
    fs::remove_file(&rollout).unwrap();
    let backup_id = list_session_backups(&profile).unwrap()[0].snapshots[0]
        .backup_id
        .clone();

    let error = delete_backup_snapshot_with_confirmation(&profile, &backup_id, false).unwrap_err();
    assert!(error.to_string().contains("last backup"));

    let report = delete_backup_snapshot_with_confirmation(&profile, &backup_id, true).unwrap();
    assert!(report.deleted);
    assert!(list_session_backups(&profile).unwrap().is_empty());
}

#[test]
fn backup_creation_fails_before_writing_when_max_bytes_is_too_small() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let rollout = profile.sessions_dir().join("thread-1.jsonl");
    write_rollout(&rollout, "thread-1", "/tmp/project", "cm");
    let mut settings = AppSettings::default();
    settings.backup.max_bytes = Some(1);
    save_settings(&profile, &settings).unwrap();

    let error = create_session_backup(&profile, "thread-1", BackupTrigger::Manual).unwrap_err();

    assert!(error.to_string().contains("max_bytes"));
    assert!(list_session_backups(&profile).unwrap().is_empty());
}

fn write_rollout(path: &Path, id: &str, cwd: &str, provider: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!(
            r#"{{"type":"session_meta","payload":{{"id":"{id}","cwd":"{cwd}","source":"cli","model_provider":"{provider}"}}}}"#
        ),
    )
    .unwrap();
}

fn write_index(profile: &CodexProfile, id: &str, title: &str) {
    fs::write(
        profile.session_index_path(),
        format!(r#"{{"id":"{id}","thread_name":"{title}","updated_at":"2026-05-06T00:00:00Z"}}"#)
            + "\n",
    )
    .unwrap();
}

fn create_state_db(path: &Path, rollout: &Path) {
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
            '/tmp/project', 'SQLite title', 'workspace-write', 'on-request',
            1, 0, 'hello', 'gpt-5.5', 'high', 1770790115043, 1770794029123
        );
        "#,
        rollout.display()
    ))
    .unwrap();
}

fn rewrite_manifest_created_at(
    manifest: &codex_session_manager::backup_store::SessionBackupManifest,
    created_at_unix: i64,
) {
    let manifest_path = Path::new(manifest.backup_session_path.as_ref().unwrap())
        .parent()
        .unwrap()
        .join("manifest.json");
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    value["created_at_unix"] = serde_json::Value::Number(created_at_unix.into());
    fs::write(&manifest_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}

fn backup_total_size(profile: &CodexProfile) -> u64 {
    walkdir::WalkDir::new(profile.codex_home.join("backups"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.metadata().unwrap().len())
        .sum()
}
