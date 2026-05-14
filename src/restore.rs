use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::backup::BackupManifest;
use crate::backup_store::{self, BackupTrigger, SessionBackupManifest};
use crate::favorites;
use crate::profile::CodexProfile;
use crate::rollout::read_rollout_meta;
use crate::safety;
use crate::state_db::{StateDb, ThreadRecord};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreReport {
    pub applied: bool,
    pub files_restored: usize,
    pub manifest_path: String,
    pub backup_id: Option<String>,
    pub session_id: Option<String>,
    pub restored_session_path: Option<String>,
    pub index_entries: usize,
    pub sqlite_rows: usize,
    pub preflight_backup_manifest: Option<String>,
    pub favorite_restored: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestorePreview {
    pub backup_id: String,
    pub session_id: String,
    pub restore_session_path: Option<String>,
    pub overwrites_existing: bool,
    pub index_entries: usize,
    pub favorite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreSessionOptions {
    pub apply: bool,
    pub overwrite_existing: bool,
    pub restore_favorite: bool,
}

#[derive(Debug, Clone)]
struct RestoreTarget {
    path: Option<PathBuf>,
    overwrites_existing: bool,
}

impl RestoreReport {
    pub fn to_text(&self) -> String {
        [
            "action: restore backup manifest".to_string(),
            format!("mode: {}", if self.applied { "applied" } else { "dry-run" }),
            format!("manifest: {}", self.manifest_path),
            format!("files restored: {}", self.files_restored),
        ]
        .join("\n")
    }
}

pub fn restore_from_manifest(
    manifest_path: &Path,
    only_original_paths: &[String],
    apply: bool,
) -> Result<RestoreReport> {
    restore_from_manifest_with_guard(
        manifest_path,
        only_original_paths,
        apply,
        safety::ensure_codex_not_running,
    )
}

pub fn restore_from_manifest_with_guard<F>(
    manifest_path: &Path,
    only_original_paths: &[String],
    apply: bool,
    guard: F,
) -> Result<RestoreReport>
where
    F: FnOnce() -> Result<()>,
{
    let manifest = read_manifest(manifest_path)?;
    let entries = manifest
        .entries
        .iter()
        .filter(|entry| {
            only_original_paths.is_empty()
                || only_original_paths
                    .iter()
                    .any(|path| path == &entry.original_path)
        })
        .collect::<Vec<_>>();

    let report = RestoreReport {
        applied: apply,
        files_restored: entries.len(),
        manifest_path: manifest_path.display().to_string(),
        backup_id: None,
        session_id: None,
        restored_session_path: None,
        index_entries: 0,
        sqlite_rows: 0,
        preflight_backup_manifest: None,
        favorite_restored: false,
        warnings: Vec::new(),
    };

    if !apply {
        return Ok(report);
    }

    guard()?;
    for entry in entries {
        restore_path(
            Path::new(&entry.backup_path),
            Path::new(&entry.original_path),
        )?;
    }
    Ok(report)
}

pub fn preview_restore_session_backup(
    profile: &CodexProfile,
    backup_id: &str,
) -> Result<RestorePreview> {
    let (manifest, _) = backup_store::read_session_backup_manifest(profile, backup_id)?;
    let target = restore_target(profile, &manifest)?;
    Ok(RestorePreview {
        backup_id: backup_id.to_string(),
        session_id: manifest.session_id,
        restore_session_path: target.path.map(|path| path.display().to_string()),
        overwrites_existing: target.overwrites_existing,
        index_entries: manifest.index_entries.len(),
        favorite: manifest.favorite,
    })
}

pub fn restore_session_backup(
    profile: &CodexProfile,
    backup_id: &str,
    options: &RestoreSessionOptions,
) -> Result<RestoreReport> {
    restore_session_backup_with_guard(
        profile,
        backup_id,
        options,
        safety::ensure_codex_not_running,
    )
}

pub fn restore_session_backup_with_guard<F>(
    profile: &CodexProfile,
    backup_id: &str,
    options: &RestoreSessionOptions,
    guard: F,
) -> Result<RestoreReport>
where
    F: FnOnce() -> Result<()>,
{
    let (manifest, manifest_path) = backup_store::read_session_backup_manifest(profile, backup_id)?;
    let target = restore_target(profile, &manifest)?;
    let mut report = RestoreReport {
        applied: options.apply,
        files_restored: usize::from(
            target.path.is_some() && manifest.backup_session_path.is_some(),
        ),
        manifest_path: manifest_path.display().to_string(),
        backup_id: Some(backup_id.to_string()),
        session_id: Some(manifest.session_id.clone()),
        restored_session_path: target.path.as_ref().map(|path| path.display().to_string()),
        index_entries: manifest.index_entries.len(),
        sqlite_rows: 0,
        preflight_backup_manifest: None,
        favorite_restored: false,
        warnings: Vec::new(),
    };

    if !options.apply {
        return Ok(report);
    }

    guard()?;
    let Some(target_path) = target.path else {
        bail!("backup {backup_id} does not contain a session JSONL copy");
    };
    if target.overwrites_existing {
        if !options.overwrite_existing {
            bail!(
                "restore target {} already exists; enable overwrite_existing to replace it",
                target_path.display()
            );
        }
        let preflight = backup_store::create_session_backup(
            profile,
            &manifest.session_id,
            BackupTrigger::RestorePreflight,
        )?;
        report.preflight_backup_manifest = Some(manifest_path_from_backup(&preflight)?);
    }

    let source = Path::new(
        manifest
            .backup_session_path
            .as_deref()
            .context("backup manifest does not contain backup_session_path")?,
    );
    restore_path(source, &target_path)?;
    report.index_entries = merge_index_entries(&profile.session_index_path(), &manifest)?;
    report.sqlite_rows = sync_sqlite_from_restored_session(profile, &manifest, &target_path)?;
    if options.restore_favorite && manifest.favorite {
        favorites::set_favorite(profile, &manifest.session_id, true)?;
        report.favorite_restored = true;
    }
    Ok(report)
}

fn read_manifest(path: &Path) -> Result<BackupManifest> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read backup manifest {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse backup manifest {}", path.display()))
}

fn restore_path(from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create restore dir {}", parent.display()))?;
    }

    if from.is_dir() {
        if to.exists() {
            fs::remove_dir_all(to).with_context(|| format!("failed to remove {}", to.display()))?;
        }
        copy_dir_recursive(from, to)
    } else {
        fs::copy(from, to)
            .with_context(|| format!("failed to restore {} to {}", from.display(), to.display()))?;
        Ok(())
    }
}

fn restore_target(
    profile: &CodexProfile,
    manifest: &SessionBackupManifest,
) -> Result<RestoreTarget> {
    let original = manifest.original_session_path.as_deref().map(PathBuf::from);
    if let Some(original) = original {
        if original.is_file() || !original.exists() {
            return Ok(RestoreTarget {
                overwrites_existing: original.exists(),
                path: Some(original),
            });
        }
    }

    let backup_path = manifest.backup_session_path.as_deref().map(PathBuf::from);
    let Some(file_name) = backup_path.as_deref().and_then(Path::file_name) else {
        return Ok(RestoreTarget {
            path: None,
            overwrites_existing: false,
        });
    };
    let path = restored_rollout_path(profile, file_name);
    Ok(RestoreTarget {
        overwrites_existing: path.exists(),
        path: Some(path),
    })
}

fn restored_rollout_path(profile: &CodexProfile, file_name: &OsStr) -> PathBuf {
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

fn merge_index_entries(path: &Path, manifest: &SessionBackupManifest) -> Result<usize> {
    if manifest.index_entries.is_empty() {
        return Ok(0);
    }

    let mut existing_values = Vec::new();
    let mut existing_ids = HashSet::new();
    if path.exists() {
        let text = fs::read_to_string(path)
            .with_context(|| format!("failed to read session index {}", path.display()))?;
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(line)?;
            if let Some(id) = value.get("id").and_then(|value| value.as_str()) {
                existing_ids.insert(id.to_string());
            }
            existing_values.push(value);
        }
    }

    let mut added = 0;
    for value in &manifest.index_entries {
        if let Some(id) = value.get("id").and_then(|value| value.as_str()) {
            if existing_ids.contains(id) {
                continue;
            }
            existing_ids.insert(id.to_string());
        }
        existing_values.push(value.clone());
        added += 1;
    }
    if added == 0 {
        return Ok(0);
    }

    let text = existing_values
        .into_iter()
        .map(|value| serde_json::to_string(&value))
        .collect::<serde_json::Result<Vec<_>>>()?
        .join("\n");
    fs::write(path, format!("{text}\n"))
        .with_context(|| format!("failed to write session index {}", path.display()))?;
    Ok(added)
}

fn sync_sqlite_from_restored_session(
    profile: &CodexProfile,
    manifest: &SessionBackupManifest,
    restored_path: &Path,
) -> Result<usize> {
    if !profile.state_db_path().exists() {
        return Ok(0);
    }
    let meta = read_rollout_meta(restored_path)?.unwrap_or_default();
    let sqlite_thread = manifest.sqlite_thread.as_ref();
    let title = manifest
        .index_entries
        .iter()
        .rev()
        .find_map(|value| value.get("thread_name").and_then(|value| value.as_str()))
        .map(str::to_string)
        .or_else(|| sqlite_thread.and_then(|thread| thread.title.clone()));
    let updated_at = manifest
        .index_entries
        .iter()
        .rev()
        .find_map(|value| value.get("updated_at").and_then(|value| value.as_str()))
        .map(str::to_string)
        .or_else(|| sqlite_thread.and_then(|thread| thread.updated_at.clone()));
    let archived = restored_path.starts_with(profile.archived_sessions_dir());
    let restored = ThreadRecord {
        id: manifest.session_id.clone(),
        rollout_path: Some(restored_path.display().to_string()),
        cwd: meta
            .cwd
            .or_else(|| sqlite_thread.and_then(|thread| thread.cwd.clone())),
        source: meta
            .source
            .or_else(|| sqlite_thread.and_then(|thread| thread.source.clone()))
            .or_else(|| Some("cli".to_string())),
        model_provider: meta
            .model_provider
            .or_else(|| sqlite_thread.and_then(|thread| thread.model_provider.clone())),
        model: sqlite_thread.and_then(|thread| thread.model.clone()),
        reasoning_effort: sqlite_thread.and_then(|thread| thread.reasoning_effort.clone()),
        has_user_event: !manifest.index_entries.is_empty()
            || sqlite_thread.is_some_and(|thread| thread.has_user_event),
        archived,
        created_at: sqlite_thread.and_then(|thread| thread.created_at.clone()),
        updated_at,
        created_at_ms: sqlite_thread.and_then(|thread| thread.created_at_ms),
        updated_at_ms: sqlite_thread.and_then(|thread| thread.updated_at_ms),
        title,
        first_user_message: sqlite_thread.and_then(|thread| thread.first_user_message.clone()),
    };
    let mut db = StateDb::open(&profile.state_db_path())?;
    db.upsert_restored_thread(&restored)
}

fn manifest_path_from_backup(manifest: &SessionBackupManifest) -> Result<String> {
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

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let target = PathBuf::from(to).join(entry.file_name());
        if source.is_dir() {
            copy_dir_recursive(&source, &target)?;
        } else {
            fs::copy(&source, &target).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    source.display(),
                    target.display()
                )
            })?;
        }
    }
    Ok(())
}
