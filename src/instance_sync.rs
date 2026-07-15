use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use toml_edit::{DocumentMut, Item, Table};

use crate::backup;
use crate::instance_registry::{list_managed_instances, ManagedInstance};
use crate::path_map::path_buf_for_current_os;
use crate::profile::CodexProfile;
use crate::rollout::{read_all_rollout_meta, RolloutMeta};
use crate::safety;
use crate::state_db::{StateDb, ThreadRecord};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSyncRequest {
    pub source_instance_id: i64,
    pub target_instance_ids: Vec<i64>,
    pub session_ids: Vec<String>,
    pub config_paths: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSyncSourceData {
    pub source_instance_id: i64,
    pub sessions: Vec<InstanceSyncSourceSession>,
    pub config_paths: Vec<ConfigPathNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSyncSourceSession {
    pub id: String,
    pub title: Option<String>,
    pub project: Option<String>,
    pub archived: bool,
    pub source: Option<String>,
    pub model_provider: Option<String>,
    pub model: Option<String>,
    pub source_path: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSyncConfigDiffRequest {
    pub source_instance_id: i64,
    pub target_instance_ids: Vec<i64>,
    pub config_path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSyncConfigDiff {
    pub source_instance_id: i64,
    pub config_path: Vec<String>,
    pub source_value: String,
    pub targets: Vec<InstanceSyncConfigDiffTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSyncConfigDiffTarget {
    pub target_instance_id: i64,
    pub target_path: String,
    pub status: InstanceSyncConfigDiffStatus,
    pub original_value: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceSyncConfigDiffStatus {
    Changed,
    Same,
    Missing,
    ReadError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigPathNode {
    pub path: Vec<String>,
    pub label: String,
    pub selectable: bool,
    pub children: Vec<ConfigPathNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSyncExecutionReport {
    pub source_instance_id: i64,
    pub targets: Vec<InstanceSyncTargetReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSyncPreview {
    pub source_instance_id: i64,
    pub session_count: usize,
    pub config_path_count: usize,
    pub targets: Vec<InstanceSyncTargetPreview>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSyncTargetPreview {
    pub target_instance_id: i64,
    pub target_path: String,
    pub sessions_to_add: Vec<String>,
    pub sessions_to_skip: Vec<String>,
    pub session_conflicts: Vec<InstanceSyncConflict>,
    pub config_paths_to_apply: usize,
    pub project_path_warnings: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSyncTargetReport {
    pub target_instance_id: i64,
    pub target_path: String,
    pub backup_dir: Option<String>,
    pub sessions_added: Vec<String>,
    pub sessions_skipped: Vec<String>,
    pub session_conflicts: Vec<InstanceSyncConflict>,
    pub index_entries: usize,
    pub sqlite_rows: usize,
    pub config_paths_applied: usize,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceSyncConflict {
    pub session_id: String,
    pub reason: String,
    pub target_path: Option<String>,
}

#[derive(Debug, Clone)]
struct LocatedSession {
    id: String,
    path: PathBuf,
    relative_path: PathBuf,
    archived: bool,
    meta: RolloutMeta,
}

#[derive(Debug, Clone)]
struct ImportedSession {
    source: LocatedSession,
    target_path: PathBuf,
}

pub fn execute_instance_sync(
    registry_database_path: &Path,
    request: &InstanceSyncRequest,
) -> Result<InstanceSyncExecutionReport> {
    execute_instance_sync_with_guard(
        registry_database_path,
        request,
        safety::ensure_codex_not_running,
    )
}

pub fn preview_instance_sync(
    registry_database_path: &Path,
    request: &InstanceSyncRequest,
) -> Result<InstanceSyncPreview> {
    let prepared = prepare_request(registry_database_path, request)?;
    let mut targets = Vec::with_capacity(prepared.targets.len());
    for target in &prepared.targets {
        let target_id = target.id;
        let target_path = target.path.display().to_string();
        match preview_target(
            &prepared.source,
            target,
            &prepared.sessions,
            &prepared.config_paths,
        ) {
            Ok(preview) => targets.push(preview),
            Err(error) => targets.push(InstanceSyncTargetPreview {
                target_instance_id: target_id,
                target_path,
                sessions_to_add: Vec::new(),
                sessions_to_skip: Vec::new(),
                session_conflicts: Vec::new(),
                config_paths_to_apply: 0,
                project_path_warnings: Vec::new(),
                error: Some(format!("{error:#}")),
            }),
        }
    }
    Ok(InstanceSyncPreview {
        source_instance_id: request.source_instance_id,
        session_count: prepared.sessions.len(),
        config_path_count: prepared.config_paths.len(),
        targets,
    })
}

pub fn list_instance_sync_source_data(
    registry_database_path: &Path,
    source_instance_id: i64,
) -> Result<InstanceSyncSourceData> {
    let instances = list_managed_instances(registry_database_path)?;
    let source = resolve_available_instance(&instances, source_instance_id)?;
    let source_threads = source_threads_by_id(&source.profile)?;
    let source_index_entries =
        raw_session_index_entries_by_id(&source.profile.session_index_path())?;
    let mut sessions = source_sessions_by_id(&source.profile)?
        .into_values()
        .filter(|sessions| sessions.len() == 1)
        .filter_map(|sessions| sessions.into_iter().next())
        .map(|session| {
            let thread = source_threads.get(&session.id);
            let index = source_index_entries.get(&session.id);
            InstanceSyncSourceSession {
                id: session.id,
                title: thread
                    .and_then(|thread| thread.title.clone())
                    .or_else(|| thread.and_then(|thread| thread.first_user_message.clone()))
                    .or_else(|| raw_index_string(index, "thread_name")),
                project: session
                    .meta
                    .cwd
                    .clone()
                    .or_else(|| thread.and_then(|thread| thread.cwd.clone())),
                archived: session.archived,
                source: session
                    .meta
                    .source
                    .clone()
                    .or_else(|| thread.and_then(|thread| thread.source.clone())),
                model_provider: session
                    .meta
                    .model_provider
                    .clone()
                    .or_else(|| thread.and_then(|thread| thread.model_provider.clone())),
                model: thread
                    .and_then(|thread| thread.model.clone())
                    .or_else(|| raw_index_string(index, "model")),
                source_path: session.path.display().to_string(),
                updated_at: raw_index_string(index, "updated_at")
                    .or_else(|| thread.and_then(|thread| thread.updated_at.clone())),
            }
        })
        .collect::<Vec<_>>();
    sessions.sort_by(|left, right| left.id.cmp(&right.id));
    let config_document = read_config_document(&source.profile.config_path())?;

    Ok(InstanceSyncSourceData {
        source_instance_id,
        sessions,
        config_paths: config_path_tree(config_document.as_table(), &[]),
    })
}

pub fn preview_instance_sync_config_diff(
    registry_database_path: &Path,
    request: &InstanceSyncConfigDiffRequest,
) -> Result<InstanceSyncConfigDiff> {
    let config_path =
        normalized_config_paths(&[request.config_path.clone()]).and_then(|mut paths| {
            paths
                .pop()
                .ok_or_else(|| anyhow::anyhow!("instance sync config path cannot be empty"))
        })?;
    let instances = list_managed_instances(registry_database_path)?;
    let source = resolve_available_instance(&instances, request.source_instance_id)?;
    let target_ids =
        normalized_target_ids(request.source_instance_id, &request.target_instance_ids)?;
    let targets = target_ids
        .into_iter()
        .map(|target_id| resolve_available_instance(&instances, target_id))
        .collect::<Result<Vec<_>>>()?;
    let source_document = read_config_document(&source.profile.config_path())?;
    let source_item = config_item_at_path(&source_document, &config_path).ok_or_else(|| {
        anyhow::anyhow!(
            "source config path {} does not exist",
            config_path_label(&config_path)
        )
    })?;
    if source_item.is_table() {
        bail!(
            "source config path {} is a table; select an individual value instead",
            config_path_label(&config_path)
        );
    }

    let source_value = formatted_config_item(source_item);
    let targets = targets
        .iter()
        .map(|target| config_diff_target(target, &config_path, source_item))
        .collect();

    Ok(InstanceSyncConfigDiff {
        source_instance_id: request.source_instance_id,
        config_path,
        source_value,
        targets,
    })
}

fn config_diff_target(
    target: &SyncInstance,
    config_path: &[String],
    source_item: &Item,
) -> InstanceSyncConfigDiffTarget {
    let target_path = target.path.display().to_string();
    let result = (|| -> Result<InstanceSyncConfigDiffTarget> {
        let document = read_config_document(&target.profile.config_path())?;
        let Some(target_item) = config_item_at_path(&document, config_path) else {
            return Ok(InstanceSyncConfigDiffTarget {
                target_instance_id: target.id,
                target_path: target_path.clone(),
                status: InstanceSyncConfigDiffStatus::Missing,
                original_value: None,
                error: None,
            });
        };
        if target_item.is_table() {
            bail!(
                "target config path {} is a table, not a value",
                config_path_label(config_path)
            );
        }

        Ok(InstanceSyncConfigDiffTarget {
            target_instance_id: target.id,
            target_path: target_path.clone(),
            status: if config_items_match(source_item, target_item) {
                InstanceSyncConfigDiffStatus::Same
            } else {
                InstanceSyncConfigDiffStatus::Changed
            },
            original_value: Some(formatted_config_item(target_item)),
            error: None,
        })
    })();

    result.unwrap_or_else(|error| InstanceSyncConfigDiffTarget {
        target_instance_id: target.id,
        target_path,
        status: InstanceSyncConfigDiffStatus::ReadError,
        original_value: None,
        error: Some(format!("{error:#}")),
    })
}

fn formatted_config_item(item: &Item) -> String {
    item.to_string().trim().to_string()
}

fn config_items_match(left: &Item, right: &Item) -> bool {
    formatted_config_item(left) == formatted_config_item(right)
}

pub fn execute_instance_sync_with_guard<F>(
    registry_database_path: &Path,
    request: &InstanceSyncRequest,
    mut guard: F,
) -> Result<InstanceSyncExecutionReport>
where
    F: FnMut() -> Result<()>,
{
    let prepared = prepare_request(registry_database_path, request)?;

    let mut reports = Vec::with_capacity(prepared.targets.len());
    let mut targets = prepared.targets.into_iter();
    while let Some(target) = targets.next() {
        if let Err(error) = guard() {
            if reports.is_empty() {
                return Err(error);
            }

            let error = format!("同步已停止：{error:#}");
            reports.push(blocked_target_report(target, error.clone()));
            reports
                .extend(targets.map(|remaining_target| {
                    blocked_target_report(remaining_target, error.clone())
                }));
            break;
        }
        reports.push(sync_target(
            &prepared.source,
            &target,
            &prepared.sessions,
            &prepared.config_paths,
        ));
    }

    Ok(InstanceSyncExecutionReport {
        source_instance_id: request.source_instance_id,
        targets: reports,
    })
}

fn blocked_target_report(target: SyncInstance, error: String) -> InstanceSyncTargetReport {
    let mut report = empty_target_report(&target);
    report.error = Some(error);
    report
}

fn empty_target_report(target: &SyncInstance) -> InstanceSyncTargetReport {
    InstanceSyncTargetReport {
        target_instance_id: target.id,
        target_path: target.path.display().to_string(),
        backup_dir: None,
        sessions_added: Vec::new(),
        sessions_skipped: Vec::new(),
        session_conflicts: Vec::new(),
        index_entries: 0,
        sqlite_rows: 0,
        config_paths_applied: 0,
        warnings: Vec::new(),
        error: None,
    }
}

struct PreparedRequest {
    source: SyncInstance,
    targets: Vec<SyncInstance>,
    sessions: Vec<LocatedSession>,
    config_paths: Vec<Vec<String>>,
}

#[derive(Debug, Clone)]
struct SyncInstance {
    id: i64,
    path: PathBuf,
    profile: CodexProfile,
}

fn prepare_request(
    registry_database_path: &Path,
    request: &InstanceSyncRequest,
) -> Result<PreparedRequest> {
    let session_ids = normalized_session_ids(&request.session_ids)?;
    let config_paths = normalized_config_paths(&request.config_paths)?;
    if session_ids.is_empty() && config_paths.is_empty() {
        bail!("instance sync request must select at least one session or configuration path");
    }

    let instances = list_managed_instances(registry_database_path)?;
    let source = resolve_available_instance(&instances, request.source_instance_id)?;
    let target_ids =
        normalized_target_ids(request.source_instance_id, &request.target_instance_ids)?;
    let targets = target_ids
        .into_iter()
        .map(|target_id| resolve_available_instance(&instances, target_id))
        .collect::<Result<Vec<_>>>()?;
    let sessions = selected_source_sessions(&source.profile, &session_ids)?;
    validate_source_config_paths(&source.profile, &config_paths)?;

    Ok(PreparedRequest {
        source,
        targets,
        sessions,
        config_paths,
    })
}

fn resolve_available_instance(instances: &[ManagedInstance], id: i64) -> Result<SyncInstance> {
    let instance = instances
        .iter()
        .find(|instance| instance.id == id)
        .ok_or_else(|| anyhow::anyhow!("managed instance {id} does not exist"))?;
    if !instance.available {
        bail!("managed instance {id} is not available");
    }
    let path = PathBuf::from(&instance.path);
    let profile = CodexProfile::new(
        format!("managed-instance-{id}"),
        path.clone(),
        None,
        None,
        Vec::new(),
    )?;
    Ok(SyncInstance { id, path, profile })
}

fn normalized_target_ids(source_id: i64, target_ids: &[i64]) -> Result<Vec<i64>> {
    if source_id <= 0 {
        bail!("instance sync source must be a registered instance");
    }
    if target_ids.is_empty() {
        bail!("instance sync request must include at least one target instance");
    }

    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(target_ids.len());
    for target_id in target_ids {
        if *target_id <= 0 {
            bail!("instance sync target must be a registered instance");
        }
        if *target_id == source_id {
            bail!("instance sync source cannot also be a target");
        }
        if !seen.insert(*target_id) {
            bail!("instance sync targets cannot contain duplicates");
        }
        normalized.push(*target_id);
    }
    Ok(normalized)
}

fn normalized_session_ids(session_ids: &[String]) -> Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(session_ids.len());
    for id in session_ids {
        let id = id.trim();
        if id.is_empty() {
            bail!("instance sync session IDs cannot be empty");
        }
        if !seen.insert(id.to_string()) {
            bail!("instance sync session IDs cannot contain duplicates");
        }
        normalized.push(id.to_string());
    }
    Ok(normalized)
}

fn normalized_config_paths(config_paths: &[Vec<String>]) -> Result<Vec<Vec<String>>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(config_paths.len());
    for path in config_paths {
        if path.is_empty() || path.iter().any(|segment| segment.trim().is_empty()) {
            bail!("instance sync config paths cannot be empty");
        }
        let path = path
            .iter()
            .map(|segment| segment.trim().to_string())
            .collect::<Vec<_>>();
        let encoded =
            serde_json::to_string(&path).context("failed to validate instance sync config path")?;
        if !seen.insert(encoded) {
            bail!("instance sync config paths cannot contain duplicates");
        }
        normalized.push(path);
    }
    Ok(normalized)
}

fn selected_source_sessions(
    profile: &CodexProfile,
    session_ids: &[String],
) -> Result<Vec<LocatedSession>> {
    if session_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut by_id = source_sessions_by_id(profile)?;
    session_ids
        .iter()
        .map(|session_id| {
            let sessions = by_id
                .remove(session_id)
                .ok_or_else(|| anyhow::anyhow!("source session {session_id} does not exist"))?;
            if sessions.len() != 1 {
                bail!(
                    "source session {session_id} has multiple JSONL files; refusing to choose one"
                );
            }
            Ok(sessions.into_iter().next().expect("length already checked"))
        })
        .collect()
}

fn source_sessions_by_id(profile: &CodexProfile) -> Result<HashMap<String, Vec<LocatedSession>>> {
    let mut by_id: HashMap<String, Vec<LocatedSession>> = HashMap::new();
    for (directory, archived) in [
        (profile.sessions_dir(), false),
        (profile.archived_sessions_dir(), true),
    ] {
        for meta in read_all_rollout_meta(&directory)? {
            let Some(id) = meta.id.clone().filter(|id| !id.trim().is_empty()) else {
                continue;
            };
            let relative_path = meta.path.strip_prefix(&directory).with_context(|| {
                format!(
                    "session {} is not under expected source directory {}",
                    meta.path.display(),
                    directory.display()
                )
            })?;
            by_id.entry(id.clone()).or_default().push(LocatedSession {
                id,
                path: meta.path.clone(),
                relative_path: relative_path.to_path_buf(),
                archived,
                meta,
            });
        }
    }
    Ok(by_id)
}

fn preview_target(
    source: &SyncInstance,
    target: &SyncInstance,
    sessions: &[LocatedSession],
    config_paths: &[Vec<String>],
) -> Result<InstanceSyncTargetPreview> {
    if !target.path.is_dir() || !target.profile.config_path().is_file() {
        bail!("managed instance {} is no longer available", target.id);
    }

    let _prepared_target_config = prepare_target_config_sync(source, target, config_paths)?;
    let existing_by_id = target_sessions_by_id(&target.profile)?;
    let target_thread_ids = target_thread_ids(&target.profile)?;
    let source_index_entries =
        raw_session_index_entries_by_id(&source.profile.session_index_path())?;
    let target_index_values = read_raw_session_index(&target.profile.session_index_path())?;
    let target_index_entries = session_index_entries_by_id(&target_index_values);
    let mut preview = InstanceSyncTargetPreview {
        target_instance_id: target.id,
        target_path: target.path.display().to_string(),
        sessions_to_add: Vec::new(),
        sessions_to_skip: Vec::new(),
        session_conflicts: Vec::new(),
        config_paths_to_apply: config_paths.len(),
        project_path_warnings: Vec::new(),
        error: None,
    };

    for session in sessions {
        let target_root = if session.archived {
            target.profile.archived_sessions_dir()
        } else {
            target.profile.sessions_dir()
        };
        let destination = target_root.join(&session.relative_path);
        let same_id_paths = existing_by_id.get(&session.id).cloned().unwrap_or_default();
        if same_id_paths.is_empty()
            && target_thread_ids
                .as_ref()
                .is_some_and(|ids| ids.contains(&session.id))
        {
            preview.session_conflicts.push(InstanceSyncConflict {
                session_id: session.id.clone(),
                reason: "目标 SQLite 已存在同 ID 会话，保守跳过".to_string(),
                target_path: None,
            });
            continue;
        }
        if let Some(reason) = existing_session_index_conflict(
            &source_index_entries,
            &target_index_entries,
            &session.id,
        ) {
            preview.session_conflicts.push(InstanceSyncConflict {
                session_id: session.id.clone(),
                reason,
                target_path: Some(target.profile.session_index_path().display().to_string()),
            });
            continue;
        }
        if let Some(conflict) = existing_session_conflict(session, &same_id_paths, &destination)? {
            match conflict {
                ExistingSessionState::SameContent => {
                    preview.sessions_to_skip.push(session.id.clone())
                }
                ExistingSessionState::Conflict(path) => {
                    preview.session_conflicts.push(InstanceSyncConflict {
                        session_id: session.id.clone(),
                        reason: "目标已存在同 ID 但内容不同的会话 JSONL".to_string(),
                        target_path: Some(path.display().to_string()),
                    })
                }
            }
            continue;
        }
        if destination.exists() {
            if files_equal(&session.path, &destination)? {
                preview.sessions_to_skip.push(session.id.clone());
            } else {
                preview.session_conflicts.push(InstanceSyncConflict {
                    session_id: session.id.clone(),
                    reason: "目标会话文件路径已被其他内容占用".to_string(),
                    target_path: Some(destination.display().to_string()),
                });
            }
            continue;
        }

        preview.sessions_to_add.push(session.id.clone());
        if let Some(warning) = missing_project_path_warning(session) {
            preview.project_path_warnings.push(warning);
        }
    }

    Ok(preview)
}

fn sync_target(
    source: &SyncInstance,
    target: &SyncInstance,
    sessions: &[LocatedSession],
    config_paths: &[Vec<String>],
) -> InstanceSyncTargetReport {
    let mut report = empty_target_report(target);
    let result = (|| -> Result<()> {
        // Re-check the target immediately before writing so a stale UI selection
        // cannot cause a write after its config file or directory disappeared.
        if !target.path.is_dir() || !target.profile.config_path().is_file() {
            bail!("managed instance {} is no longer available", target.id);
        }

        let prepared_target_config = prepare_target_config_sync(source, target, config_paths)?;
        let existing_by_id = target_sessions_by_id(&target.profile)?;
        let target_thread_ids = target_thread_ids(&target.profile)?;
        let source_threads = source_threads_by_id(&source.profile)?;
        let source_index_entries =
            raw_session_index_entries_by_id(&source.profile.session_index_path())?;
        let target_index_values = read_raw_session_index(&target.profile.session_index_path())?;
        let target_index_entries = session_index_entries_by_id(&target_index_values);
        let backup = backup::create_backup(&target.profile, false)?;
        report.backup_dir = Some(backup.backup_dir.display().to_string());
        let mut imported = Vec::new();

        for session in sessions {
            let target_root = if session.archived {
                target.profile.archived_sessions_dir()
            } else {
                target.profile.sessions_dir()
            };
            let destination = target_root.join(&session.relative_path);
            let same_id_paths = existing_by_id.get(&session.id).cloned().unwrap_or_default();
            if same_id_paths.is_empty()
                && target_thread_ids
                    .as_ref()
                    .is_some_and(|ids| ids.contains(&session.id))
            {
                report.session_conflicts.push(InstanceSyncConflict {
                    session_id: session.id.clone(),
                    reason: "目标 SQLite 已存在同 ID 会话，保守跳过".to_string(),
                    target_path: None,
                });
                continue;
            }
            if let Some(reason) = existing_session_index_conflict(
                &source_index_entries,
                &target_index_entries,
                &session.id,
            ) {
                report.session_conflicts.push(InstanceSyncConflict {
                    session_id: session.id.clone(),
                    reason,
                    target_path: Some(target.profile.session_index_path().display().to_string()),
                });
                continue;
            }
            if let Some(conflict) =
                existing_session_conflict(session, &same_id_paths, &destination)?
            {
                match conflict {
                    ExistingSessionState::SameContent => {
                        report.sessions_skipped.push(session.id.clone())
                    }
                    ExistingSessionState::Conflict(path) => {
                        report.session_conflicts.push(InstanceSyncConflict {
                            session_id: session.id.clone(),
                            reason: "目标已存在同 ID 但内容不同的会话 JSONL".to_string(),
                            target_path: Some(path.display().to_string()),
                        })
                    }
                }
                continue;
            }
            if destination.exists() {
                if files_equal(&session.path, &destination)? {
                    report.sessions_skipped.push(session.id.clone());
                } else {
                    report.session_conflicts.push(InstanceSyncConflict {
                        session_id: session.id.clone(),
                        reason: "目标会话文件路径已被其他内容占用".to_string(),
                        target_path: Some(destination.display().to_string()),
                    });
                }
                continue;
            }

            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!(
                        "failed to create target session directory {}",
                        parent.display()
                    )
                })?;
            }
            copy_session_file_new(&session.path, &destination)?;
            report.sessions_added.push(session.id.clone());
            imported.push(ImportedSession {
                source: session.clone(),
                target_path: destination,
            });
        }
        report.warnings.extend(
            imported
                .iter()
                .filter_map(|session| missing_project_path_warning(&session.source)),
        );
        report.index_entries = merge_imported_session_index(
            &target.profile.session_index_path(),
            target_index_values,
            &source_index_entries,
            &imported,
        )?;
        report.sqlite_rows = sync_imported_threads(
            &target.profile,
            &source_threads,
            &source_index_entries,
            &imported,
            &mut report.warnings,
        )?;
        if let Some(target_config) = prepared_target_config {
            write_target_config(target, target_config)?;
            report.config_paths_applied = config_paths.len();
        }

        Ok(())
    })();
    if let Err(error) = result {
        report.error = Some(format!("{error:#}"));
    }
    report
}

fn validate_source_config_paths(
    profile: &CodexProfile,
    config_paths: &[Vec<String>],
) -> Result<()> {
    if config_paths.is_empty() {
        return Ok(());
    }
    let document = read_config_document(&profile.config_path())?;
    for path in config_paths {
        let item = config_item_at_path(&document, path).ok_or_else(|| {
            anyhow::anyhow!(
                "source config path {} does not exist",
                config_path_label(path)
            )
        })?;
        if item.is_table() {
            bail!(
                "source config path {} is a table; select an individual value instead",
                config_path_label(path)
            );
        }
    }
    Ok(())
}

fn prepare_target_config_sync(
    source: &SyncInstance,
    target: &SyncInstance,
    config_paths: &[Vec<String>],
) -> Result<Option<DocumentMut>> {
    if config_paths.is_empty() {
        return Ok(None);
    }

    let source_document = read_config_document(&source.profile.config_path())?;
    let mut target_document = read_config_document(&target.profile.config_path())?;
    for path in config_paths {
        let source_item = config_item_at_path(&source_document, path)
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "source config path {} no longer exists",
                    config_path_label(path)
                )
            })?;
        if source_item.is_table() {
            bail!(
                "source config path {} is a table; select an individual value instead",
                config_path_label(path)
            );
        }
        set_config_item_at_path(&mut target_document, path, source_item)?;
    }
    Ok(Some(target_document))
}

fn write_target_config(target: &SyncInstance, target_document: DocumentMut) -> Result<()> {
    fs::write(target.profile.config_path(), target_document.to_string()).with_context(|| {
        format!(
            "failed to write target config {}",
            target.profile.config_path().display()
        )
    })
}

fn read_config_document(path: &Path) -> Result<DocumentMut> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read config {}", path.display()))?;
    text.parse::<DocumentMut>()
        .with_context(|| format!("failed to parse config {}", path.display()))
}

fn config_item_at_path<'a>(document: &'a DocumentMut, path: &[String]) -> Option<&'a Item> {
    let mut table = document.as_table();
    for (index, segment) in path.iter().enumerate() {
        let item = table.get(segment)?;
        if index + 1 == path.len() {
            return Some(item);
        }
        table = item.as_table()?;
    }
    None
}

fn set_config_item_at_path(document: &mut DocumentMut, path: &[String], item: Item) -> Result<()> {
    set_config_item_in_table(document.as_table_mut(), path, item)
}

fn set_config_item_in_table(table: &mut Table, path: &[String], item: Item) -> Result<()> {
    let (segment, remaining) = path
        .split_first()
        .expect("config paths were validated as non-empty");
    if remaining.is_empty() {
        table.insert(segment, item);
        return Ok(());
    }

    if !table.contains_key(segment) {
        table.insert(segment, Item::Table(Table::new()));
    }
    let child = table
        .get_mut(segment)
        .expect("inserted table should be addressable");
    let child_table = child.as_table_mut().ok_or_else(|| {
        anyhow::anyhow!(
            "target config path {} has a non-table parent",
            config_path_label(path)
        )
    })?;
    set_config_item_in_table(child_table, remaining, item)
}

fn config_path_label(path: &[String]) -> String {
    path.join(".")
}

fn config_path_tree(table: &Table, parent_path: &[String]) -> Vec<ConfigPathNode> {
    table
        .iter()
        .map(|(key, item)| {
            let mut path = parent_path.to_vec();
            path.push(key.to_string());
            let children = item
                .as_table()
                .map(|table| config_path_tree(table, &path))
                .unwrap_or_default();
            ConfigPathNode {
                label: key.to_string(),
                path,
                selectable: !item.is_table(),
                children,
            }
        })
        .collect()
}

fn target_thread_ids(profile: &CodexProfile) -> Result<Option<HashSet<String>>> {
    if !profile.state_db_path().is_file() {
        return Ok(None);
    }
    let database = StateDb::open(&profile.state_db_path())?;
    let ids = database
        .read_threads()?
        .into_iter()
        .map(|thread| thread.id)
        .collect();
    Ok(Some(ids))
}

fn source_threads_by_id(profile: &CodexProfile) -> Result<HashMap<String, ThreadRecord>> {
    if !profile.state_db_path().is_file() {
        return Ok(HashMap::new());
    }
    Ok(StateDb::open(&profile.state_db_path())?
        .read_threads()?
        .into_iter()
        .map(|thread| (thread.id.clone(), thread))
        .collect())
}

fn read_raw_session_index(path: &Path) -> Result<Vec<serde_json::Value>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read session index {}", path.display()))?;
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(line_number, line)| {
            serde_json::from_str(line).with_context(|| {
                format!(
                    "failed to parse session index {} line {}",
                    path.display(),
                    line_number + 1
                )
            })
        })
        .collect()
}

