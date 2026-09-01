#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod wsl;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Command;

use codex_session_manager::backup_store::{
    BackupDeleteReport, BackupGroupDeleteReport, SessionBackupSummary,
};
use codex_session_manager::compact::CompactReport;
use codex_session_manager::db_repair::{
    DatabaseRepairApplyReport, DatabaseRepairOptions, DatabaseRepairPreview,
};
use codex_session_manager::favorites::FavoritesFile;
use codex_session_manager::instance_registry::{
    self, InstanceAvailability, InstanceRuntime, InstanceScanReport, InstanceSyncPlan,
    InstanceSyncPlanDraft, ManagedInstance, WslInstanceRegistration,
};
use codex_session_manager::instance_sync::{
    self, InstanceSyncConfigDiff, InstanceSyncConfigDiffRequest,
    InstanceSyncConfigDifferenceSummary, InstanceSyncConfigDifferenceSummaryRequest,
    InstanceSyncExecutionReport, InstanceSyncNonRootConfigDifferenceRequest,
    InstanceSyncNonRootConfigDifferenceSelection, InstanceSyncPreview, InstanceSyncRequest,
    InstanceSyncSourceData,
};
use codex_session_manager::migrate::{self, SessionEdit};
use codex_session_manager::profile_operation::{
    decode_operation_result, execute_profile_operation, BridgeRequest, ProfileOperation,
    ProfileSpec,
};
use codex_session_manager::restore::{RestorePreview, RestoreReport, RestoreSessionOptions};
use codex_session_manager::safety;
use codex_session_manager::session_list::{SessionListFilter, SessionSummary};
use codex_session_manager::session_ops::SessionMutationReport;
use codex_session_manager::settings::AppSettings;
use codex_session_manager::wsl::{is_wsl_mounted_path, is_wsl_unc_path};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tauri::Manager;
use wsl::{WslBridgeManager, WslDiscoveryError, WslRegistrationInput, WslStatus};

const PROJECT_GITHUB_URL: &str = "https://github.com/aisspire/codexSessionManager";
const APP_ICON: tauri::image::Image<'_> = tauri::include_image!("icons/128x128.png");

#[derive(Debug, Clone, Deserialize)]
struct ProfileInput {
    codex_home: String,
    #[serde(default)]
    managed_instance_id: Option<i64>,
    profile: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    path_maps: Vec<String>,
}

#[tauri::command]
async fn list_sessions(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    filter: SessionListFilter,
) -> Result<Vec<SessionSummary>, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::ListSessions { filter },
    )
    .await
}

#[tauri::command]
async fn load_settings(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
) -> Result<AppSettings, String> {
    run_profile_operation(&app, &bridge, profile, ProfileOperation::LoadSettings).await
}

#[tauri::command]
async fn save_settings(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::SaveSettings { settings },
    )
    .await
}

#[tauri::command]
async fn list_session_backups(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
) -> Result<Vec<SessionBackupSummary>, String> {
    run_profile_operation(&app, &bridge, profile, ProfileOperation::ListSessionBackups).await
}

#[tauri::command]
async fn preview_restore_session_backup(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    backup_id: String,
) -> Result<RestorePreview, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::PreviewRestoreSessionBackup { backup_id },
    )
    .await
}

#[tauri::command]
async fn restore_session_backup(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    backup_id: String,
    options: RestoreSessionOptions,
) -> Result<RestoreReport, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::RestoreSessionBackup { backup_id, options },
    )
    .await
}

#[tauri::command]
async fn delete_session_backup(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    backup_id: String,
    confirmed_last_archive: bool,
) -> Result<BackupDeleteReport, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::DeleteSessionBackup {
            backup_id,
            confirmed_last_archive,
        },
    )
    .await
}

#[tauri::command]
async fn delete_session_backup_groups(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    session_ids: Vec<String>,
    confirmed_last_archives: bool,
) -> Result<BackupGroupDeleteReport, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::DeleteSessionBackupGroups {
            session_ids,
            confirmed_last_archives,
        },
    )
    .await
}

