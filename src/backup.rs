use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use time::OffsetDateTime;

use crate::profile::CodexProfile;

#[derive(Debug, Clone)]
pub struct BackupResult {
    pub backup_dir: PathBuf,
    pub copied_files: Vec<PathBuf>,
}

pub fn create_backup(profile: &CodexProfile, include_sessions: bool) -> Result<BackupResult> {
    let backup_dir = profile.codex_home.join("backups").join(format!(
        "codex-session-manager-{}",
        OffsetDateTime::now_utc().unix_timestamp()
    ));
    fs::create_dir_all(&backup_dir)
        .with_context(|| format!("failed to create backup dir {}", backup_dir.display()))?;

    let mut copied_files = Vec::new();
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
        }
    }

    if include_sessions && profile.sessions_dir().exists() {
        let target = backup_dir.join("sessions");
        copy_dir_recursive(&profile.sessions_dir(), &target)?;
        copied_files.push(target);
    }

    Ok(BackupResult {
        backup_dir,
        copied_files,
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
