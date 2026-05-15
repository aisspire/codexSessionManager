use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::backup;
use crate::profile::CodexProfile;
use crate::rollout::{read_all_rollout_meta, RolloutMeta};
use crate::safety;
use crate::session_index::read_session_index;
use crate::state_db::{StateDb, ThreadRecord};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseRepairPreview {
    pub items: Vec<DatabaseRepairItem>,
    pub backup_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseRepairItem {
    pub id: String,
    pub kind: DatabaseRepairKind,
    pub session_id: String,
    pub summary: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub rollout_path: Option<String>,
    pub applicable: bool,
    pub skip_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DatabaseRepairKind {
    MissingThreadRow,
    RepairRolloutPath,
    NormalizeRolloutPath,
    SyncArchivedState,
    SqliteOnlyThread,
    DuplicateJsonl,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseRepairOptions {
    pub selected: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseRepairApplyReport {
    pub applied_items: usize,
    pub sqlite_rows: usize,
    pub backup_dir: Option<String>,
    pub backup_files: Vec<String>,
    pub skipped_items: Vec<DatabaseRepairItem>,
}

#[derive(Debug, Clone)]
struct LocatedMeta {
    meta: RolloutMeta,
    archived: bool,
}

pub fn preview_database_repairs(profile: &CodexProfile) -> Result<DatabaseRepairPreview> {
    let db = StateDb::open(&profile.state_db_path())?;
    let threads = db.read_threads()?;
    let metas_by_id = rollout_metas_by_id(profile)?;
    let threads_by_id = threads
        .iter()
        .map(|thread| (thread.id.as_str(), thread))
        .collect::<HashMap<_, _>>();
    let mut items = Vec::new();

    for (session_id, metas) in metas_by_id.iter() {
        if metas.len() != 1 {
            items.push(DatabaseRepairItem {
                id: item_id(DatabaseRepairKind::DuplicateJsonl, session_id),
                kind: DatabaseRepairKind::DuplicateJsonl,
                session_id: session_id.clone(),
                summary: "同一会话存在多个 JSONL，需人工确认".to_string(),
                before: Some(format!("{} files", metas.len())),
                after: None,
                rollout_path: None,
                applicable: false,
                skip_reason: Some("JSONL 文件不唯一，保守跳过".to_string()),
            });
            continue;
        }

        let located = &metas[0];
        let actual_path = located.meta.path.display().to_string();
        let Some(thread) = threads_by_id.get(session_id.as_str()) else {
            items.push(DatabaseRepairItem {
                id: item_id(DatabaseRepairKind::MissingThreadRow, session_id),
                kind: DatabaseRepairKind::MissingThreadRow,
                session_id: session_id.clone(),
                summary: "JSONL 存在但 SQLite threads 缺少行".to_string(),
                before: None,
                after: Some("create threads row".to_string()),
                rollout_path: Some(actual_path),
                applicable: true,
                skip_reason: None,
            });
            continue;
        };

        if let Some(kind) =
            rollout_path_repair_kind(thread.rollout_path.as_deref(), &located.meta.path)
        {
            items.push(DatabaseRepairItem {
                id: item_id(kind.clone(), session_id),
                kind,
                session_id: session_id.clone(),
                summary: "SQLite rollout_path 与当前 JSONL 路径不一致".to_string(),
                before: thread.rollout_path.clone(),
                after: Some(actual_path.clone()),
                rollout_path: Some(actual_path.clone()),
                applicable: true,
                skip_reason: None,
            });
        }

        if thread.archived != located.archived {
            items.push(DatabaseRepairItem {
                id: item_id(DatabaseRepairKind::SyncArchivedState, session_id),
                kind: DatabaseRepairKind::SyncArchivedState,
                session_id: session_id.clone(),
                summary: "SQLite archived 状态与 JSONL 所在目录不一致".to_string(),
                before: Some(
                    if thread.archived {
                        "archived"
                    } else {
                        "active"
                    }
                    .to_string(),
                ),
                after: Some(
                    if located.archived {
                        "archived"
                    } else {
                        "active"
                    }
                    .to_string(),
                ),
                rollout_path: Some(actual_path),
                applicable: true,
                skip_reason: None,
            });
        }
    }

    for thread in threads {
        if !metas_by_id.contains_key(&thread.id) {
            items.push(DatabaseRepairItem {
                id: item_id(DatabaseRepairKind::SqliteOnlyThread, &thread.id),
                kind: DatabaseRepairKind::SqliteOnlyThread,
                session_id: thread.id.clone(),
                summary: "SQLite threads 存在但未找到对应 JSONL".to_string(),
                before: thread.rollout_path.clone(),
                after: None,
                rollout_path: thread.rollout_path,
                applicable: false,
                skip_reason: Some("SQLite-only 行只报告，不删除".to_string()),
            });
        }
    }

    for item in &mut items {
        if item.kind == DatabaseRepairKind::SqliteOnlyThread {
            item.summary = "SQLite threads 存在但未找到对应 JSONL，可删除这条主记录".to_string();
            item.after = Some("delete threads row".to_string());
            item.applicable = true;
            item.skip_reason = None;
        }
    }

    items.sort_by(|left, right| {
        left.session_id
            .cmp(&right.session_id)
            .then(kind_rank(&left.kind).cmp(&kind_rank(&right.kind)))
    });

    Ok(DatabaseRepairPreview {
        items,
        backup_note: "apply 前会备份 state_5.sqlite、session_index.jsonl 及同目录关键文件到 backups/codex-session-manager-<timestamp>"
            .to_string(),
    })
}

pub fn preview_database_sync_from_local(profile: &CodexProfile) -> Result<DatabaseRepairPreview> {
    preview_database_repairs(profile)
}

pub fn apply_database_sync_from_local(profile: &CodexProfile) -> Result<DatabaseRepairApplyReport> {
    apply_database_sync_from_local_with_guard(profile, safety::ensure_codex_not_running)
}

pub fn apply_database_sync_from_local_with_guard<F>(
    profile: &CodexProfile,
    guard: F,
) -> Result<DatabaseRepairApplyReport>
where
    F: FnOnce() -> Result<()>,
{
    let preview = preview_database_sync_from_local(profile)?;
    let selected = preview
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    apply_database_repairs_with_guard(profile, &DatabaseRepairOptions { selected }, guard)
}

pub fn apply_database_repairs(
    profile: &CodexProfile,
    options: &DatabaseRepairOptions,
) -> Result<DatabaseRepairApplyReport> {
    apply_database_repairs_with_guard(profile, options, safety::ensure_codex_not_running)
}

pub fn apply_database_repairs_with_guard<F>(
    profile: &CodexProfile,
    options: &DatabaseRepairOptions,
    guard: F,
) -> Result<DatabaseRepairApplyReport>
where
    F: FnOnce() -> Result<()>,
{
    let preview = preview_database_repairs(profile)?;
    let selected = options
        .selected
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let selected_items = preview
        .items
        .into_iter()
        .filter(|item| selected.contains(item.id.as_str()))
        .collect::<Vec<_>>();
    let applicable = selected_items
        .iter()
        .filter(|item| item.applicable)
        .cloned()
        .collect::<Vec<_>>();
    let skipped_items = selected_items
        .into_iter()
        .filter(|item| !item.applicable)
        .collect::<Vec<_>>();

    if applicable.is_empty() {
        return Ok(DatabaseRepairApplyReport {
            applied_items: 0,
            sqlite_rows: 0,
            backup_dir: None,
            backup_files: Vec::new(),
            skipped_items,
        });
    }

    guard()?;
    let backup = backup::create_backup(profile, false)?;
    let mut db = StateDb::open(&profile.state_db_path())?;
    let repair_context = repair_context(profile)?;
    let mut sqlite_rows = 0;

    for item in &applicable {
        match item.kind {
            DatabaseRepairKind::MissingThreadRow => {
                let Some(located) = repair_context.unique_meta(&item.session_id) else {
                    continue;
                };
                let index = repair_context.index_by_id.get(&item.session_id);
                sqlite_rows += db.insert_repaired_thread(&ThreadRecord {
                    id: item.session_id.clone(),
                    rollout_path: Some(located.meta.path.display().to_string()),
                    cwd: located.meta.cwd.clone(),
                    source: located
                        .meta
                        .source
                        .clone()
                        .or_else(|| Some("cli".to_string())),
                    model_provider: located.meta.model_provider.clone(),
                    model: None,
                    reasoning_effort: None,
                    has_user_event: index.is_some(),
                    archived: located.archived,
                    created_at: None,
                    updated_at: index.and_then(|entry| entry.updated_at.clone()),
                    created_at_ms: None,
                    updated_at_ms: None,
                    title: index.and_then(|entry| entry.thread_name.clone()),
                    first_user_message: None,
                })?;
            }
            DatabaseRepairKind::RepairRolloutPath | DatabaseRepairKind::NormalizeRolloutPath => {
                if let Some(after) = item.after.as_deref() {
                    sqlite_rows += db.update_rollout_path(&item.session_id, after)?;
                }
            }
            DatabaseRepairKind::SyncArchivedState => {
                let archived = item.after.as_deref() == Some("archived");
                sqlite_rows += db.set_archived(std::slice::from_ref(&item.session_id), archived)?;
            }
            DatabaseRepairKind::SqliteOnlyThread => {
                sqlite_rows += db.delete_threads(std::slice::from_ref(&item.session_id))?;
            }
            DatabaseRepairKind::DuplicateJsonl => {}
        }
    }

    Ok(DatabaseRepairApplyReport {
        applied_items: applicable.len(),
        sqlite_rows,
        backup_dir: Some(backup.backup_dir.display().to_string()),
        backup_files: backup
            .copied_files
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        skipped_items,
    })
}

struct RepairContext {
    metas_by_id: HashMap<String, Vec<LocatedMeta>>,
    index_by_id: HashMap<String, crate::session_index::SessionIndexEntry>,
}

impl RepairContext {
    fn unique_meta(&self, id: &str) -> Option<&LocatedMeta> {
        let metas = self.metas_by_id.get(id)?;
        if metas.len() == 1 {
            metas.first()
        } else {
            None
        }
    }
}

fn repair_context(profile: &CodexProfile) -> Result<RepairContext> {
    Ok(RepairContext {
        metas_by_id: rollout_metas_by_id(profile)?,
        index_by_id: read_session_index(&profile.session_index_path())?
            .into_iter()
            .map(|entry| (entry.id.clone(), entry))
            .collect(),
    })
}

fn rollout_metas_by_id(profile: &CodexProfile) -> Result<HashMap<String, Vec<LocatedMeta>>> {
    let active = read_all_rollout_meta(&profile.sessions_dir())?
        .into_iter()
        .map(|meta| LocatedMeta {
            meta,
            archived: false,
        });
    let archived = read_all_rollout_meta(&profile.archived_sessions_dir())?
        .into_iter()
        .map(|meta| LocatedMeta {
            meta,
            archived: true,
        });
    let mut by_id: HashMap<String, Vec<LocatedMeta>> = HashMap::new();

    for located in active.chain(archived) {
        let Some(id) = located.meta.id.clone() else {
            continue;
        };
        by_id.entry(id).or_default().push(located);
    }

    Ok(by_id)
}

fn rollout_path_repair_kind(
    current: Option<&str>,
    actual_path: &Path,
) -> Option<DatabaseRepairKind> {
    let actual = actual_path.display().to_string();
    let Some(current) = current.map(str::trim).filter(|value| !value.is_empty()) else {
        return Some(DatabaseRepairKind::RepairRolloutPath);
    };
    if normalize_for_compare(current) == normalize_for_compare(&actual) {
        return None;
    }
    if is_wsl_mount_path(current) {
        return Some(DatabaseRepairKind::NormalizeRolloutPath);
    }
    if !PathBuf::from(current).exists() {
        return Some(DatabaseRepairKind::RepairRolloutPath);
    }
    None
}

fn normalize_for_compare(value: &str) -> String {
    value.trim().replace('\\', "/").to_ascii_lowercase()
}

fn is_wsl_mount_path(value: &str) -> bool {
    let text = value.trim().replace('\\', "/");
    let Some(rest) = text.strip_prefix("/mnt/") else {
        return false;
    };
    rest.chars()
        .next()
        .is_some_and(|drive| drive.is_ascii_alphabetic())
        && rest.as_bytes().get(1) == Some(&b'/')
}

fn item_id(kind: DatabaseRepairKind, session_id: &str) -> String {
    format!("{kind:?}:{session_id}")
}

fn kind_rank(kind: &DatabaseRepairKind) -> u8 {
    match kind {
        DatabaseRepairKind::MissingThreadRow => 0,
        DatabaseRepairKind::RepairRolloutPath => 1,
        DatabaseRepairKind::NormalizeRolloutPath => 2,
        DatabaseRepairKind::SyncArchivedState => 3,
        DatabaseRepairKind::SqliteOnlyThread => 4,
        DatabaseRepairKind::DuplicateJsonl => 5,
    }
}
