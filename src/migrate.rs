use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::path_map::apply_first_path_map;
use crate::profile::CodexProfile;
use crate::rollout::{read_all_rollout_meta, rewrite_session_meta};
use crate::session_index::{
    append_session_index_entries, missing_user_index_entries, read_session_index,
    update_session_index_thread_names,
};
use crate::state_db::StateDb;

#[derive(Debug, Clone)]
pub struct ApplyOptions {
    pub apply: bool,
}

impl Default for ApplyOptions {
    fn default() -> Self {
        Self {
            apply: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MutationReport {
    pub action: String,
    pub applied: bool,
    pub sqlite_rows: usize,
    pub jsonl_files: usize,
    pub index_entries: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEdit {
    pub provider: Option<String>,
    pub project: Option<String>,
    pub title: Option<String>,
    pub title_prefix: Option<String>,
}

impl MutationReport {
    pub fn to_text(&self) -> String {
        let lines = vec![
            format!("action: {}", self.action),
            format!("mode: {}", if self.applied { "applied" } else { "dry-run" }),
            format!("sqlite rows: {}", self.sqlite_rows),
            format!("jsonl files: {}", self.jsonl_files),
            format!("session_index entries: {}", self.index_entries),
        ];

        lines.join("\n")
    }
}

pub fn edit_selected_sessions(
    profile: &CodexProfile,
    ids: &[String],
    edit: &SessionEdit,
    options: &ApplyOptions,
) -> Result<MutationReport> {
    let provider = edit
        .provider
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let project = edit
        .project
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let title = edit
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let title_prefix = edit
        .title_prefix
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut report = MutationReport {
        action: "edit selected sessions".to_string(),
        applied: options.apply,
        ..MutationReport::default()
    };

    if ids.is_empty()
        || (provider.is_none() && project.is_none() && title.is_none() && title_prefix.is_none())
    {
        return Ok(report);
    }

    let id_set = ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut db = StateDb::open(&profile.state_db_path())?;
    let threads = db.read_threads()?;
    let metas = read_all_rollout_meta(&profile.sessions_dir())?;
    let renames = build_session_renames(&threads, ids, title, title_prefix);

    report.sqlite_rows = threads
        .iter()
        .filter(|thread| id_set.contains(thread.id.as_str()))
        .filter(|thread| {
            provider.is_some_and(|value| thread.model_provider.as_deref() != Some(value))
                || project.is_some_and(|value| thread.cwd.as_deref() != Some(value))
                || renames.iter().any(|(id, new_title, _)| {
                    id == &thread.id && thread.title.as_deref() != Some(new_title)
                })
        })
        .count();
    report.jsonl_files = metas
        .iter()
        .filter(|meta| meta.id.as_deref().is_some_and(|id| id_set.contains(id)))
        .filter(|meta| {
            provider.is_some_and(|value| meta.model_provider.as_deref() != Some(value))
                || project.is_some_and(|value| meta.cwd.as_deref() != Some(value))
        })
        .count();
    report.index_entries = renames.len();

    if !options.apply {
        return Ok(report);
    }

    let sqlite_rows = report.sqlite_rows;
    db.update_selected_session_fields(ids, provider, project)?;
    db.update_session_titles(
        &renames
            .iter()
            .map(|(id, title, _)| (id.clone(), title.clone()))
            .collect::<Vec<_>>(),
    )?;
    report.sqlite_rows = sqlite_rows;
    report.index_entries =
        update_session_index_thread_names(&profile.session_index_path(), &renames)?;

    let mut jsonl_files = 0;
    for meta in metas {
        if !meta.id.as_deref().is_some_and(|id| id_set.contains(id)) {
            continue;
        }
        let changed = rewrite_session_meta(&meta.path, |payload| {
            let mut changed = false;
            if let Some(provider) = provider {
                if payload.model_provider.as_deref() != Some(provider) {
                    payload.model_provider = Some(provider.to_string());
                    changed = true;
                }
            }
            if let Some(project) = project {
                if payload.cwd.as_deref() != Some(project) {
                    payload.cwd = Some(project.to_string());
                    changed = true;
                }
            }
            changed
        })?;
        if changed {
            jsonl_files += 1;
        }
    }
    report.jsonl_files = jsonl_files;
    Ok(report)
}

fn build_session_renames(
    threads: &[crate::state_db::ThreadRecord],
    ids: &[String],
    title: Option<&str>,
    title_prefix: Option<&str>,
) -> Vec<(String, String, Option<String>)> {
    let id_set = ids.iter().map(String::as_str).collect::<HashSet<_>>();
    if let Some(title) = title {
        return threads
            .iter()
            .filter(|thread| id_set.contains(thread.id.as_str()))
            .map(|thread| {
                (
                    thread.id.clone(),
                    title.to_string(),
                    thread.updated_at.clone(),
                )
            })
            .collect();
    }

    let Some(title_prefix) = title_prefix else {
        return Vec::new();
    };

    let mut selected = threads
        .iter()
        .enumerate()
        .filter(|(_, thread)| id_set.contains(thread.id.as_str()))
        .collect::<Vec<_>>();
    selected.sort_by(|(left_index, left), (right_index, right)| {
        left.updated_at_ms
            .cmp(&right.updated_at_ms)
            .then(left.updated_at.cmp(&right.updated_at))
            .then(left.created_at_ms.cmp(&right.created_at_ms))
            .then(left.created_at.cmp(&right.created_at))
            .then(left_index.cmp(right_index))
    });

    selected
        .into_iter()
        .enumerate()
        .map(|(index, (_, thread))| {
            (
                thread.id.clone(),
                format!("{title_prefix}({})", index + 1),
                thread.updated_at.clone(),
            )
        })
        .collect()
}

pub fn migrate_provider(
    profile: &CodexProfile,
    from: &str,
    to: &str,
    options: &ApplyOptions,
) -> Result<MutationReport> {
    let mut report = MutationReport {
        action: format!("migrate provider {from} -> {to}"),
        applied: options.apply,
        ..MutationReport::default()
    };

    let mut db = StateDb::open(&profile.state_db_path())?;
    let threads = db.read_threads()?;
    let metas = read_all_rollout_meta(&profile.sessions_dir())?;

    report.sqlite_rows = threads
        .iter()
        .filter(|thread| thread.model_provider.as_deref() == Some(from))
        .count();
    report.jsonl_files = metas
        .iter()
        .filter(|meta| meta.model_provider.as_deref() == Some(from))
        .count();

    if !options.apply {
        return Ok(report);
    }

    report.sqlite_rows = db.update_provider(from, to)?;

    let mut jsonl_files = 0;
    for meta in metas {
        if meta.model_provider.as_deref() != Some(from) {
            continue;
        }
        let changed = rewrite_session_meta(&meta.path, |payload| {
            payload.model_provider = Some(to.to_string());
            true
        })?;
        if changed {
            jsonl_files += 1;
        }
    }
    report.jsonl_files = jsonl_files;
    Ok(report)
}

pub fn migrate_paths(profile: &CodexProfile, options: &ApplyOptions) -> Result<MutationReport> {
    let mut report = MutationReport {
        action: "migrate paths".to_string(),
        applied: options.apply,
        ..MutationReport::default()
    };

    let mut db = StateDb::open(&profile.state_db_path())?;
    let threads = db.read_threads()?;
    let metas = read_all_rollout_meta(&profile.sessions_dir())?;

    report.sqlite_rows = threads
        .iter()
        .filter(|thread| {
            thread
                .rollout_path
                .as_deref()
                .and_then(|value| apply_first_path_map(value, &profile.path_maps))
                .is_some()
                || thread
                    .cwd
                    .as_deref()
                    .and_then(|value| apply_first_path_map(value, &profile.path_maps))
                    .is_some()
        })
        .count();
    report.jsonl_files = metas
        .iter()
        .filter(|meta| {
            meta.cwd
                .as_deref()
                .and_then(|value| apply_first_path_map(value, &profile.path_maps))
                .is_some()
        })
        .count();

    if !options.apply {
        return Ok(report);
    }

    report.sqlite_rows = db.update_paths(&profile.path_maps)?;

    let mut jsonl_files = 0;
    for meta in metas {
        let Some(old_cwd) = meta.cwd.as_deref() else {
            continue;
        };
        let Some(new_cwd) = apply_first_path_map(old_cwd, &profile.path_maps) else {
            continue;
        };
        let changed = rewrite_session_meta(&meta.path, |payload| {
            payload.cwd = Some(new_cwd);
            true
        })?;
        if changed {
            jsonl_files += 1;
        }
    }
    report.jsonl_files = jsonl_files;
    Ok(report)
}

pub fn repair_session_index(
    profile: &CodexProfile,
    options: &ApplyOptions,
) -> Result<MutationReport> {
    let mut report = MutationReport {
        action: "repair session_index.jsonl".to_string(),
        applied: options.apply,
        ..MutationReport::default()
    };

    let db = StateDb::open(&profile.state_db_path())?;
    let threads = db.read_threads()?;
    let entries = read_session_index(&profile.session_index_path())?;
    let missing = missing_user_index_entries(&threads, &entries);
    report.index_entries = missing.len();

    if !options.apply {
        return Ok(report);
    }

    append_session_index_entries(&profile.session_index_path(), &missing)?;
    Ok(report)
}

pub fn repair_has_user_event(
    profile: &CodexProfile,
    options: &ApplyOptions,
) -> Result<MutationReport> {
    let mut report = MutationReport {
        action: "repair has_user_event".to_string(),
        applied: options.apply,
        ..MutationReport::default()
    };

    let mut db = StateDb::open(&profile.state_db_path())?;
    let threads = db.read_threads()?;
    report.sqlite_rows = threads
        .iter()
        .filter(|thread| {
            !thread.has_user_event
                && !thread.archived
                && matches!(thread.source.as_deref(), Some("cli" | "vscode"))
                && (thread.first_user_message.is_some() || thread.title.is_some())
        })
        .count();

    if !options.apply {
        return Ok(report);
    }

    report.sqlite_rows = db.repair_has_user_event()?;
    Ok(report)
}
