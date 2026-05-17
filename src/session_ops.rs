use std::collections::{HashMap, HashSet};
use std::fs::{self, FileTimes, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::backup_store::{self, BackupTrigger};
use crate::path_map::path_buf_for_current_os;
use crate::profile::CodexProfile;
use crate::rollout::read_all_rollout_meta;
use crate::safety;
use crate::session_index;
use crate::state_db::StateDb;
use crate::trash::{self, TrashManifest};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionApplyOptions {
    pub apply: bool,
}

impl Default for SessionApplyOptions {
    fn default() -> Self {
        Self { apply: false }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionMutationReport {
    pub action: String,
    pub applied: bool,
    pub requested_ids: Vec<String>,
    pub sqlite_rows: usize,
    pub index_entries: usize,
    pub backup_manifests: Vec<String>,
    pub warnings: Vec<String>,
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
        if !self.backup_manifests.is_empty() {
            lines.push(format!("backup manifests: {}", self.backup_manifests.len()));
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

pub fn active_sessions(
    profile: &CodexProfile,
    ids: &[String],
    options: &SessionApplyOptions,
) -> Result<SessionMutationReport> {
    active_sessions_with_guard(profile, ids, options, safety::ensure_codex_not_running)
}

pub fn active_sessions_with_guard<F>(
    profile: &CodexProfile,
    ids: &[String],
    options: &SessionApplyOptions,
    guard: F,
) -> Result<SessionMutationReport>
where
    F: FnOnce() -> Result<()>,
{
    set_archived_with_guard(profile, ids, false, "active sessions", options, guard)
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
    let selected_ids = ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let threads = read_threads_if_present(profile)?;
    let selected_threads = threads
        .iter()
        .filter(|thread| selected_ids.contains(thread.id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let rollout_paths = refreshable_rollout_paths(profile, ids)?;

    let mut report = SessionMutationReport {
        action: "refresh session updated time".to_string(),
        applied: options.apply,
        requested_ids: ids.to_vec(),
        sqlite_rows: selected_threads.len(),
        index_entries: 0,
        trash_manifest_path: None,
        backup_manifests: Vec::new(),
        warnings: Vec::new(),
        trash_manifest: None,
    };

    if !options.apply {
        return Ok(report);
    }

    let timestamp = current_session_timestamp()?;
    touch_rollout_files_at(&rollout_paths, timestamp.system_time)?;
    if profile.state_db_path().exists() {
        let mut db = StateDb::open(&profile.state_db_path())?;
        report.sqlite_rows =
            db.update_thread_timestamps(ids, timestamp.updated_at, timestamp.updated_at_ms)?;
    } else {
        report.sqlite_rows = 0;
    }
    report.index_entries = session_index::update_session_index_updated_at(
        &profile.session_index_path(),
        &selected_threads,
        ids,
        &timestamp.updated_at_iso,
    )?;
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
    let threads = read_threads_if_present(profile)?;
    let selected = selected_threads_for_delete(profile, ids, &threads)?;

    let mut report = SessionMutationReport {
        action: "delete sessions to trash".to_string(),
        applied: options.apply,
        requested_ids: ids.to_vec(),
        sqlite_rows: selected
            .iter()
            .filter(|thread| ids.iter().any(|id| id == &thread.id))
            .count(),
        index_entries: count_session_index_entries(&profile.session_index_path(), ids)?,
        backup_manifests: Vec::new(),
        warnings: Vec::new(),
        trash_manifest_path: None,
        trash_manifest: None,
    };

    if !options.apply {
        return Ok(report);
    }

    guard()?;

    for id in ids {
        let manifest = backup_store::create_session_backup(profile, id, BackupTrigger::Delete)?;
        report
            .backup_manifests
            .push(manifest_path_from_backup(&manifest)?);
    }

    let trash = trash::move_threads_to_trash(profile, &selected)?;
    report.trash_manifest_path = Some(trash.manifest_path.display().to_string());
    report.trash_manifest = Some(trash.manifest);

    report.index_entries = remove_session_index_entries(&profile.session_index_path(), ids)?;
    if profile.state_db_path().exists() {
        let mut db = StateDb::open(&profile.state_db_path())?;
        report.sqlite_rows = db.delete_threads(ids)?;
    } else {
        report.sqlite_rows = 0;
    }
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
        backup_manifests: Vec::new(),
        warnings: Vec::new(),
        trash_manifest_path: None,
        trash_manifest: None,
    };

    if !options.apply {
        return Ok(report);
    }

    guard()?;

    let rollout_paths = move_rollout_files_for_archive_state(profile, ids, archived)?;
    let mut db = StateDb::open(&profile.state_db_path())?;
    report.sqlite_rows = db.set_archived(ids, archived)?;
    touch_rollout_files(&rollout_paths)?;
    Ok(report)
}

fn read_threads_if_present(profile: &CodexProfile) -> Result<Vec<crate::state_db::ThreadRecord>> {
    if !profile.state_db_path().exists() {
        return Ok(Vec::new());
    }
    let db = StateDb::open(&profile.state_db_path())?;
    db.read_threads()
}

fn selected_threads_for_delete(
    profile: &CodexProfile,
    ids: &[String],
    threads: &[crate::state_db::ThreadRecord],
) -> Result<Vec<crate::state_db::ThreadRecord>> {
    let selected_ids = ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut by_id = threads
        .iter()
        .filter(|thread| selected_ids.contains(thread.id.as_str()))
        .map(|thread| (thread.id.clone(), thread.clone()))
        .collect::<HashMap<_, _>>();

    for id in ids {
        if let Some(path) = backup_store::locate_unique_local_session(profile, id)? {
            by_id
                .entry(id.clone())
                .and_modify(|thread| thread.rollout_path = Some(path.display().to_string()))
                .or_insert_with(|| synthetic_thread_for_local_jsonl(id, path));
        }
    }

    Ok(ids
        .iter()
        .filter_map(|id| by_id.get(id).cloned())
        .collect::<Vec<_>>())
}

fn synthetic_thread_for_local_jsonl(id: &str, path: PathBuf) -> crate::state_db::ThreadRecord {
    crate::state_db::ThreadRecord {
        id: id.to_string(),
        rollout_path: Some(path.display().to_string()),
        cwd: None,
        source: None,
        model_provider: None,
        model: None,
        reasoning_effort: None,
        has_user_event: true,
        archived: false,
        created_at: None,
        updated_at: None,
        created_at_ms: None,
        updated_at_ms: None,
        title: None,
        first_user_message: None,
    }
}

fn manifest_path_from_backup(manifest: &backup_store::SessionBackupManifest) -> Result<String> {
    let session_path = manifest
        .backup_session_path
        .as_deref()
        .context("session backup did not include a copied JSONL path")?;
    let manifest_path = Path::new(session_path)
        .parent()
        .context("backup session path has no parent")?
        .join("manifest.json");
    Ok(manifest_path.display().to_string())
}

fn count_session_index_entries(path: &Path, ids: &[String]) -> Result<usize> {
    if !path.exists() || ids.is_empty() {
        return Ok(0);
    }
    let selected_ids = ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read session index {}", path.display()))?;
    Ok(text
        .lines()
        .filter(|line| index_line_matches_id(line, &selected_ids).unwrap_or(false))
        .count())
}

fn remove_session_index_entries(path: &Path, ids: &[String]) -> Result<usize> {
    if !path.exists() || ids.is_empty() {
        return Ok(0);
    }
    let selected_ids = ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read session index {}", path.display()))?;
    let mut removed = 0;
    let mut kept = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if index_line_matches_id(line, &selected_ids)? {
            removed += 1;
        } else {
            kept.push(line.to_string());
        }
    }
    let new_text = if kept.is_empty() {
        String::new()
    } else {
        format!("{}\n", kept.join("\n"))
    };
    fs::write(path, new_text)
        .with_context(|| format!("failed to write session index {}", path.display()))?;
    Ok(removed)
}

fn index_line_matches_id(line: &str, ids: &HashSet<&str>) -> Result<bool> {
    let value = serde_json::from_str::<serde_json::Value>(line)?;
    Ok(value
        .get("id")
        .and_then(|value| value.as_str())
        .is_some_and(|id| ids.contains(id)))
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
                .map(|path| path_buf_for_current_os(&path))
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
        move_file(&source, &destination)?;
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
        move_file(&meta.path, &destination)?;
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
                .map(|path| path_buf_for_current_os(&path))
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

fn move_file(source: &Path, destination: &Path) -> Result<()> {
    move_file_with_rename(source, destination, |from, to| fs::rename(from, to))
}

fn move_file_with_rename<F>(source: &Path, destination: &Path, rename: F) -> Result<()>
where
    F: FnOnce(&Path, &Path) -> io::Result<()>,
{
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create move dir {}", parent.display()))?;
    }

    match rename(source, destination) {
        Ok(()) => Ok(()),
        Err(error) if is_cross_device_error(&error) => {
            fs::copy(source, destination).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source.display(),
                    destination.display()
                )
            })?;
            fs::remove_file(source)
                .with_context(|| format!("failed to remove {}", source.display()))?;
            Ok(())
        }
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to move {} to {}",
                source.display(),
                destination.display()
            )
        }),
    }
}