fn raw_session_index_entries_by_id(path: &Path) -> Result<HashMap<String, serde_json::Value>> {
    Ok(read_raw_session_index(path)?
        .into_iter()
        .filter_map(|value| {
            let id = value
                .get("id")
                .and_then(|id| id.as_str())
                .map(str::to_string)?;
            Some((id, value))
        })
        .collect())
}

fn session_index_entries_by_id(
    values: &[serde_json::Value],
) -> HashMap<String, Vec<&serde_json::Value>> {
    let mut entries = HashMap::new();
    for value in values {
        let Some(id) = value
            .get("id")
            .and_then(|id| id.as_str())
            .filter(|id| !id.trim().is_empty())
        else {
            continue;
        };
        entries
            .entry(id.to_string())
            .or_insert_with(Vec::new)
            .push(value);
    }
    entries
}

fn existing_session_index_conflict(
    source_entries: &HashMap<String, serde_json::Value>,
    target_entries: &HashMap<String, Vec<&serde_json::Value>>,
    session_id: &str,
) -> Option<String> {
    let target_values = target_entries.get(session_id)?;
    let Some(source_value) = source_entries.get(session_id) else {
        return Some("目标 session_index.jsonl 已存在同 ID 条目，但源端没有可合并条目".to_string());
    };
    if target_values
        .iter()
        .all(|target_value| *target_value == source_value)
    {
        return None;
    }
    Some("目标 session_index.jsonl 已存在同 ID 但内容不同的条目".to_string())
}