#[tauri::command]
async fn toggle_favorite(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    session_id: String,
) -> Result<FavoritesFile, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::ToggleFavorite { session_id },
    )
    .await
}

#[tauri::command]
async fn set_favorite(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    session_id: String,
    favorite: bool,
) -> Result<FavoritesFile, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::SetFavorite {
            session_id,
            favorite,
        },
    )
    .await
}

#[tauri::command]
async fn archive_sessions(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    ids: Vec<String>,
    apply: bool,
) -> Result<SessionMutationReport, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::ArchiveSessions { ids, apply },
    )
    .await
}

#[tauri::command]
async fn active_sessions(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    ids: Vec<String>,
    apply: bool,
) -> Result<SessionMutationReport, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::ActiveSessions { ids, apply },
    )
    .await
}

#[tauri::command]
async fn delete_sessions(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    ids: Vec<String>,
    apply: bool,
) -> Result<SessionMutationReport, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::DeleteSessions { ids, apply },
    )
    .await
}

#[tauri::command]
async fn refresh_session_updated_at(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    ids: Vec<String>,
    apply: bool,
) -> Result<SessionMutationReport, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::RefreshSessionUpdatedAt { ids, apply },
    )
    .await
}

#[tauri::command]
async fn compact_session(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    id: String,
    apply: bool,
) -> Result<CompactReport, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::CompactSession { id, apply },
    )
    .await
}

#[tauri::command]
async fn compact_session_with_local_provider_fallback(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    id: String,
    apply: bool,
) -> Result<CompactReport, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::CompactSessionWithLocalProviderFallback { id, apply },
    )
    .await
}

#[tauri::command]
async fn edit_selected_sessions(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    ids: Vec<String>,
    edit: SessionEdit,
    apply: bool,
) -> Result<migrate::MutationReport, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::EditSelectedSessions { ids, edit, apply },
    )
    .await
}

#[tauri::command]
async fn preview_database_repairs(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
) -> Result<DatabaseRepairPreview, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::PreviewDatabaseRepairs,
    )
    .await
}

#[tauri::command]
async fn apply_database_repairs(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
    options: DatabaseRepairOptions,
) -> Result<DatabaseRepairApplyReport, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::ApplyDatabaseRepairs { options },
    )
    .await
}

#[tauri::command]
async fn detect_codex_running(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
) -> Result<bool, String> {
    detect_codex_running_with(
        || resolve_profile_target(&app, &profile),
        |target| {
            run_resolved_profile_operation::<bool>(
                &app,
                &bridge,
                target,
                ProfileOperation::DetectCodexRunning,
            )
        },
        || async {
            tauri::async_runtime::spawn_blocking(|| {
                safety::detect_codex_processes().map(|processes| !processes.is_empty())
            })
            .await
            .map_err(|error| format!("Codex process detection task failed: {error}"))?
            .map_err(|error| {
                format!("Windows Codex process detection failed for shared WSL home: {error:?}")
            })
        },
    )
    .await
}

#[tauri::command]
async fn apply_database_sync_from_local(
    app: tauri::AppHandle,
    bridge: tauri::State<'_, WslBridgeManager>,
    profile: ProfileInput,
) -> Result<DatabaseRepairApplyReport, String> {
    run_profile_operation(
        &app,
        &bridge,
        profile,
        ProfileOperation::ApplyDatabaseSyncFromLocal,
    )
    .await
}

