use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::state_db::ThreadRecord;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionIndexEntry {
    pub id: String,
    pub thread_name: Option<String>,
    pub updated_at: Option<String>,
}

pub fn read_session_index(path: &Path) -> Result<Vec<SessionIndexEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read session index {}", path.display()))?;
    let mut entries = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let entry = serde_json::from_str(line).with_context(|| {
            format!(
                "failed to parse {} line {}",
                path.display(),
                line_number + 1
            )
        })?;
        entries.push(entry);
    }
    Ok(entries)
}

pub fn append_session_index_entries(path: &Path, entries: &[SessionIndexEntry]) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open session index {}", path.display()))?;

    for entry in entries {
        writeln!(file, "{}", serde_json::to_string(entry)?)?;
    }
    file.sync_all()?;
    Ok(())
}

pub fn missing_user_index_entries(
    threads: &[ThreadRecord],
    existing: &[SessionIndexEntry],
) -> Vec<SessionIndexEntry> {
    let indexed_ids: HashSet<&str> = existing.iter().map(|entry| entry.id.as_str()).collect();

    threads
        .iter()
        .filter(|thread| is_visible_user_thread(thread))
        .filter(|thread| !indexed_ids.contains(thread.id.as_str()))
        .map(|thread| SessionIndexEntry {
            id: thread.id.clone(),
            thread_name: thread
                .title
                .clone()
                .or_else(|| thread.first_user_message.clone()),
            updated_at: thread.updated_at.clone(),
        })
        .collect()
}

pub fn is_visible_user_thread(thread: &ThreadRecord) -> bool {
    if thread.archived || !thread.has_user_event {
        return false;
    }

    matches!(thread.source.as_deref(), Some("cli" | "vscode"))
}
