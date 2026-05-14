use std::collections::BTreeMap;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use walkdir::WalkDir;

use crate::favorites;
use crate::path_map::path_buf_for_current_os;
use crate::profile::CodexProfile;
use crate::rollout::read_all_rollout_meta;
use crate::session_index::read_session_index;
use crate::settings::load_settings;
use crate::state_db::{StateDb, ThreadRecord};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBackupManifest {
    pub version: u32,
    pub app_version: Option<String>,
    pub codex_home: String,
    pub session_id: String,
    pub created_at_unix: i64,
    pub trigger: BackupTrigger,
    pub title: Option<String>,
    pub project: Option<String>,
    pub original_session_path: Option<String>,
    pub backup_session_path: Option<String>,
    pub index_entries: Vec<serde_json::Value>,
    pub sqlite_thread: Option<ThreadRecord>,
    pub local_session_existed_at_backup_time: bool,
    pub favorite: bool,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackupTrigger {
    Delete,
    Edit,
    Manual,
    DatabaseRepair,
    RestorePreflight,
}

impl BackupTrigger {
    fn as_str(self) -> &'static str {
        match self {
            Self::Delete => "delete",
            Self::Edit => "edit",
            Self::Manual => "manual",
            Self::DatabaseRepair => "database-repair",
            Self::RestorePreflight => "restore-preflight",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBackupSummary {
    pub session_id: String,
    pub title: Option<String>,
    pub project: Option<String>,
    pub local_exists: bool,
    pub snapshots: Vec<SessionBackupSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBackupSnapshot {
    pub backup_id: String,
    pub created_at_unix: i64,
    pub trigger: BackupTrigger,
    pub manifest_path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionReport {
    pub deleted_backup_dirs: Vec<String>,
    pub skipped_unique_archives: Vec<String>,
    pub total_bytes_before: u64,
    pub total_bytes_after: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupDeleteReport {
    pub backup_id: String,
    pub deleted: bool,
}

pub fn create_session_backup(
    profile: &CodexProfile,
    session_id: &str,
    trigger: BackupTrigger,
) -> Result<SessionBackupManifest> {
    let local_path = locate_unique_local_session(profile, session_id)?;
    if local_path.is_none() && matches!(trigger, BackupTrigger::Delete | BackupTrigger::Edit) {
        bail!("cannot create {trigger:?} backup for {session_id}: local JSONL file was not found");
    }

    let source_size = local_path
        .as_deref()
        .map(file_size)
        .transpose()?
        .unwrap_or(0);
    guard_backup_size(profile, source_size)?;

    let created_at_unix = OffsetDateTime::now_utc().unix_timestamp();
    let snapshot_dir = backup_root(profile)
        .join("sessions")
        .join(session_id)
        .join(format!("{}-{}", created_at_unix, trigger.as_str()));
    fs::create_dir_all(&snapshot_dir)
        .with_context(|| format!("failed to create backup dir {}", snapshot_dir.display()))?;

    let backup_session_path = if let Some(local_path) = local_path.as_deref() {
        let file_name = local_path
            .file_name()
            .context("session backup source has no file name")?;
        let target = snapshot_dir.join(file_name);
        fs::copy(local_path, &target).with_context(|| {
            format!(
                "failed to copy {} to {}",
                local_path.display(),
                target.display()
            )
        })?;
        Some(target.display().to_string())
    } else {
        None
    };

    let index_entries = raw_index_entries_for_session(&profile.session_index_path(), session_id)?;
    let sqlite_thread = sqlite_thread(profile, session_id)?;
    let title = index_entries
        .iter()
        .rev()
        .find_map(|value| value.get("thread_name").and_then(|value| value.as_str()))
        .map(str::to_string)
        .or_else(|| {
            sqlite_thread
                .as_ref()
                .and_then(|thread| thread.title.clone())
        })
        .or_else(|| {
            sqlite_thread
                .as_ref()
                .and_then(|thread| thread.first_user_message.clone())
        });
    let project = sqlite_thread
        .as_ref()
        .and_then(|thread| thread.cwd.clone())
        .or_else(|| rollout_project(local_path.as_deref()));
    let favorite = favorites::favorite_ids(profile)
        .map(|ids| ids.contains(session_id))
        .unwrap_or(false);

    let manifest_path = snapshot_dir.join("manifest.json");
    let mut manifest = SessionBackupManifest {
        version: 1,
        app_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        codex_home: profile.codex_home.display().to_string(),
        session_id: session_id.to_string(),
        created_at_unix,
        trigger,
        title,
        project,
        original_session_path: local_path.as_ref().map(|path| path.display().to_string()),
        backup_session_path,
        index_entries,
        sqlite_thread,
        local_session_existed_at_backup_time: local_path.is_some(),
        favorite,
        size_bytes: 0,
    };
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?).with_context(|| {
        format!(
            "failed to write backup manifest {}",
            manifest_path.display()
        )
    })?;
    manifest.size_bytes = dir_size(&snapshot_dir)?;
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?).with_context(|| {
        format!(
            "failed to write backup manifest {}",
            manifest_path.display()
        )
    })?;

    enforce_backup_retention(profile)?;
    Ok(manifest)
}

pub fn list_session_backups(profile: &CodexProfile) -> Result<Vec<SessionBackupSummary>> {
    let mut grouped = BTreeMap::<String, Vec<(String, SessionBackupManifest, PathBuf)>>::new();
    let sessions_root = backup_root(profile).join("sessions");
    if !sessions_root.exists() {
        return Ok(Vec::new());
    }

    for entry in WalkDir::new(&sessions_root)
        .min_depth(3)
        .max_depth(3)
        .into_iter()
    {
        let entry = entry?;
        if !entry.file_type().is_file() || entry.file_name() != "manifest.json" {
            continue;
        }
        let path = entry.path();
        let manifest = read_manifest(path)?;
        let backup_id = backup_id_for_manifest(profile, path)?;
        grouped
            .entry(manifest.session_id.clone())
            .or_default()
            .push((backup_id, manifest, path.to_path_buf()));
    }

    let mut summaries = Vec::new();
    for (session_id, mut manifests) in grouped {
        manifests.sort_by(|left, right| right.1.created_at_unix.cmp(&left.1.created_at_unix));
        let local_exists = local_session_exists(profile, &session_id)?;
        let title = manifests
            .first()
            .and_then(|(_, manifest, _)| manifest.title.clone());
        let project = manifests
            .first()
            .and_then(|(_, manifest, _)| manifest.project.clone());
        let snapshots = manifests
            .into_iter()
            .map(|(backup_id, manifest, path)| SessionBackupSnapshot {
                backup_id,
                created_at_unix: manifest.created_at_unix,
                trigger: manifest.trigger,
                manifest_path: path.display().to_string(),
                size_bytes: manifest.size_bytes,
            })
            .collect::<Vec<_>>();
        summaries.push(SessionBackupSummary {
            session_id,
            title,
            project,
            local_exists,
            snapshots,
        });
    }
    Ok(summaries)
}

pub fn delete_backup_snapshot(profile: &CodexProfile, backup_id: &str) -> Result<()> {
    delete_backup_snapshot_with_confirmation(profile, backup_id, true).map(|_| ())
}

pub fn read_session_backup_manifest(
    profile: &CodexProfile,
    backup_id: &str,
) -> Result<(SessionBackupManifest, PathBuf)> {
    let dir = backup_dir_from_id(profile, backup_id)?;
    let manifest_path = dir.join("manifest.json");
    Ok((read_manifest(&manifest_path)?, manifest_path))
}

pub fn delete_backup_snapshot_with_confirmation(
    profile: &CodexProfile,
    backup_id: &str,
    confirmed_last_archive: bool,
) -> Result<BackupDeleteReport> {
    let dir = backup_dir_from_id(profile, backup_id)?;
    let manifest = read_manifest(&dir.join("manifest.json"))?;
    if !confirmed_last_archive && is_unique_archive(profile, &manifest)? {
        bail!(
            "refusing to delete the last backup for missing local session {} without confirmation",
            manifest.session_id
        );
    }
    fs::remove_dir_all(&dir)
        .with_context(|| format!("failed to delete backup snapshot {}", dir.display()))?;
    Ok(BackupDeleteReport {
        backup_id: backup_id.to_string(),
        deleted: true,
    })
}

pub fn locate_unique_local_session(
    profile: &CodexProfile,
    session_id: &str,
) -> Result<Option<PathBuf>> {
    let mut matches = Vec::new();
    for meta in read_all_rollout_meta(&profile.sessions_dir())?
        .into_iter()
        .chain(read_all_rollout_meta(&profile.archived_sessions_dir())?)
    {
        if meta.id.as_deref() == Some(session_id) && meta.path.exists() {
            matches.push(meta.path);
        }
    }

    if matches.is_empty() {
        if let Some(path) = sqlite_thread(profile, session_id)?
            .and_then(|thread| thread.rollout_path)
            .map(|path| path_buf_for_current_os(&path))
            .filter(|path| path.exists())
        {
            matches.push(path);
        }
    }

    matches.sort();
    matches.dedup();
    if matches.len() > 1 {
        bail!("multiple local JSONL files found for session {session_id}");
    }
    Ok(matches.pop())
}

pub fn enforce_backup_retention(profile: &CodexProfile) -> Result<RetentionReport> {
    let settings = load_settings(profile)?.backup;
    let mut report = RetentionReport {
        total_bytes_before: dir_size(&backup_root(profile)).unwrap_or(0),
        total_bytes_after: 0,
        ..RetentionReport::default()
    };
    let mut snapshots = all_snapshot_dirs(profile)?;
    if snapshots.is_empty() {
        report.total_bytes_after = report.total_bytes_before;
        return Ok(report);
    }

    let protected = protected_unique_archives(profile, &snapshots)?;
    let mut delete_dirs = Vec::<PathBuf>::new();

    if let Some(max_age_days) = settings.max_age_days {
        let oldest_allowed =
            OffsetDateTime::now_utc().unix_timestamp() - (max_age_days as i64 * 86_400);
        for (manifest, dir) in &snapshots {
            if manifest.created_at_unix < oldest_allowed {
                queue_delete_or_skip(
                    &settings,
                    &protected,
                    manifest,
                    dir,
                    &mut delete_dirs,
                    &mut report,
                );
            }
        }
    }

    if let Some(max_count) = settings.max_count {
        snapshots.sort_by(|left, right| right.0.created_at_unix.cmp(&left.0.created_at_unix));
        for (manifest, dir) in snapshots.iter().skip(max_count) {
            queue_delete_or_skip(
                &settings,
                &protected,
                manifest,
                dir,
                &mut delete_dirs,
                &mut report,
            );
        }
    }

    if let Some(max_bytes) = settings.max_bytes {
        let mut projected = report.total_bytes_before;
        let mut oldest = snapshots.clone();
        oldest.sort_by(|left, right| left.0.created_at_unix.cmp(&right.0.created_at_unix));
        for (manifest, dir) in &oldest {
            if projected <= max_bytes {
                break;
            }
            if delete_dirs.contains(dir) {
                continue;
            }
            let size = dir_size(dir).unwrap_or(0);
            if queue_delete_or_skip(
                &settings,
                &protected,
                manifest,
                dir,
                &mut delete_dirs,
                &mut report,
            ) {
                projected = projected.saturating_sub(size);
            }
        }
        if projected > max_bytes {
            report.warnings.push(format!(
                "backup storage remains above max_bytes because protected archives were skipped: {projected}/{max_bytes}"
            ));
        }
    }

    delete_dirs.sort();
    delete_dirs.dedup();
    for dir in delete_dirs {
        if dir.exists() {
            fs::remove_dir_all(&dir)
                .with_context(|| format!("failed to prune backup {}", dir.display()))?;
            report.deleted_backup_dirs.push(dir.display().to_string());
        }
    }
    report.skipped_unique_archives.sort();
    report.skipped_unique_archives.dedup();
    report.total_bytes_after = dir_size(&backup_root(profile)).unwrap_or(0);
    Ok(report)
}

fn guard_backup_size(profile: &CodexProfile, source_size: u64) -> Result<()> {
    let settings = load_settings(profile)?.backup;
    if let Some(max_bytes) = settings.max_bytes {
        let current = dir_size(&backup_root(profile)).unwrap_or(0);
        if current.saturating_add(source_size) > max_bytes {
            bail!(
                "backup would exceed configured max_bytes: current {current}, required {source_size}, limit {max_bytes}"
            );
        }
    }
    Ok(())
}

fn all_snapshot_dirs(profile: &CodexProfile) -> Result<Vec<(SessionBackupManifest, PathBuf)>> {
    let sessions_root = backup_root(profile).join("sessions");
    if !sessions_root.exists() {
        return Ok(Vec::new());
    }
    let mut snapshots = Vec::new();
    for entry in WalkDir::new(&sessions_root)
        .min_depth(3)
        .max_depth(3)
        .into_iter()
    {
        let entry = entry?;
        if !entry.file_type().is_file() || entry.file_name() != "manifest.json" {
            continue;
        }
        let manifest = read_manifest(entry.path())?;
        let dir = entry
            .path()
            .parent()
            .context("manifest path has no parent")?
            .to_path_buf();
        snapshots.push((manifest, dir));
    }
    Ok(snapshots)
}

fn protected_unique_archives(
    profile: &CodexProfile,
    snapshots: &[(SessionBackupManifest, PathBuf)],
) -> Result<HashSet<String>> {
    let mut counts = HashMap::<String, usize>::new();
    for (manifest, _) in snapshots {
        *counts.entry(manifest.session_id.clone()).or_default() += 1;
    }

    let mut protected = HashSet::new();
    for (session_id, count) in counts {
        if count == 1 && !local_session_exists(profile, &session_id)? {
            protected.insert(session_id);
        }
    }
    Ok(protected)
}

fn queue_delete_or_skip(
    settings: &crate::settings::BackupSettings,
    protected: &HashSet<String>,
    manifest: &SessionBackupManifest,
    dir: &Path,
    delete_dirs: &mut Vec<PathBuf>,
    report: &mut RetentionReport,
) -> bool {
    if settings.skip_unique_archive_on_auto_prune && protected.contains(&manifest.session_id) {
        report
            .skipped_unique_archives
            .push(manifest.session_id.clone());
        report.warnings.push(format!(
            "skipped unique archive for missing local session {}",
            manifest.session_id
        ));
        return false;
    }
    delete_dirs.push(dir.to_path_buf());
    true
}

fn is_unique_archive(profile: &CodexProfile, manifest: &SessionBackupManifest) -> Result<bool> {
    if local_session_exists(profile, &manifest.session_id)? {
        return Ok(false);
    }
    let count = all_snapshot_dirs(profile)?
        .into_iter()
        .filter(|(candidate, _)| candidate.session_id == manifest.session_id)
        .count();
    Ok(count == 1)
}

fn backup_root(profile: &CodexProfile) -> PathBuf {
    profile
        .codex_home
        .join("backups")
        .join("codex-session-manager")
}

fn backup_id_for_manifest(profile: &CodexProfile, manifest_path: &Path) -> Result<String> {
    let dir = manifest_path
        .parent()
        .context("manifest path has no parent")?;
    let root = backup_root(profile);
    Ok(dir
        .strip_prefix(&root)
        .unwrap_or(dir)
        .to_string_lossy()
        .replace('\\', "/"))
}

fn backup_dir_from_id(profile: &CodexProfile, backup_id: &str) -> Result<PathBuf> {
    let root = backup_root(profile);
    let dir = root.join(backup_id.replace('/', std::path::MAIN_SEPARATOR_STR));
    if !dir.starts_with(&root) {
        bail!("backup id resolves outside backup root");
    }
    Ok(dir)
}

fn read_manifest(path: &Path) -> Result<SessionBackupManifest> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read backup manifest {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("failed to parse backup manifest {}", path.display()))
}

fn raw_index_entries_for_session(path: &Path, session_id: &str) -> Result<Vec<serde_json::Value>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read session index {}", path.display()))?;
    let mut entries = Vec::new();
    for (line_number, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value = serde_json::from_str::<serde_json::Value>(line).with_context(|| {
            format!(
                "failed to parse {} line {}",
                path.display(),
                line_number + 1
            )
        })?;
        if value.get("id").and_then(|value| value.as_str()) == Some(session_id) {
            entries.push(value);
        }
    }
    Ok(entries)
}

fn sqlite_thread(profile: &CodexProfile, session_id: &str) -> Result<Option<ThreadRecord>> {
    if !profile.state_db_path().exists() {
        return Ok(None);
    }
    let Ok(db) = StateDb::open(&profile.state_db_path()) else {
        return Ok(None);
    };
    let Ok(threads) = db.read_threads() else {
        return Ok(None);
    };
    Ok(threads.into_iter().find(|thread| thread.id == session_id))
}

fn rollout_project(path: Option<&Path>) -> Option<String> {
    path.and_then(|path| crate::rollout::read_rollout_meta(path).ok().flatten())
        .and_then(|meta| meta.cwd)
}

fn local_session_exists(profile: &CodexProfile, session_id: &str) -> Result<bool> {
    Ok(locate_unique_local_session(profile, session_id)?.is_some())
}

fn file_size(path: &Path) -> Result<u64> {
    Ok(fs::metadata(path)
        .with_context(|| format!("failed to stat {}", path.display()))?
        .len())
}

fn dir_size(path: &Path) -> Result<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let mut size = 0;
    for entry in WalkDir::new(path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            size += entry.metadata()?.len();
        }
    }
    Ok(size)
}

#[allow(dead_code)]
fn _typed_index_entries(profile: &CodexProfile) -> Result<()> {
    let _ = read_session_index(&profile.session_index_path())?;
    Ok(())
}