#[tauri::command]
async fn list_managed_instances(
    app: tauri::AppHandle,
    refresh_wsl: Option<bool>,
) -> Result<Vec<ManagedInstance>, String> {
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    let mut instances = tauri::async_runtime::spawn_blocking(move || {
        instance_registry::list_managed_instances(&database_path).map_err(format_error)
    })
    .await
    .map_err(|error| format!("managed instance list task failed: {error}"))??;
    if refresh_wsl.unwrap_or(false) {
        for instance in &mut instances {
            let InstanceRuntime::Wsl {
                distribution,
                user,
                codex_home,
                ..
            } = &instance.runtime
            else {
                continue;
            };
            match wsl::probe(distribution, Some(user), Some(codex_home)).await {
                Ok(probe) => {
                    instance.availability = if probe.available {
                        InstanceAvailability::Available
                    } else {
                        InstanceAvailability::Unavailable
                    };
                    instance.availability_error = probe.error;
                }
                Err(error) => {
                    instance.availability = InstanceAvailability::Unavailable;
                    instance.availability_error = Some(format!("{error:?}"));
                }
            }
        }
    }
    Ok(instances)
}

#[tauri::command]
async fn scan_managed_instances(
    app: tauri::AppHandle,
    parent_path: String,
) -> Result<InstanceScanReport, String> {
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        instance_registry::scan_and_register(&database_path, Path::new(&parent_path))
            .map_err(format_error)
    })
    .await
    .map_err(|error| format!("managed instance scan task failed: {error}"))?
}

#[tauri::command]
async fn rename_managed_instance(
    app: tauri::AppHandle,
    instance_id: i64,
    display_name: String,
) -> Result<ManagedInstance, String> {
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        instance_registry::rename_managed_instance(&database_path, instance_id, &display_name)
            .map_err(format_error)
    })
    .await
    .map_err(|error| format!("managed instance rename task failed: {error}"))?
}

#[tauri::command]
async fn delete_managed_instance(app: tauri::AppHandle, instance_id: i64) -> Result<(), String> {
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        delete_managed_instance_from_registry(&database_path, instance_id)
    })
    .await
    .map_err(|error| format!("managed instance delete task failed: {error}"))?
}

#[tauri::command]
async fn ignore_managed_instance(app: tauri::AppHandle, instance_id: i64) -> Result<(), String> {
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        ignore_managed_instance_from_registry(&database_path, instance_id)
    })
    .await
    .map_err(|error| format!("managed instance ignore task failed: {error}"))?
}

#[tauri::command]
async fn open_managed_instance_path(app: tauri::AppHandle, instance_id: i64) -> Result<(), String> {
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    let path = tauri::async_runtime::spawn_blocking(move || {
        instance_registry::managed_instance_path(&database_path, instance_id)
    })
    .await
    .map_err(|error| format!("managed instance path task failed: {error}"))?
    .map_err(format_error)?;
    open_path_in_default_file_manager(&path)
}

#[derive(Debug, Clone, Serialize)]
struct RegisteredWslDiscoveryReport {
    instances: Vec<ManagedInstance>,
    errors: Vec<WslDiscoveryError>,
}

#[tauri::command]
async fn get_wsl_status() -> Result<WslStatus, String> {
    wsl::get_status().await.map_err(format_error)
}

#[tauri::command]
async fn discover_wsl_instances(
    app: tauri::AppHandle,
) -> Result<RegisteredWslDiscoveryReport, String> {
    let discovery = wsl::discover().await.map_err(format_error)?;
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    let mut instances = Vec::new();
    let mut errors = discovery.errors;
    for probe in discovery.instances {
        let registration = registration_from_probe(&probe);
        let database_path = database_path.clone();
        match tauri::async_runtime::spawn_blocking(move || {
            instance_registry::register_wsl_instance(&database_path, &registration)
        })
        .await
        {
            Ok(Ok(instance)) => instances.push(instance),
            Ok(Err(error)) => errors.push(WslDiscoveryError {
                distribution: probe.distribution,
                error: format!("{error:?}"),
            }),
            Err(error) => errors.push(WslDiscoveryError {
                distribution: probe.distribution,
                error: format!("WSL registration task failed: {error}"),
            }),
        }
    }
    Ok(RegisteredWslDiscoveryReport { instances, errors })
}

