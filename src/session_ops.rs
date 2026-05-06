use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::backup;
use crate::profile::CodexProfile;
use crate::safety;
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