fn is_cross_device_error(error: &io::Error) -> bool {
    matches!(error.raw_os_error(), Some(17) | Some(18))
}

fn touch_rollout_files(paths: &[PathBuf]) -> Result<()> {
    touch_rollout_files_at(paths, SystemTime::now())
}

fn touch_rollout_files_at(paths: &[PathBuf], now: SystemTime) -> Result<()> {
    let times = FileTimes::new().set_accessed(now).set_modified(now);
    for path in paths {
        touch_rollout_file(path, times)?;
    }
    Ok(())
}

struct SessionTimestamp {
    system_time: SystemTime,
    updated_at: i64,
    updated_at_ms: i64,
    updated_at_iso: String,
}

fn current_session_timestamp() -> Result<SessionTimestamp> {
    let system_time = SystemTime::now();
    let duration = system_time
        .duration_since(SystemTime::UNIX_EPOCH)
        .context("system time is before unix epoch")?;
    let updated_at =
        i64::try_from(duration.as_secs()).context("session timestamp seconds overflowed i64")?;
    let updated_at_ms =
        i64::try_from(duration.as_millis()).context("session timestamp millis overflowed i64")?;
    let updated_at_iso = time::OffsetDateTime::from_unix_timestamp(updated_at)
        .context("failed to convert session timestamp")?
        .format(&time::format_description::well_known::Rfc3339)
        .context("failed to format session timestamp")?;
    Ok(SessionTimestamp {
        system_time,
        updated_at,
        updated_at_ms,
        updated_at_iso,
    })
}

fn touch_rollout_file(path: &Path, times: FileTimes) -> Result<()> {
    OpenOptions::new()
        .write(true)
        .open(path)
        .with_context(|| format!("failed to open rollout file {}", path.display()))?
        .set_times(times)
        .with_context(|| format!("failed to touch rollout file {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::io;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn move_file_falls_back_to_copy_and_delete_when_rename_crosses_devices() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source.jsonl");
        let destination = dir.path().join("nested").join("destination.jsonl");
        fs::write(&source, "session body").unwrap();

        move_file_with_rename(&source, &destination, |_, _| {
            Err(io::Error::from_raw_os_error(17))
        })
        .unwrap();

        assert!(!source.exists());
        assert_eq!(fs::read_to_string(destination).unwrap(), "session body");
    }
}