#[tauri::command]
async fn register_wsl_instance(
    app: tauri::AppHandle,
    input: WslRegistrationInput,
) -> Result<ManagedInstance, String> {
    let probe = wsl::probe(
        &input.distribution,
        input.user.as_deref(),
        Some(&input.codex_home),
    )
    .await
    .map_err(format_error)?;
    if !probe.available {
        return Err(probe
            .error
            .unwrap_or_else(|| "WSL Codex instance is not available".to_string()));
    }
    let registration = registration_from_probe(&probe);
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    let mut instance = tauri::async_runtime::spawn_blocking(move || {
        instance_registry::register_wsl_instance(&database_path, &registration)
            .map_err(format_error)
    })
    .await
    .map_err(|error| format!("WSL registration task failed: {error}"))??;
    instance.availability = InstanceAvailability::Available;
    Ok(instance)
}

#[tauri::command]
async fn translate_path_for_profile(
    app: tauri::AppHandle,
    profile: ProfileInput,
    path: String,
) -> Result<String, String> {
    match resolve_profile_target(&app, &profile).await? {
        ResolvedProfileTarget::Native { .. } => Ok(path),
        ResolvedProfileTarget::Wsl {
            distribution, user, ..
        } => wsl::translate_windows_path(&distribution, &user, &path)
            .await
            .map_err(format_error),
    }
}

#[tauri::command]
async fn list_instance_sync_plans(app: tauri::AppHandle) -> Result<Vec<InstanceSyncPlan>, String> {
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        instance_registry::list_instance_sync_plans(&database_path).map_err(format_error)
    })
    .await
    .map_err(|error| format!("instance sync plan list task failed: {error}"))?
}

#[tauri::command]
async fn save_instance_sync_plan(
    app: tauri::AppHandle,
    draft: InstanceSyncPlanDraft,
) -> Result<InstanceSyncPlan, String> {
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        instance_registry::save_instance_sync_plan(&database_path, &draft).map_err(format_error)
    })
    .await
    .map_err(|error| format!("instance sync plan save task failed: {error}"))?
}

#[tauri::command]
async fn delete_instance_sync_plan(app: tauri::AppHandle, plan_id: i64) -> Result<(), String> {
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        instance_registry::delete_instance_sync_plan(&database_path, plan_id).map_err(format_error)
    })
    .await
    .map_err(|error| format!("instance sync plan delete task failed: {error}"))?
}

#[tauri::command]
async fn list_instance_sync_source_data(
    app: tauri::AppHandle,
    source_instance_id: i64,
) -> Result<InstanceSyncSourceData, String> {
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        instance_sync::list_instance_sync_source_data(&database_path, source_instance_id)
            .map_err(format_error)
    })
    .await
    .map_err(|error| format!("instance sync source data task failed: {error}"))?
}

#[tauri::command]
async fn preview_instance_sync_config_diff(
    app: tauri::AppHandle,
    request: InstanceSyncConfigDiffRequest,
) -> Result<InstanceSyncConfigDiff, String> {
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        instance_sync::preview_instance_sync_config_diff(&database_path, &request)
            .map_err(format_error)
    })
    .await
    .map_err(|error| format!("instance sync config diff task failed: {error}"))?
}

#[tauri::command]
async fn summarize_instance_sync_config_differences(
    app: tauri::AppHandle,
    request: InstanceSyncConfigDifferenceSummaryRequest,
) -> Result<InstanceSyncConfigDifferenceSummary, String> {
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        instance_sync::summarize_instance_sync_config_differences(&database_path, &request)
            .map_err(format_error)
    })
    .await
    .map_err(|error| format!("instance sync config difference summary task failed: {error}"))?
}

#[tauri::command]
async fn select_instance_sync_non_root_config_differences(
    app: tauri::AppHandle,
    request: InstanceSyncNonRootConfigDifferenceRequest,
) -> Result<InstanceSyncNonRootConfigDifferenceSelection, String> {
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        instance_sync::select_instance_sync_non_root_config_differences(&database_path, &request)
            .map_err(format_error)
    })
    .await
    .map_err(|error| format!("instance sync non-root config selection task failed: {error}"))?
}

