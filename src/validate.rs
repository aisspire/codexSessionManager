use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;

use crate::profile::CodexProfile;
use crate::rollout::read_all_rollout_meta;
use crate::state_db::StateDb;

#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    pub sqlite_integrity: String,
    pub thread_count: usize,
    pub rollout_meta_count: usize,
    pub issues: Vec<String>,
}

impl ValidationReport {
    pub fn is_ok(&self) -> bool {
        self.sqlite_integrity == "ok" && self.issues.is_empty()
    }

    pub fn to_text(&self) -> String {
        let mut lines = vec![
            "Codex session validation report".to_string(),
            format!("sqlite integrity: {}", self.sqlite_integrity),
            format!("threads: {}", self.thread_count),
            format!("rollout metadata files: {}", self.rollout_meta_count),
            format!("issues: {}", self.issues.len()),
        ];

        lines.extend(self.issues.iter().map(|issue| format!("- {issue}")));
        lines.join("\n")
    }
}

pub fn validate_profile(profile: &CodexProfile) -> Result<ValidationReport> {
    let db = StateDb::open(&profile.state_db_path())?;
    let threads = db.read_threads()?;
    let metas = read_all_rollout_meta(&profile.sessions_dir())?;
    let sqlite_integrity = db.integrity_check()?;

    let meta_by_path: HashMap<&Path, _> = metas
        .iter()
        .map(|meta| (meta.path.as_path(), meta))
        .collect();
    let meta_ids: HashSet<&str> = metas.iter().filter_map(|meta| meta.id.as_deref()).collect();

    let mut issues = Vec::new();
    for thread in &threads {
        if let Some(path) = &thread.rollout_path {
            if !Path::new(path).exists() {
                issues.push(format!(
                    "thread {} rollout_path is missing: {}",
                    thread.id, path
                ));
            }

            if let Some(meta) = meta_by_path.get(Path::new(path)) {
                if meta.id.as_deref() != Some(thread.id.as_str()) {
                    issues.push(format!(
                        "thread {} has rollout JSONL id {}",
                        thread.id,
                        meta.id.as_deref().unwrap_or("<missing>")
                    ));
                }
            }
        }

        if !meta_ids.contains(thread.id.as_str()) {
            issues.push(format!(
                "thread {} has no matching JSONL session_meta id",
                thread.id
            ));
        }
    }

    Ok(ValidationReport {
        sqlite_integrity,
        thread_count: threads.len(),
        rollout_meta_count: metas.len(),
        issues,
    })
}
