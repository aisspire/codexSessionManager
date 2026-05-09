use std::collections::{HashMap, HashSet};
use std::fs::{self, FileTimes, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::backup;
use crate::profile::CodexProfile;
use crate::rollout::read_all_rollout_meta;
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
    refresh_session_updated_at_with_guard(profile, ids, options, safety::ensure_codex_not_running)
}

pub fn refresh_session_updated_at_with_guard<F>(
    profile: &CodexProfile,
    ids: &[String],
    options: &SessionApplyOptions,
    _guard: F,
) -> Result<SessionMutationReport>
where
    F: FnOnce() -> Result<()>,
{
    let rollout_paths = refreshable_rollout_paths(profile, ids)?;

    let mut report = SessionMutationReport {
        action: "touch session rollout files".to_string(),
        applied: options.apply,
        requested_ids: ids.to_vec(),
        sqlite_rows: 0,
        index_entries: 0,
        backup_dir: None,
        trash_manifest_path: None,
        trash_manifest: None,
    };

    if !options.apply {
        return Ok(report);
    }

    if options.backup {
        let backup = backup::create_backup(profile, options.include_sessions_backup)?;
        report.backup_dir = Some(backup.backup_dir.display().to_string());
    }

    touch_rollout_files(&rollout_paths)?;
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

    let rollout_paths = move_rollout_files_for_archive_state(profile, ids, archived)?;
    let mut db = StateDb::open(&profile.state_db_path())?;
    report.sqlite_rows = db.set_archived(ids, archived)?;
    touch_rollout_files(&rollout_paths)?;
    Ok(report)
}

fn refreshable_rollout_paths(profile: &CodexProfile, ids: &[String]) -> Result<Vec<PathBuf>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let selected_ids = ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let db = StateDb::open(&profile.state_db_path())?;
    let mut by_id = db
        .read_threads()?
        .into_iter()
        .filter(|thread| selected_ids.contains(thread.id.as_str()))
        .filter_map(|thread| {
            thread
                .rollout_path
                .map(PathBuf::from)
                .map(|path| (thread.id, path))
        })
        .collect::<HashMap<_, _>>();

    for meta in read_all_rollout_meta(&profile.sessions_dir())? {
        let Some(id) = meta.id else {
            continue;
        };
        if selected_ids.contains(id.as_str()) {
            by_id.insert(id, meta.path);
        }
    }

    Ok(ids
        .iter()
        .filter_map(|id| by_id.get(id).cloned())
        .collect::<Vec<_>>())
}

fn move_rollout_files_for_archive_state(
    profile: &CodexProfile,
    ids: &[String],
    archived: bool,
) -> Result<Vec<PathBuf>> {
    if archived {
        return move_rollout_files_to_archive(profile, ids);
    }
    move_rollout_files_to_sessions(profile, ids)
}

fn move_rollout_files_to_archive(profile: &CodexProfile, ids: &[String]) -> Result<Vec<PathBuf>> {
    let mut moved_paths = Vec::new();
    for source in refreshable_rollout_paths(profile, ids)? {
        if !source.exists() {
            continue;
        }
        let Some(file_name) = source.file_name() else {
            continue;
        };
        fs::create_dir_all(profile.archived_sessions_dir())?;
        let destination = profile.archived_sessions_dir().join(file_name);
        fs::rename(&source, &destination)?;
        moved_paths.push(destination);
    }
    Ok(moved_paths)
}

fn move_rollout_files_to_sessions(profile: &CodexProfile, ids: &[String]) -> Result<Vec<PathBuf>> {
    let selected_ids = ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut moved_paths = Vec::new();
    let destinations = session_rollout_destinations(profile, ids)?;

    for meta in read_all_rollout_meta(&profile.archived_sessions_dir())? {
        let Some(id) = meta.id.as_deref() else {
            continue;
        };
        if !selected_ids.contains(id) || !meta.path.exists() {
            continue;
        }
        let destination = destinations
            .get(id)
            .cloned()
            .unwrap_or_else(|| restored_rollout_path(profile, &meta.path));
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&meta.path, &destination)?;
        moved_paths.push(destination);
    }

    Ok(moved_paths)
}

fn session_rollout_destinations(
    profile: &CodexProfile,
    ids: &[String],
) -> Result<HashMap<String, PathBuf>> {
    let selected_ids = ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let db = StateDb::open(&profile.state_db_path())?;
    Ok(db
        .read_threads()?
        .into_iter()
        .filter(|thread| selected_ids.contains(thread.id.as_str()))
        .filter_map(|thread| {
            thread
                .rollout_path
                .filter(|path| !path.is_empty())
                .map(PathBuf::from)
                .map(|path| (thread.id, path))
        })
        .collect())
}

fn restored_rollout_path(profile: &CodexProfile, archived_path: &Path) -> PathBuf {
    let Some(file_name) = archived_path.file_name() else {
        return profile.sessions_dir().join("restored-session.jsonl");
    };
    let file_name_text = file_name.to_string_lossy();
    let parts = file_name_text
        .strip_prefix("rollout-")
        .and_then(|rest| rest.get(0..10))
        .map(|date| date.split('-').collect::<Vec<_>>());

    if let Some(parts) = parts {
        if parts.len() == 3 {
            return profile
                .sessions_dir()
                .join(parts[0])
                .join(parts[1])
                .join(parts[2])
                .join(file_name);
        }
    }

    profile.sessions_dir().join(file_name)
}

fn touch_rollout_files(paths: &[PathBuf]) -> Result<()> {
    let now = SystemTime::now();
    let times = FileTimes::new().set_accessed(now).set_modified(now);
    for path in paths {
        touch_rollout_file(path, times)?;
    }
    Ok(())
}

fn touch_rollout_file(path: &Path, times: FileTimes) -> Result<()> {
    OpenOptions::new()
        .write(true)
        .open(path)
        .with_context(|| format!("failed to open rollout file {}", path.display()))?
        .set_times(times)
        .with_context(|| format!("failed to touch rollout file {}", path.display()))
}
