use anyhow::Result;
use serde::{Deserialize, Serialize};
use time::{macros::format_description, OffsetDateTime};

use crate::backup;
use crate::profile::CodexProfile;
use crate::safety;
use crate::session_index::update_session_index_updated_at;
use crate::state_db::StateDb;
use crate::trash::{self, TrashManifest};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionApplyOptions {
    pub apply: bool,
    pub backup: bool,
    pub include_sessions_backup: bool,
}

impl Default for SessionApplyOptions {
    fn default() -> Self {
        Self {
            apply: false,
            backup: true,
            include_sessions_backup: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMutationReport {
    pub action: String,
    pub applied: bool,
    pub requested_ids: Vec<String>,
    pub sqlite_rows: usize,
    pub index_entries: usize,
    pub backup_dir: Option<String>,
    pub trash_manifest_path: Option<String>,
    pub trash_manifest: Option<TrashManifest>,
}

impl SessionMutationReport {
    pub fn to_text(&self) -> String {
        let mut lines = vec![
            format!("action: {}", self.action),
            format!("mode: {}", if self.applied { "applied" } else { "dry-run" }),
            format!("requested ids: {}", self.requested_ids.len()),
            format!("sqlite rows: {}", self.sqlite_rows),
            format!("session_index entries: {}", self.index_entries),
        ];
        if let Some(backup_dir) = &self.backup_dir {
            lines.push(format!("backup: {backup_dir}"));
        }
        if let Some(trash_manifest_path) = &self.trash_manifest_path {
            lines.push(format!("trash manifest: {trash_manifest_path}"));
        }
        lines.join("\n")
    }
}

pub fn archive_sessions(
    profile: &CodexProfile,
    ids: &[String],
    options: &SessionApplyOptions,
) -> Result<SessionMutationReport> {
    archive_sessions_with_guard(profile, ids, options, safety::ensure_codex_not_running)
}

pub fn archive_sessions_with_guard<F>(
    profile: &CodexProfile,
    ids: &[String],
    options: &SessionApplyOptions,
    guard: F,
) -> Result<SessionMutationReport>
where
    F: FnOnce() -> Result<()>,
{
    set_archived_with_guard(profile, ids, true, "archive sessions", options, guard)
}

pub fn restore_sessions(
    profile: &CodexProfile,
    ids: &[String],
    options: &SessionApplyOptions,
) -> Result<SessionMutationReport> {
    restore_sessions_with_guard(profile, ids, options, safety::ensure_codex_not_running)
}

pub fn restore_sessions_with_guard<F>(
    profile: &CodexProfile,
    ids: &[String],
    options: &SessionApplyOptions,
    guard: F,
) -> Result<SessionMutationReport>
where
    F: FnOnce() -> Result<()>,
{
    set_archived_with_guard(profile, ids, false, "restore sessions", options, guard)
}

pub fn delete_sessions(
    profile: &CodexProfile,
    ids: &[String],
    options: &SessionApplyOptions,
) -> Result<SessionMutationReport> {
    delete_sessions_with_guard(profile, ids, options, safety::ensure_codex_not_running)
}

pub fn refresh_session_updated_at(
    profile: &CodexProfile,
    ids: &[String],
    options: &SessionApplyOptions,
) -> Result<SessionMutationReport> {
    let now = OffsetDateTime::now_utc();
    let updated_at = now.format(&format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second]Z"
    ))?;
    let updated_at_ms = now.unix_timestamp() * 1000 + i64::from(now.millisecond());
    refresh_session_updated_at_with_guard(
        profile,
        ids,
        &updated_at,
        updated_at_ms,
        options,
        safety::ensure_codex_not_running,
    )
}

pub fn refresh_session_updated_at_with_guard<F>(
    profile: &CodexProfile,
    ids: &[String],
    updated_at: &str,
    updated_at_ms: i64,
    options: &SessionApplyOptions,
    guard: F,
) -> Result<SessionMutationReport>
where
    F: FnOnce() -> Result<()>,
{
    let db = StateDb::open(&profile.state_db_path())?;
    let threads = db.read_threads()?;
    let sqlite_rows = threads
        .iter()
        .filter(|thread| ids.iter().any(|id| id == &thread.id))
        .filter(|thread| {
            thread.updated_at.as_deref() != Some(updated_at)
                || thread.updated_at_ms != Some(updated_at_ms)
        })
        .count();
    let index_entries = refreshable_session_index_entries(&threads, ids);

    let mut report = SessionMutationReport {
        action: "refresh session updated_at".to_string(),
        applied: options.apply,
        requested_ids: ids.to_vec(),
        sqlite_rows,
        index_entries,
        backup_dir: None,
        trash_manifest_path: None,
        trash_manifest: None,
    };

    if !options.apply {
        return Ok(report);
    }

    guard()?;
    if options.backup {
        let backup = backup::create_backup(profile, options.include_sessions_backup)?;
        report.backup_dir = Some(backup.backup_dir.display().to_string());
    }

    let mut db = StateDb::open(&profile.state_db_path())?;
    report.sqlite_rows = db.update_selected_session_updated_at(ids, updated_at, updated_at_ms)?;
    let threads = db.read_threads()?;
    report.index_entries =
        update_session_index_updated_at(&profile.session_index_path(), &threads, ids, updated_at)?;
    Ok(report)
}

pub fn delete_sessions_with_guard<F>(
    profile: &CodexProfile,
    ids: &[String],
    options: &SessionApplyOptions,
    guard: F,
) -> Result<SessionMutationReport>
where
    F: FnOnce() -> Result<()>,
{
    let db = StateDb::open(&profile.state_db_path())?;
    let threads = db.read_threads()?;
    let selected = threads
        .into_iter()
        .filter(|thread| ids.iter().any(|id| id == &thread.id))
        .collect::<Vec<_>>();

    let mut report = SessionMutationReport {
        action: "delete sessions to trash".to_string(),
        applied: options.apply,
        requested_ids: ids.to_vec(),
        sqlite_rows: selected.iter().filter(|thread| !thread.archived).count(),
        index_entries: 0,
        backup_dir: None,
        trash_manifest_path: None,
        trash_manifest: None,
    };

    if !options.apply {
        return Ok(report);
    }

    guard()?;
    if options.backup {
        let backup = backup::create_backup(profile, options.include_sessions_backup)?;
        report.backup_dir = Some(backup.backup_dir.display().to_string());
    }

    let trash = trash::move_threads_to_trash(profile, &selected)?;
    report.trash_manifest_path = Some(trash.manifest_path.display().to_string());
    report.trash_manifest = Some(trash.manifest);

    let mut db = StateDb::open(&profile.state_db_path())?;
    report.sqlite_rows = db.set_archived(ids, true)?;
    Ok(report)
}

fn set_archived_with_guard<F>(
    profile: &CodexProfile,
    ids: &[String],
    archived: bool,
    action: &str,
    options: &SessionApplyOptions,
    guard: F,
) -> Result<SessionMutationReport>
where
    F: FnOnce() -> Result<()>,
{
    let db = StateDb::open(&profile.state_db_path())?;
    let threads = db.read_threads()?;
    let sqlite_rows = threads
        .iter()
        .filter(|thread| ids.iter().any(|id| id == &thread.id))
        .filter(|thread| thread.archived != archived)
        .count();

    let mut report = SessionMutationReport {
        action: action.to_string(),
        applied: options.apply,
        requested_ids: ids.to_vec(),
        sqlite_rows,
        index_entries: 0,
        backup_dir: None,
        trash_manifest_path: None,
        trash_manifest: None,
    };

    if !options.apply {
        return Ok(report);
    }

    guard()?;
    if options.backup {
        let backup = backup::create_backup(profile, options.include_sessions_backup)?;
        report.backup_dir = Some(backup.backup_dir.display().to_string());
    }

    let mut db = StateDb::open(&profile.state_db_path())?;
    report.sqlite_rows = db.set_archived(ids, archived)?;
    Ok(report)
}

fn refreshable_session_index_entries(
    threads: &[crate::state_db::ThreadRecord],
    ids: &[String],
) -> usize {
    threads
        .iter()
        .filter(|thread| ids.iter().any(|id| id == &thread.id))
        .filter(|thread| crate::session_index::is_visible_user_thread(thread))
        .count()
}