#[tauri::command]
async fn preview_instance_sync(
    app: tauri::AppHandle,
    request: InstanceSyncRequest,
) -> Result<InstanceSyncPreview, String> {
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        instance_sync::preview_instance_sync(&database_path, &request).map_err(format_error)
    })
    .await
    .map_err(|error| format!("instance sync preview task failed: {error}"))?
}

#[tauri::command]
async fn execute_instance_sync(
    app: tauri::AppHandle,
    request: InstanceSyncRequest,
) -> Result<InstanceSyncExecutionReport, String> {
    let database_path = managed_instance_registry_database(&app).map_err(format_error)?;
    tauri::async_runtime::spawn_blocking(move || {
        instance_sync::execute_instance_sync(&database_path, &request).map_err(format_error)
    })
    .await
    .map_err(|error| format!("instance sync execution task failed: {error}"))?
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    if !is_allowed_external_url(&url) {
        return Err("external URL is not allowed".to_string());
    }
    open_url_in_default_browser(&url)
}

enum ResolvedProfileTarget {
    Native {
        profile: ProfileSpec,
    },
    Wsl {
        profile: ProfileSpec,
        distribution: String,
        user: String,
        architecture: String,
    },
}

async fn detect_codex_running_with<
    Resolve,
    ResolveFuture,
    Execute,
    ExecuteFuture,
    Windows,
    WindowsFuture,
>(
    resolve: Resolve,
    execute: Execute,
    detect_windows: Windows,
) -> Result<bool, String>
where
    Resolve: FnOnce() -> ResolveFuture,
    ResolveFuture: Future<Output = Result<ResolvedProfileTarget, String>>,
    Execute: FnOnce(ResolvedProfileTarget) -> ExecuteFuture,
    ExecuteFuture: Future<Output = Result<bool, String>>,
    Windows: FnOnce() -> WindowsFuture,
    WindowsFuture: Future<Output = Result<bool, String>>,
{
    let target = resolve().await?;
    let shared_wsl_home = matches!(
        &target,
        ResolvedProfileTarget::Wsl { profile, .. }
            if is_wsl_mounted_path(&profile.codex_home)
    );
    let running = execute(target).await?;
    if !shared_wsl_home {
        return Ok(running);
    }
    Ok(running || detect_windows().await?)
}

async fn run_profile_operation<T: DeserializeOwned>(
    app: &tauri::AppHandle,
    bridge: &WslBridgeManager,
    input: ProfileInput,
    operation: ProfileOperation,
) -> Result<T, String> {
    let target = resolve_profile_target(app, &input).await?;
    run_resolved_profile_operation(app, bridge, target, operation).await
}

async fn run_resolved_profile_operation<T: DeserializeOwned>(
    app: &tauri::AppHandle,
    bridge: &WslBridgeManager,
    target: ResolvedProfileTarget,
    operation: ProfileOperation,
) -> Result<T, String> {
    let requires_codex_stopped = operation.requires_codex_stopped();
    let value = match target {
        ResolvedProfileTarget::Native { profile } => {
            if requires_codex_stopped {
                ensure_codex_stopped_for_native_profile().await?;
            }
            let request = BridgeRequest::new(profile, operation);
            tauri::async_runtime::spawn_blocking(move || execute_profile_operation(&request))
                .await
                .map_err(|error| format!("native profile operation task failed: {error}"))?
                .map_err(format_error)?
        }
        ResolvedProfileTarget::Wsl {
            profile,
            distribution,
            user,
            architecture,
        } => {
            if requires_codex_stopped && is_wsl_mounted_path(&profile.codex_home) {
                ensure_windows_codex_stopped_for_shared_wsl_home().await?;
            }
            let request = BridgeRequest::new(profile, operation);
            bridge
                .invoke(app, &distribution, &user, &architecture, &request)
                .await
                .map_err(format_error)?
        }
    };
    decode_operation_result(value).map_err(format_error)
}

async fn ensure_codex_stopped_for_native_profile() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(safety::ensure_codex_not_running)
        .await
        .map_err(|error| format!("Codex process detection task failed: {error}"))?
        .map_err(format_error)
}

