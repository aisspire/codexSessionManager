use std::fs::{self, FileTimes, OpenOptions};
use std::time::{Duration, SystemTime};

use codex_session_manager::profile::CodexProfile;
use codex_session_manager::session_ops::{
    archive_sessions_with_guard, refresh_session_updated_at_with_guard,
    restore_sessions_with_guard, SessionApplyOptions,
};
use rusqlite::{params, Connection};
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
fn archives_and_restores_touch_rollout_files_to_notify_codex() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    let rollout_path = profile.sessions_dir().join("thread-1.jsonl");
    fs::create_dir_all(profile.sessions_dir()).unwrap();
    write_rollout(&rollout_path, "thread-1");
    set_rollout_path(&profile.state_db_path(), "thread-1", &rollout_path);
    let archived_path = profile.archived_sessions_dir().join("thread-1.jsonl");
    let old_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1_770_790_000);
    let options = SessionApplyOptions {
        apply: true,
        backup: false,
        include_sessions_backup: false,
    };

    set_file_times(&rollout_path, old_time);
    archive_sessions_with_guard(&profile, &["thread-1".to_string()], &options, || Ok(()))
        .unwrap();
    assert!(archived_path.metadata().unwrap().modified().unwrap() > old_time);

    set_file_times(&archived_path, old_time);
    restore_sessions_with_guard(&profile, &["thread-1".to_string()], &options, || Ok(()))
        .unwrap();
    assert!(rollout_path.metadata().unwrap().modified().unwrap() > old_time);
}

#[test]
fn archives_and_restores_move_rollout_files_between_codex_directories() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    let rollout_path = profile
        .sessions_dir()
        .join("2026")
        .join("05")
        .join("07")
        .join("rollout-2026-05-07T21-52-45-thread-1.jsonl");
    fs::create_dir_all(rollout_path.parent().unwrap()).unwrap();
    write_rollout(&rollout_path, "thread-1");
    set_rollout_path(&profile.state_db_path(), "thread-1", &rollout_path);
    let archived_path = profile
        .archived_sessions_dir()
        .join("rollout-2026-05-07T21-52-45-thread-1.jsonl");
    let options = SessionApplyOptions {
        apply: true,
        backup: false,
        include_sessions_backup: false,
    };

    archive_sessions_with_guard(&profile, &["thread-1".to_string()], &options, || Ok(()))
        .unwrap();

    assert!(!rollout_path.exists());
    assert!(archived_path.exists());
    assert_archived(&profile.state_db_path(), "thread-1", true);

    restore_sessions_with_guard(&profile, &["thread-1".to_string()], &options, || Ok(()))
        .unwrap();

    assert!(rollout_path.exists());
    assert!(!archived_path.exists());
    assert_archived(&profile.state_db_path(), "thread-1", false);
}

#[test]
fn restores_codex_archived_rollout_even_when_sqlite_is_not_archived() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    let restored_path = profile
        .sessions_dir()
        .join("2026")
        .join("05")
        .join("07")
        .join("rollout-2026-05-07T21-52-45-thread-1.jsonl");
    let archived_path = profile
        .archived_sessions_dir()
        .join("rollout-2026-05-07T21-52-45-thread-1.jsonl");
    fs::create_dir_all(profile.archived_sessions_dir()).unwrap();
    write_rollout(&archived_path, "thread-1");
    set_rollout_path(&profile.state_db_path(), "thread-1", &restored_path);
    assert_archived(&profile.state_db_path(), "thread-1", false);
    let options = SessionApplyOptions {
        apply: true,
        backup: false,
        include_sessions_backup: false,
    };

    let report =
        restore_sessions_with_guard(&profile, &["thread-1".to_string()], &options, || Ok(()))
            .unwrap();

    assert_eq!(report.sqlite_rows, 0);
    assert!(restored_path.exists());
    assert!(!archived_path.exists());
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