fn merge_imported_session_index(
    path: &Path,
    mut target_values: Vec<serde_json::Value>,
    source_entries: &HashMap<String, serde_json::Value>,
    imported: &[ImportedSession],
) -> Result<usize> {
    let mut existing_ids = target_values
        .iter()
        .filter_map(|value| {
            value
                .get("id")
                .and_then(|id| id.as_str())
                .map(str::to_string)
        })
        .collect::<HashSet<_>>();
    let mut added = 0;
    for session in imported {
        if !existing_ids.insert(session.source.id.clone()) {
            continue;
        }
        if let Some(value) = source_entries.get(&session.source.id) {
            target_values.push(value.clone());
            added += 1;
        }
    }
    if added == 0 {
        return Ok(0);
    }
    let text = target_values
        .into_iter()
        .map(|value| serde_json::to_string(&value))
        .collect::<serde_json::Result<Vec<_>>>()?
        .join("\n");
    fs::write(path, format!("{text}\n"))
        .with_context(|| format!("failed to write session index {}", path.display()))?;
    Ok(added)
}

fn sync_imported_threads(
    target_profile: &CodexProfile,
    source_threads: &HashMap<String, ThreadRecord>,
    source_index_entries: &HashMap<String, serde_json::Value>,
    imported: &[ImportedSession],
    warnings: &mut Vec<String>,
) -> Result<usize> {
    if imported.is_empty() {
        return Ok(0);
    }
    if !target_profile.state_db_path().is_file() {
        warnings.push("目标缺少 state_5.sqlite，已复制 JSONL 但未写入 SQLite threads".to_string());
        return Ok(0);
    }
    let mut database = StateDb::open(&target_profile.state_db_path())?;
    let mut changed = 0;
    for session in imported {
        let source_thread = source_threads.get(&session.source.id);
        let source_index = source_index_entries.get(&session.source.id);
        let imported_thread = imported_thread(session, source_thread, source_index);
        changed += database.upsert_thread(&imported_thread)?;
    }
    Ok(changed)
}

