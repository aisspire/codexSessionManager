use std::process::Command;

use codex_session_manager::migrate::{self, ApplyOptions, SessionEdit};
use codex_session_manager::path_map::PathMap;
use codex_session_manager::profile::CodexProfile;
use codex_session_manager::session_list::{self, SessionListFilter, SessionSummary};
use codex_session_manager::session_ops::{self, SessionApplyOptions, SessionMutationReport};
use serde::Deserialize;

const PROJECT_GITHUB_URL: &str = "https://github.com/aisspire/codexSessionManager";

#[derive(Debug, Clone, Deserialize)]
struct ProfileInput {
    codex_home: String,
    profile: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    path_maps: Vec<String>,
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
fn active_sessions(
    profile: ProfileInput,
    ids: Vec<String>,
    apply: bool,
) -> Result<SessionMutationReport, String> {
    let profile = build_profile(profile)?;
    session_ops::active_sessions(&profile, &ids, &apply_options(apply)).map_err(format_error)
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
fn refresh_session_updated_at(
    profile: ProfileInput,
    ids: Vec<String>,
    apply: bool,
) -> Result<SessionMutationReport, String> {
    let profile = build_profile(profile)?;
    session_ops::refresh_session_updated_at(&profile, &ids, &apply_options(apply))
        .map_err(format_error)
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
        &ApplyOptions { apply },
    )
    .map_err(format_error)
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    if !is_allowed_external_url(&url) {
        return Err("external URL is not allowed".to_string());
    }
    open_url_in_default_browser(&url)
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
    SessionApplyOptions { apply }
}

fn format_error(error: anyhow::Error) -> String {
    format!("{error:?}")
}

fn is_allowed_external_url(url: &str) -> bool {
    url == PROJECT_GITHUB_URL
}

fn open_url_in_default_browser(url: &str) -> Result<(), String> {
    let mut command = default_browser_command(url);
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to open default browser: {error}"))
}

fn default_browser_command(url: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    }

    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg(url);
        command
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_sessions,
            archive_sessions,
            active_sessions,
            delete_sessions,
            refresh_session_updated_at,
            edit_selected_sessions,
            open_external_url
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Codex Session Manager");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_only_the_project_github_repository() {
        assert!(is_allowed_external_url(
            "https://github.com/aisspire/codexSessionManager"
        ));
        assert!(!is_allowed_external_url("https://github.com/aisspire/other"));
        assert!(!is_allowed_external_url("https://example.com/aisspire/codexSessionManager"));
    }
}