#[test]
fn refreshes_selected_session_rollout_files_without_rewriting_indexes() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    let rollout_1 = profile.sessions_dir().join("thread-1.jsonl");
    let rollout_2 = profile.sessions_dir().join("thread-2.jsonl");
    fs::create_dir_all(profile.sessions_dir()).unwrap();
    write_rollout(&rollout_1, "thread-1");
    write_rollout(&rollout_2, "thread-2");
    set_rollout_path(&profile.state_db_path(), "thread-1", &rollout_1);
    set_rollout_path(&profile.state_db_path(), "thread-2", &rollout_2);
    let old_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1_770_790_000);
    let old_times = FileTimes::new()
        .set_accessed(old_time)
        .set_modified(old_time);
    OpenOptions::new()
        .write(true)
        .open(&rollout_1)
        .unwrap()
        .set_times(old_times)
        .unwrap();
    OpenOptions::new()
        .write(true)
        .open(&rollout_2)
        .unwrap()
        .set_times(old_times)
        .unwrap();
    fs::write(
        profile.session_index_path(),
        concat!(
            "{\"id\":\"thread-1\",\"thread_name\":\"Old title\",\"updated_at\":\"2026-01-01T00:00:00Z\"}\n",
            "{\"id\":\"other\",\"thread_name\":\"Other\",\"updated_at\":\"2026-01-02T00:00:00Z\"}\n",
        ),
    )
    .unwrap();
    let ids = vec!["thread-1".to_string(), "thread-2".to_string()];
    let options = SessionApplyOptions {
        apply: true,
        backup: false,
        include_sessions_backup: false,
    };

    let report =
        refresh_session_updated_at_with_guard(&profile, &ids, &options, || Ok(())).unwrap();

    assert!(report.applied);
    assert_eq!(report.sqlite_rows, 0);
    assert_eq!(report.index_entries, 0);
    assert!(rollout_1.metadata().unwrap().modified().unwrap() > old_time);
    assert!(rollout_2.metadata().unwrap().modified().unwrap() > old_time);
    assert_updated_at(
        &profile.state_db_path(),
        "thread-1",
        1770790115,
        1770794029,
        1770790115043,
        1770794029123,
    );
    assert_updated_at(
        &profile.state_db_path(),
        "thread-2",
        1770790115,
        1770794029,
        1770790115043,
        1770794029122,
    );

    let index = fs::read_to_string(profile.session_index_path()).unwrap();
    let lines = index.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0],
        r#"{"id":"thread-1","thread_name":"Old title","updated_at":"2026-01-01T00:00:00Z"}"#
    );
    assert_eq!(
        lines[1],
        r#"{"id":"other","thread_name":"Other","updated_at":"2026-01-02T00:00:00Z"}"#
    );
}

#[test]
fn refreshes_session_rollout_files_while_codex_is_running() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    create_state_db(&profile.state_db_path());
    let rollout_path = profile.sessions_dir().join("thread-1.jsonl");
    fs::create_dir_all(profile.sessions_dir()).unwrap();
    write_rollout(&rollout_path, "thread-1");
    set_rollout_path(&profile.state_db_path(), "thread-1", &rollout_path);
    let old_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1_770_790_000);
    let old_times = FileTimes::new()
        .set_accessed(old_time)
        .set_modified(old_time);
    OpenOptions::new()
        .write(true)
        .open(&rollout_path)
        .unwrap()
        .set_times(old_times)
        .unwrap();
    let options = SessionApplyOptions {
        apply: true,
        backup: false,
        include_sessions_backup: false,
    };

    let report = refresh_session_updated_at_with_guard(
        &profile,
        &["thread-1".to_string()],
        &options,
        || anyhow::bail!("Codex appears to be running"),
    )
    .unwrap();

    assert!(report.applied);
    assert!(rollout_path.metadata().unwrap().modified().unwrap() > old_time);
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

fn write_rollout(path: &std::path::Path, id: &str) {
    fs::write(
        path,
        format!(
            r#"{{"type":"session_meta","payload":{{"id":"{id}","cwd":"/mnt/e/code/project-a","source":"cli","model_provider":"cm"}}}}"#
        ),
    )
    .unwrap();
}

fn set_rollout_path(path: &std::path::Path, id: &str, rollout_path: &std::path::Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute(
        "UPDATE threads SET rollout_path = ?1 WHERE id = ?2",
        params![rollout_path.display().to_string(), id],
    )
    .unwrap();
}

fn set_file_times(path: &std::path::Path, time: SystemTime) {
    let times = FileTimes::new().set_accessed(time).set_modified(time);
    OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(times)
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

fn assert_updated_at(
    path: &std::path::Path,
    id: &str,
    expected_created_at: i64,
    expected_updated_at: i64,
    expected_created_at_ms: i64,
    expected_updated_at_ms: i64,
) {
    let conn = Connection::open(path).unwrap();
    let (created_at, updated_at, created_at_ms, updated_at_ms): (i64, i64, i64, i64) = conn
        .query_row(
            "SELECT created_at, updated_at, created_at_ms, updated_at_ms FROM threads WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(created_at, expected_created_at);
    assert_eq!(updated_at, expected_updated_at);
    assert_eq!(created_at_ms, expected_created_at_ms);
    assert_eq!(updated_at_ms, expected_updated_at_ms);
}