fn imported_thread(
    session: &ImportedSession,
    source_thread: Option<&ThreadRecord>,
    source_index: Option<&serde_json::Value>,
) -> ThreadRecord {
    ThreadRecord {
        id: session.source.id.clone(),
        rollout_path: Some(session.target_path.display().to_string()),
        cwd: session
            .source
            .meta
            .cwd
            .clone()
            .or_else(|| source_thread.and_then(|thread| thread.cwd.clone())),
        source: session
            .source
            .meta
            .source
            .clone()
            .or_else(|| source_thread.and_then(|thread| thread.source.clone()))
            .or_else(|| Some("cli".to_string())),
        model_provider: session
            .source
            .meta
            .model_provider
            .clone()
            .or_else(|| source_thread.and_then(|thread| thread.model_provider.clone())),
        model: source_thread.and_then(|thread| thread.model.clone()),
        reasoning_effort: source_thread.and_then(|thread| thread.reasoning_effort.clone()),
        has_user_event: source_index.is_some()
            || source_thread.is_some_and(|thread| thread.has_user_event),
        archived: session.source.archived,
        created_at: source_thread.and_then(|thread| thread.created_at.clone()),
        updated_at: raw_index_string(source_index, "updated_at")
            .or_else(|| source_thread.and_then(|thread| thread.updated_at.clone())),
        created_at_ms: source_thread.and_then(|thread| thread.created_at_ms),
        updated_at_ms: source_thread.and_then(|thread| thread.updated_at_ms),
        title: source_thread
            .and_then(|thread| thread.title.clone())
            .or_else(|| raw_index_string(source_index, "thread_name")),
        first_user_message: source_thread.and_then(|thread| thread.first_user_message.clone()),
    }
}