async fn ensure_windows_codex_stopped_for_shared_wsl_home() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let processes = tauri::async_runtime::spawn_blocking(safety::detect_codex_processes)
            .await
            .map_err(|error| format!("Codex process detection task failed: {error}"))?
            .map_err(|error| {
                format!(
                    "cannot safely write the shared WSL Codex home because Windows process detection failed: {error:?}"
                )
            })?;
        if !processes.is_empty() {
            return Err(
                "Codex appears to be running on Windows; close it before writing the shared WSL Codex home"
                    .to_string(),
            );
        }
    }
    Ok(())
}

async fn resolve_profile_target(
    app: &tauri::AppHandle,
    input: &ProfileInput,
) -> Result<ResolvedProfileTarget, String> {
    let Some(instance_id) = input.managed_instance_id else {
        #[cfg(target_os = "windows")]
        if instance_registry::legacy_wsl_path(&input.codex_home).is_some()
            || is_wsl_unc_path(&input.codex_home)
            || is_wsl_mounted_path(&input.codex_home)
        {
            return Err(
                "WSL/UNC Codex 主目录必须先通过“发现 WSL”或手动 WSL 表单登记，不能直接跨文件系统访问 SQLite"
                    .to_string(),
            );
        }
        return Ok(ResolvedProfileTarget::Native {
            profile: profile_spec(input, input.codex_home.clone()),
        });
    };

    let database_path = managed_instance_registry_database(app).map_err(format_error)?;
    let instance = tauri::async_runtime::spawn_blocking(move || {
        instance_registry::managed_instance(&database_path, instance_id)
    })
    .await
    .map_err(|error| format!("managed instance lookup task failed: {error}"))?
    .map_err(format_error)?;
    match instance.runtime {
        InstanceRuntime::Native => {
            if instance.availability == InstanceAvailability::Unavailable {
                return Err(instance.availability_error.unwrap_or_else(|| {
                    format!("managed instance {instance_id} is not available")
                }));
            }
            if instance_registry::legacy_wsl_path(&instance.path).is_some()
                || is_wsl_unc_path(&instance.path)
                || is_wsl_mounted_path(&instance.path)
            {
                return Err("该记录尚未绑定 WSL 运行时，请重新发现或手动登记后再访问".to_string());
            }
            Ok(ResolvedProfileTarget::Native {
                profile: profile_spec(input, instance.path),
            })
        }
        InstanceRuntime::Wsl {
            distribution,
            user,
            codex_home,
            architecture,
            ..
        } => {
            let probe = wsl::probe(&distribution, Some(&user), Some(&codex_home))
                .await
                .map_err(|error| {
                    format!(
                        "WSL profile target is unavailable for {distribution}/{user}: {error:?}"
                    )
                })?;
            if !probe.available {
                return Err(probe.error.unwrap_or_else(|| {
                    format!("WSL profile target is unavailable for {distribution}/{user}")
                }));
            }
            if probe.user != user {
                return Err(format!(
                    "WSL probe resolved user {}, but the managed instance requires {}",
                    probe.user, user
                ));
            }
            if probe.codex_home != codex_home {
                return Err(format!(
                    "WSL probe resolved Codex home {}, but the managed instance requires {}",
                    probe.codex_home, codex_home
                ));
            }
            let registered_architecture =
                codex_session_manager::wsl::normalize_architecture(&architecture)
                    .map_err(format_error)?;
            if probe.architecture != registered_architecture {
                return Err(format!(
                    "WSL architecture changed for {distribution}/{user}: registered {}, detected {}",
                    registered_architecture, probe.architecture
                ));
            }
            Ok(ResolvedProfileTarget::Wsl {
                profile: profile_spec(input, probe.codex_home),
                distribution,
                user,
                architecture: probe.architecture,
            })
        }
    }
}

