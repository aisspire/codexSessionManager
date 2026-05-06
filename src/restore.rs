use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::backup::BackupManifest;
use crate::safety;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreReport {
    pub applied: bool,
    pub files_restored: usize,
    pub manifest_path: String,
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