fn raw_index_string(value: Option<&serde_json::Value>, key: &str) -> Option<String> {
    value
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn target_sessions_by_id(profile: &CodexProfile) -> Result<HashMap<String, Vec<PathBuf>>> {
    let mut by_id: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for directory in [profile.sessions_dir(), profile.archived_sessions_dir()] {
        for meta in read_all_rollout_meta(&directory)? {
            if let Some(id) = meta.id.filter(|id| !id.trim().is_empty()) {
                by_id.entry(id).or_default().push(meta.path);
            }
        }
    }
    Ok(by_id)
}

enum ExistingSessionState {
    SameContent,
    Conflict(PathBuf),
}

fn existing_session_conflict(
    source: &LocatedSession,
    target_paths: &[PathBuf],
    destination: &Path,
) -> Result<Option<ExistingSessionState>> {
    let mut found_same_content = false;
    for target_path in target_paths {
        if files_equal(&source.path, target_path)? {
            found_same_content = true;
            continue;
        }
        return Ok(Some(ExistingSessionState::Conflict(target_path.clone())));
    }
    if found_same_content {
        return Ok(Some(ExistingSessionState::SameContent));
    }
    if destination.exists() {
        return Ok(Some(if files_equal(&source.path, destination)? {
            ExistingSessionState::SameContent
        } else {
            ExistingSessionState::Conflict(destination.to_path_buf())
        }));
    }
    Ok(None)
}

fn files_equal(left: &Path, right: &Path) -> Result<bool> {
    if fs::metadata(left)?.len() != fs::metadata(right)?.len() {
        return Ok(false);
    }
    Ok(fs::read(left)? == fs::read(right)?)
}

fn copy_session_file_new(source: &Path, destination: &Path) -> Result<()> {
    let mut source_file = fs::File::open(source)
        .with_context(|| format!("failed to open source session {}", source.display()))?;
    let mut destination_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .with_context(|| {
            format!(
                "failed to create target session {} without overwriting an existing file",
                destination.display()
            )
        })?;

    let result = (|| -> Result<()> {
        io::copy(&mut source_file, &mut destination_file).with_context(|| {
            format!(
                "failed to copy source session {} to {}",
                source.display(),
                destination.display()
            )
        })?;
        destination_file.sync_all().with_context(|| {
            format!(
                "failed to flush copied target session {}",
                destination.display()
            )
        })?;
        Ok(())
    })();
    drop(destination_file);

    if let Err(error) = result {
        fs::remove_file(destination).with_context(|| {
            format!(
                "failed to remove incomplete target session {}",
                destination.display()
            )
        })?;
        return Err(error);
    }
    Ok(())
}

fn missing_project_path_warning(session: &LocatedSession) -> Option<String> {
    let project = session.meta.cwd.as_deref()?;
    if path_buf_for_current_os(project).is_dir() {
        return None;
    }
    Some(format!(
        "会话 {} 的项目路径在目标环境中不存在，已按原样保留：{}",
        session.id, project
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::copy_session_file_new;

    #[test]
    fn copying_a_session_never_overwrites_a_path_created_after_preflight() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("source.jsonl");
        let destination = directory.path().join("destination.jsonl");
        fs::write(&source, "source session").unwrap();
        fs::write(&destination, "target session").unwrap();

        let error = copy_session_file_new(&source, &destination).unwrap_err();

        assert!(error.to_string().contains("without overwriting"));
        assert_eq!(fs::read_to_string(destination).unwrap(), "target session");
    }
}