fn profile_spec(input: &ProfileInput, codex_home: String) -> ProfileSpec {
    ProfileSpec {
        name: input
            .profile
            .clone()
            .unwrap_or_else(|| "desktop".to_string()),
        codex_home,
        provider: input.provider.clone(),
        model: input.model.clone(),
        path_maps: input.path_maps.clone(),
    }
}

fn registration_from_probe(probe: &wsl::WslProbe) -> WslInstanceRegistration {
    WslInstanceRegistration {
        distribution: probe.distribution.clone(),
        user: probe.user.clone(),
        codex_home: probe.codex_home.clone(),
        host_path: probe.host_path.clone(),
        architecture: probe.architecture.clone(),
    }
}

fn format_error(error: anyhow::Error) -> String {
    format!("{error:?}")
}

fn is_allowed_external_url(url: &str) -> bool {
    url == PROJECT_GITHUB_URL
}

fn managed_instance_registry_database(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    let app_data_directory = app
        .path()
        .app_data_dir()
        .map_err(|error| anyhow::anyhow!("failed to resolve app data directory: {error}"))?;
    Ok(managed_instance_registry_path(&app_data_directory))
}

fn managed_instance_registry_path(app_data_directory: &Path) -> PathBuf {
    app_data_directory.join("managed-instances.sqlite")
}

fn delete_managed_instance_from_registry(
    database_path: &Path,
    instance_id: i64,
) -> Result<(), String> {
    instance_registry::soft_delete_managed_instance(database_path, instance_id)
        .map_err(format_error)
}

fn ignore_managed_instance_from_registry(
    database_path: &Path,
    instance_id: i64,
) -> Result<(), String> {
    instance_registry::permanently_ignore_managed_instance(database_path, instance_id)
        .map_err(format_error)
}

fn open_url_in_default_browser(url: &str) -> Result<(), String> {
    let mut command = default_browser_command(url);
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to open default browser: {error}"))
}

fn open_path_in_default_file_manager(path: &Path) -> Result<(), String> {
    let mut command = default_file_manager_command(path);
    command.spawn().map(|_| ()).map_err(|error| {
        format!(
            "failed to open instance directory {}: {error}",
            path.display()
        )
    })
}

fn default_browser_command(url: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        hide_child_console(&mut command);
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

fn default_file_manager_command(path: &Path) -> Command {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("explorer");
        command.arg(path);
        hide_child_console(&mut command);
        command
    }

    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg(path);
        command
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    }
}

#[cfg(target_os = "windows")]
fn hide_child_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

