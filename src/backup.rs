use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::profile::CodexProfile;

#[derive(Debug, Clone)]
pub struct BackupResult {
    pub backup_dir: PathBuf,
    pub copied_files: Vec<PathBuf>,
    pub manifest_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub created_at_unix: i64,
    pub entries: Vec<BackupManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifestEntry {
    pub original_path: String,
    pub backup_path: String,
}

pub fn create_backup(profile: &CodexProfile, include_sessions: bool) -> Result<BackupResult> {
    let created_at_unix = OffsetDateTime::now_utc().unix_timestamp();
    let backup_dir = profile
        .codex_home
        .join("backups")
        .join(format!("codex-session-manager-{}", created_at_unix));
    fs::create_dir_all(&backup_dir)
        .with_context(|| format!("failed to create backup dir {}", backup_dir.display()))?;

    let mut copied_files = Vec::new();
    let mut manifest_entries = Vec::new();
    for path in key_backup_files(profile) {
        if path.exists() {
            let file_name = path
                .file_name()
                .context("backup source has no file name")?
                .to_os_string();
            let target = backup_dir.join(file_name);
            fs::copy(&path, &target).with_context(|| {
                format!("failed to copy {} to {}", path.display(), target.display())
            })?;
            copied_files.push(target);
            manifest_entries.push(BackupManifestEntry {
                original_path: path.display().to_string(),
                backup_path: copied_files.last().unwrap().display().to_string(),
            });
        }
    }

    if include_sessions && profile.sessions_dir().exists() {
        let target = backup_dir.join("sessions");
        copy_dir_recursive(&profile.sessions_dir(), &target)?;
        copied_files.push(target);
        manifest_entries.push(BackupManifestEntry {
            original_path: profile.sessions_dir().display().to_string(),
            backup_path: copied_files.last().unwrap().display().to_string(),
        });
    }

    let manifest = BackupManifest {
        created_at_unix,
        entries: manifest_entries,
    };
    let manifest_path = backup_dir.join("manifest.json");
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?).with_context(|| {
        format!(
            "failed to write backup manifest {}",
            manifest_path.display()
        )
    })?;

    Ok(BackupResult {
        backup_dir,
        copied_files,
        manifest_path: Some(manifest_path),
    })
}

fn key_backup_files(profile: &CodexProfile) -> Vec<PathBuf> {
    let db = profile.state_db_path();
    vec![
        db.clone(),
        db.with_file_name("state_5.sqlite-wal"),
        db.with_file_name("state_5.sqlite-shm"),
        profile.session_index_path(),
        profile.config_path(),
        profile.global_state_path(),
    ]
}

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let target = to.join(entry.file_name());
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
