use std::path::PathBuf;

use codex_session_manager::app_server::{self, HttpAppServerTransport};
use codex_session_manager::backup;
use codex_session_manager::migrate::{self, ApplyOptions, SessionEdit};
use codex_session_manager::path_map::PathMap;
use codex_session_manager::profile::CodexProfile;
use codex_session_manager::restore;
use codex_session_manager::session_list::{self, SessionListFilter, SessionSummary};
use codex_session_manager::session_ops::{self, SessionApplyOptions, SessionMutationReport};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
struct ProfileInput {
    codex_home: String,
    profile: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    path_maps: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct BackupResponse {
    backup_dir: String,
    manifest_path: Option<String>,
    copied_entries: usize,
}

#[tauri::command]
fn list_sessions(
    profile: ProfileInput,
    filter: SessionListFilter,
) -> Result<Vec<SessionSummary>, String> {
    let profile = build_profile(profile)?;
    session_list::list_sessions(&profile, &filter).map_err(format_error)
}

#[tauri::command]
fn archive_sessions(
    profile: ProfileInput,
    ids: Vec<String>,
    apply: bool,
) -> Result<SessionMutationReport, String> {
    let profile = build_profile(profile)?;
    session_ops::archive_sessions(&profile, &ids, &apply_options(apply)).map_err(format_error)
}

#[tauri::command]
fn restore_sessions(
    profile: ProfileInput,
    ids: Vec<String>,
    apply: bool,
) -> Result<SessionMutationReport, String> {
    let profile = build_profile(profile)?;
    session_ops::restore_sessions(&profile, &ids, &apply_options(apply)).map_err(format_error)
}

#[tauri::command]
fn delete_sessions(
    profile: ProfileInput,
    ids: Vec<String>,
    apply: bool,
) -> Result<SessionMutationReport, String> {
    let profile = build_profile(profile)?;
    session_ops::delete_sessions(&profile, &ids, &apply_options(apply)).map_err(format_error)
}

#[tauri::command]
fn edit_selected_sessions(
    profile: ProfileInput,
    ids: Vec<String>,
    edit: SessionEdit,
    apply: bool,
) -> Result<migrate::MutationReport, String> {
    if ids.is_empty() {
        return Err("please select at least one session".to_string());
    }
    if edit
        .project
        .as_deref()
        .map_or(true, |value| value.trim().is_empty())
        && edit
            .provider
            .as_deref()
            .map_or(true, |value| value.trim().is_empty())
        && edit
            .title
            .as_deref()
            .map_or(true, |value| value.trim().is_empty())
        && edit
            .title_prefix
            .as_deref()
            .map_or(true, |value| value.trim().is_empty())
    {
        return Err("please enter a provider, project, title, or title prefix to edit".to_string());
    }

    let profile = build_profile(profile)?;
    migrate::edit_selected_sessions(
        &profile,
        &ids,
        &edit,
        &ApplyOptions {
            apply,
            backup: true,
            include_sessions_backup: false,
        },
    )
    .map_err(format_error)
}

#[tauri::command]
fn create_backup(profile: ProfileInput, include_sessions: bool) -> Result<BackupResponse, String> {
    let profile = build_profile(profile)?;
    let backup = backup::create_backup(&profile, include_sessions).map_err(format_error)?;
    Ok(BackupResponse {
        backup_dir: backup.backup_dir.display().to_string(),
        manifest_path: backup.manifest_path.map(|path| path.display().to_string()),
        copied_entries: backup.copied_files.len(),
    })
}

#[tauri::command]
fn restore_manifest(
    manifest_path: String,
    files: Vec<String>,
    apply: bool,
) -> Result<restore::RestoreReport, String> {
    restore::restore_from_manifest(PathBuf::from(manifest_path).as_path(), &files, apply)
        .map_err(format_error)
}

#[tauri::command]
fn app_server_probe(
    profile: ProfileInput,
    endpoint: String,
) -> Result<app_server::AppServerProbeReport, String> {
    let profile = build_profile(profile)?;
    let sessions = session_list::list_sessions(&profile, &SessionListFilter::default())
        .map_err(format_error)?;
    let transport = HttpAppServerTransport::new(endpoint);
    app_server::probe_app_server(&transport, &sessions).map_err(format_error)
}

fn build_profile(input: ProfileInput) -> Result<CodexProfile, String> {
    let path_maps = input
        .path_maps
        .iter()
        .map(|spec| PathMap::parse(spec).map_err(format_error))
        .collect::<Result<Vec<_>, _>>()?;
    CodexProfile::new(
        input.profile.unwrap_or_else(|| "desktop".to_string()),
        input.codex_home,
        input.provider,
        input.model,
        path_maps,
    )
    .map_err(format_error)
}

fn apply_options(apply: bool) -> SessionApplyOptions {
    SessionApplyOptions {
        apply,
        backup: true,
        include_sessions_backup: false,
    }
}

fn format_error(error: anyhow::Error) -> String {
    format!("{error:?}")
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_sessions,
            archive_sessions,
            restore_sessions,
            delete_sessions,
            edit_selected_sessions,
            create_backup,
            restore_manifest,
            app_server_probe
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Codex Session Manager");
}