fn main() {
    tauri::Builder::default()
        .manage(WslBridgeManager::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                window.set_icon(APP_ICON.clone())?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_sessions,
            load_settings,
            save_settings,
            list_session_backups,
            preview_restore_session_backup,
            restore_session_backup,
            delete_session_backup,
            delete_session_backup_groups,
            toggle_favorite,
            set_favorite,
            archive_sessions,
            active_sessions,
            delete_sessions,
            refresh_session_updated_at,
            compact_session,
            compact_session_with_local_provider_fallback,
            edit_selected_sessions,
            preview_database_repairs,
            apply_database_repairs,
            detect_codex_running,
            apply_database_sync_from_local,
            list_managed_instances,
            scan_managed_instances,
            rename_managed_instance,
            delete_managed_instance,
            ignore_managed_instance,
            open_managed_instance_path,
            get_wsl_status,
            discover_wsl_instances,
            register_wsl_instance,
            translate_path_for_profile,
            list_instance_sync_plans,
            save_instance_sync_plan,
            delete_instance_sync_plan,
            list_instance_sync_source_data,
            preview_instance_sync_config_diff,
            summarize_instance_sync_config_differences,
            select_instance_sync_non_root_config_differences,
            preview_instance_sync,
            execute_instance_sync,
            open_external_url
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Codex Session Manager");
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn allows_only_the_project_github_repository() {
        assert!(is_allowed_external_url(
            "https://github.com/aisspire/codexSessionManager"
        ));
        assert!(!is_allowed_external_url(
            "https://github.com/aisspire/other"
        ));
        assert!(!is_allowed_external_url(
            "https://example.com/aisspire/codexSessionManager"
        ));
    }

    #[test]
    fn keeps_managed_instance_registry_in_app_data_directory() {
        assert_eq!(
            managed_instance_registry_path(std::path::Path::new("app-data")),
            std::path::PathBuf::from("app-data").join("managed-instances.sqlite")
        );
    }

    #[test]
    fn detects_shared_wsl_home_with_one_resolution_and_windows_process_check() {
        let resolve_calls = Cell::new(0);
        let windows_check_calls = Cell::new(0);
        let result = tauri::async_runtime::block_on(detect_codex_running_with(
            || {
                resolve_calls.set(resolve_calls.get() + 1);
                async {
                    Ok::<_, String>(ResolvedProfileTarget::Wsl {
                        profile: ProfileSpec {
                            name: "test".to_string(),
                            codex_home: "/mnt/c/Users/dev/.codex".to_string(),
                            provider: None,
                            model: None,
                            path_maps: Vec::new(),
                        },
                        distribution: "Ubuntu".to_string(),
                        user: "dev".to_string(),
                        architecture: "x86_64".to_string(),
                    })
                }
            },
            |target| async move {
                assert!(matches!(target, ResolvedProfileTarget::Wsl { .. }));
                Ok::<_, String>(false)
            },
            || {
                windows_check_calls.set(windows_check_calls.get() + 1);
                async { Ok::<_, String>(true) }
            },
        ));

        assert_eq!(result.unwrap(), true);
        assert_eq!(resolve_calls.get(), 1);
        assert_eq!(windows_check_calls.get(), 1);
    }

    #[test]
    fn delete_managed_instance_bridge_only_deletes_the_registry_record() {
        let unique_suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_directory = std::env::temp_dir().join(format!(
            "codex-session-manager-delete-managed-instance-{}-{unique_suffix}",
            std::process::id()
        ));
        let instance_directory = test_directory.join("instance");
        let database_path = test_directory.join("managed-instances.sqlite");
        std::fs::create_dir_all(&instance_directory).unwrap();
        std::fs::write(
            instance_directory.join("config.toml"),
            "model = \"gpt-5\"\n",
        )
        .unwrap();

        instance_registry::scan_and_register(&database_path, &test_directory).unwrap();
        let instance = instance_registry::list_managed_instances(&database_path)
            .unwrap()
            .pop()
            .unwrap();

        delete_managed_instance_from_registry(&database_path, instance.id).unwrap();

        assert!(instance_registry::list_managed_instances(&database_path)
            .unwrap()
            .is_empty());
        assert!(instance_directory.is_dir());
        assert!(instance_directory.join("config.toml").is_file());
        std::fs::remove_dir_all(test_directory).unwrap();
    }

    #[test]
    fn ignore_managed_instance_bridge_keeps_the_instance_ignored_after_rescan() {
        let unique_suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_directory = std::env::temp_dir().join(format!(
            "codex-session-manager-ignore-managed-instance-{}-{unique_suffix}",
            std::process::id()
        ));
        let instance_directory = test_directory.join("instance");
        let database_path = test_directory.join("managed-instances.sqlite");
        std::fs::create_dir_all(&instance_directory).unwrap();
        std::fs::write(
            instance_directory.join("config.toml"),
            "model = \"gpt-5\"\n",
        )
        .unwrap();

        instance_registry::scan_and_register(&database_path, &test_directory).unwrap();
        let instance = instance_registry::list_managed_instances(&database_path)
            .unwrap()
            .pop()
            .unwrap();

        ignore_managed_instance_from_registry(&database_path, instance.id).unwrap();
        let rescan = instance_registry::scan_and_register(&database_path, &test_directory).unwrap();

        assert!(instance_registry::list_managed_instances(&database_path)
            .unwrap()
            .is_empty());
        assert_eq!(rescan.ignored, 1);
        assert!(instance_directory.is_dir());
        assert!(instance_directory.join("config.toml").is_file());
        std::fs::remove_dir_all(test_directory).unwrap();
    }
}
