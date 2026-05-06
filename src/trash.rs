use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::profile::CodexProfile;
use crate::state_db::ThreadRecord;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrashManifest {
    pub created_at_unix: i64,
    pub entries: Vec<TrashEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrashEntry {
    pub session_id: String,
    pub original_path: String,
    pub trashed_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrashResult {
    pub manifest_path: PathBuf,
    pub manifest: TrashManifest,
}

pub fn move_threads_to_trash(
    profile: &CodexProfile,
    threads: &[ThreadRecord],
) -> Result<TrashResult> {
    let created_at_unix = OffsetDateTime::now_utc().unix_timestamp();
    let trash_dir = profile
        .codex_home
        .join("trash")
        .join("codex-session-manager")
        .join(created_at_unix.to_string());
    fs::create_dir_all(&trash_dir)
        .with_context(|| format!("failed to create trash dir {}", trash_dir.display()))?;

    let mut entries = Vec::new();
    for thread in threads {
        let Some(original) = thread
            .rollout_path
            .as_deref()
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        let original_path = PathBuf::from(original);
        if !original_path.exists() {
            continue;
        }
        let file_name = original_path
            .file_name()
            .context("rollout path has no file name")?;
        let trashed_path = trash_dir.join(format!("{}-{}", thread.id, file_name.to_string_lossy()));
        move_file(&original_path, &trashed_path)?;
        entries.push(TrashEntry {
            session_id: thread.id.clone(),
            original_path: original_path.display().to_string(),
            trashed_path: trashed_path.display().to_string(),
        });
    }

    let manifest = TrashManifest {
        created_at_unix,
        entries,
    };
    let manifest_path = trash_dir.join("manifest.json");
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)
        .with_context(|| format!("failed to write trash manifest {}", manifest_path.display()))?;

    Ok(TrashResult {
        manifest_path,
        manifest,
    })
}

fn move_file(from: &Path, to: &Path) -> Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(from, to).with_context(|| {
                format!("failed to copy {} to {}", from.display(), to.display())
            })?;
            fs::remove_file(from)
                .with_context(|| format!("failed to remove {}", from.display()))?;
            Ok(())
        }
    }
}
