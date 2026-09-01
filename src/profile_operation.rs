use anyhow::{bail, Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

use crate::backup_store;
use crate::compact::{self, CompactOptions};
use crate::db_repair::{self, DatabaseRepairOptions};
use crate::favorites;
use crate::migrate::{self, ApplyOptions, SessionEdit};
use crate::path_map::PathMap;
use crate::profile::CodexProfile;
use crate::restore::{self, RestoreSessionOptions};
use crate::safety;
use crate::session_list::{self, SessionListFilter};
use crate::session_ops::{self, SessionApplyOptions};
use crate::settings::{self, AppSettings};

pub const WSL_BRIDGE_PROTOCOL_VERSION: u32 = 1;
pub const WSL_BRIDGE_APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const WSL_BRIDGE_RESPONSE_MARKER: &str = "__CSM_WSL_BRIDGE_RESPONSE__";
pub const WSL_BRIDGE_NORMAL_OPERATION_TIMEOUT: Duration = Duration::from_secs(150);
pub const WSL_BRIDGE_LONG_OPERATION_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeIdentity {
    pub protocol_version: u32,
    pub app_version: String,
    pub target_architecture: String,
}

impl BridgeIdentity {
    pub fn for_architecture(architecture: &str) -> Result<Self> {
        Ok(Self {
            protocol_version: WSL_BRIDGE_PROTOCOL_VERSION,
            app_version: WSL_BRIDGE_APP_VERSION.to_string(),
            target_architecture: crate::wsl::normalize_architecture(architecture)?,
        })
    }

    pub fn current() -> Result<Self> {
        Self::for_architecture(std::env::consts::ARCH)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSpec {
    pub name: String,
    pub codex_home: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub path_maps: Vec<String>,
}

impl ProfileSpec {
    pub fn build(&self) -> Result<CodexProfile> {
        let path_maps = self
            .path_maps
            .iter()
            .map(|spec| PathMap::parse(spec))
            .collect::<Result<Vec<_>>>()?;
        CodexProfile::new(
            self.name.clone(),
            self.codex_home.clone(),
            self.provider.clone(),
            self.model.clone(),
            path_maps,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ProfileOperation {
    ListSessions {
        filter: SessionListFilter,
    },
    LoadSettings,
    SaveSettings {
        settings: AppSettings,
    },
    ListSessionBackups,
    PreviewRestoreSessionBackup {
        backup_id: String,
    },
    RestoreSessionBackup {
        backup_id: String,
        options: RestoreSessionOptions,
    },
    DeleteSessionBackup {
        backup_id: String,
        confirmed_last_archive: bool,
    },
    DeleteSessionBackupGroups {
        session_ids: Vec<String>,
        confirmed_last_archives: bool,
    },
    ToggleFavorite {
        session_id: String,
    },
    SetFavorite {
        session_id: String,
        favorite: bool,
    },
    ArchiveSessions {
        ids: Vec<String>,
        apply: bool,
    },
    ActiveSessions {
        ids: Vec<String>,
        apply: bool,
    },
    DeleteSessions {
        ids: Vec<String>,
        apply: bool,
    },
    RefreshSessionUpdatedAt {
        ids: Vec<String>,
        apply: bool,
    },
    CompactSession {
        id: String,
        apply: bool,
    },
    CompactSessionWithLocalProviderFallback {
        id: String,
        apply: bool,
    },
    EditSelectedSessions {
        ids: Vec<String>,
        edit: SessionEdit,
        apply: bool,
    },
    PreviewDatabaseRepairs,
    ApplyDatabaseRepairs {
        options: DatabaseRepairOptions,
    },
    DetectCodexRunning,
    ApplyDatabaseSyncFromLocal,
}

impl ProfileOperation {
    pub fn requires_codex_stopped(&self) -> bool {
        match self {
            Self::ListSessions { .. }
            | Self::LoadSettings
            | Self::ListSessionBackups
            | Self::PreviewRestoreSessionBackup { .. }
            | Self::PreviewDatabaseRepairs
            | Self::DetectCodexRunning => false,
            Self::SaveSettings { .. }
            | Self::DeleteSessionBackup { .. }
            | Self::DeleteSessionBackupGroups { .. }
            | Self::ToggleFavorite { .. }
            | Self::SetFavorite { .. } => true,
            Self::ArchiveSessions { apply, .. }
            | Self::ActiveSessions { apply, .. }
            | Self::DeleteSessions { apply, .. }
            | Self::RefreshSessionUpdatedAt { apply, .. }
            | Self::CompactSession { apply, .. }
            | Self::CompactSessionWithLocalProviderFallback { apply, .. }
            | Self::EditSelectedSessions { apply, .. } => *apply,
            Self::RestoreSessionBackup { options, .. } => options.apply,
            Self::ApplyDatabaseRepairs { .. } | Self::ApplyDatabaseSyncFromLocal => true,
        }
    }

    /// Returns the deadline used by the WSL bridge for this operation.
    ///
    /// Long-running operations are deliberately identified here, next to the
    /// exhaustive operation enum, so a newly added write operation cannot
    /// silently inherit an inappropriate timeout in the desktop bridge.
    pub fn wsl_timeout(&self) -> Duration {
        match self {
            Self::RestoreSessionBackup { options, .. } if options.apply => {
                WSL_BRIDGE_LONG_OPERATION_TIMEOUT
            }
            Self::CompactSession { .. }
            | Self::CompactSessionWithLocalProviderFallback { .. }
            | Self::ApplyDatabaseRepairs { .. }
            | Self::ApplyDatabaseSyncFromLocal
            | Self::DeleteSessionBackupGroups { .. } => WSL_BRIDGE_LONG_OPERATION_TIMEOUT,
            Self::ArchiveSessions { apply, .. }
            | Self::ActiveSessions { apply, .. }
            | Self::DeleteSessions { apply, .. }
            | Self::RefreshSessionUpdatedAt { apply, .. }
            | Self::EditSelectedSessions { apply, .. }
                if *apply =>
            {
                WSL_BRIDGE_LONG_OPERATION_TIMEOUT
            }
            _ => WSL_BRIDGE_NORMAL_OPERATION_TIMEOUT,
        }
    }

    /// A missing response marker is ambiguous: the helper may have completed
    /// a write and only lost its output. Only operations that are read-only,
    /// previews, or explicitly have `apply: false` may be safely retried.
    pub fn can_retry_after_missing_response_marker(&self) -> bool {
        match self {
            Self::ListSessions { .. }
            | Self::LoadSettings
            | Self::ListSessionBackups
            | Self::PreviewRestoreSessionBackup { .. }
            | Self::PreviewDatabaseRepairs
            | Self::DetectCodexRunning => true,
            Self::RestoreSessionBackup { options, .. } => !options.apply,
            Self::ArchiveSessions { apply, .. }
            | Self::ActiveSessions { apply, .. }
            | Self::DeleteSessions { apply, .. }
            | Self::RefreshSessionUpdatedAt { apply, .. }
            | Self::CompactSession { apply, .. }
            | Self::CompactSessionWithLocalProviderFallback { apply, .. }
            | Self::EditSelectedSessions { apply, .. } => !*apply,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeRequest {
    pub protocol_version: u32,
    pub profile: ProfileSpec,
    pub operation: ProfileOperation,
}

impl BridgeRequest {
    pub fn new(profile: ProfileSpec, operation: ProfileOperation) -> Self {
        Self {
            protocol_version: WSL_BRIDGE_PROTOCOL_VERSION,
            profile,
            operation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BridgeResponse {
    pub protocol_version: u32,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl BridgeResponse {
    pub fn success(result: Value) -> Self {
        Self {
            protocol_version: WSL_BRIDGE_PROTOCOL_VERSION,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            protocol_version: WSL_BRIDGE_PROTOCOL_VERSION,
            ok: false,
            result: None,
            error: Some(error.into()),
        }
    }
}

pub fn decode_operation_result<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).context("failed to decode profile operation response")
}

pub fn execute_profile_operation(request: &BridgeRequest) -> Result<Value> {
    if request.protocol_version != WSL_BRIDGE_PROTOCOL_VERSION {
        bail!(
            "WSL bridge protocol mismatch: requested {}, helper supports {}",
            request.protocol_version,
            WSL_BRIDGE_PROTOCOL_VERSION
        );
    }
    let profile = request.profile.build()?;
    let value = match &request.operation {
        ProfileOperation::ListSessions { filter } => {
            serde_json::to_value(session_list::list_sessions(&profile, filter)?)?
        }
        ProfileOperation::LoadSettings => serde_json::to_value(settings::load_settings(&profile)?)?,
        ProfileOperation::SaveSettings { settings: value } => {
            settings::save_settings(&profile, value)?;
            serde_json::to_value(value)?
        }
        ProfileOperation::ListSessionBackups => {
            serde_json::to_value(backup_store::list_session_backups(&profile)?)?
        }
        ProfileOperation::PreviewRestoreSessionBackup { backup_id } => serde_json::to_value(
            restore::preview_restore_session_backup(&profile, backup_id)?,
        )?,
        ProfileOperation::RestoreSessionBackup { backup_id, options } => serde_json::to_value(
            restore::restore_session_backup(&profile, backup_id, options)?,
        )?,
        ProfileOperation::DeleteSessionBackup {
            backup_id,
            confirmed_last_archive,
        } => serde_json::to_value(backup_store::delete_backup_snapshot_with_confirmation(
            &profile,
            backup_id,
            *confirmed_last_archive,
        )?)?,
        ProfileOperation::DeleteSessionBackupGroups {
            session_ids,
            confirmed_last_archives,
        } => serde_json::to_value(backup_store::delete_backup_groups(
            &profile,
            session_ids,
            *confirmed_last_archives,
        )?)?,
        ProfileOperation::ToggleFavorite { session_id } => {
            serde_json::to_value(favorites::toggle_favorite(&profile, session_id)?)?
        }
        ProfileOperation::SetFavorite {
            session_id,
            favorite,
        } => serde_json::to_value(favorites::set_favorite(&profile, session_id, *favorite)?)?,
        ProfileOperation::ArchiveSessions { ids, apply } => serde_json::to_value(
            session_ops::archive_sessions(&profile, ids, &SessionApplyOptions { apply: *apply })?,
        )?,
        ProfileOperation::ActiveSessions { ids, apply } => serde_json::to_value(
            session_ops::active_sessions(&profile, ids, &SessionApplyOptions { apply: *apply })?,
        )?,
        ProfileOperation::DeleteSessions { ids, apply } => serde_json::to_value(
            session_ops::delete_sessions(&profile, ids, &SessionApplyOptions { apply: *apply })?,
        )?,
        ProfileOperation::RefreshSessionUpdatedAt { ids, apply } => {
            serde_json::to_value(session_ops::refresh_session_updated_at(
                &profile,
                ids,
                &SessionApplyOptions { apply: *apply },
            )?)?
        }
        ProfileOperation::CompactSession { id, apply } => serde_json::to_value(
            compact::compact_session(&profile, id, &CompactOptions { apply: *apply })?,
        )?,
        ProfileOperation::CompactSessionWithLocalProviderFallback { id, apply } => {
            serde_json::to_value(compact::compact_session_with_local_provider_fallback(
                &profile,
                id,
                &CompactOptions { apply: *apply },
            )?)?
        }
        ProfileOperation::EditSelectedSessions { ids, edit, apply } => {
            validate_session_edit(ids, edit)?;
            serde_json::to_value(migrate::edit_selected_sessions(
                &profile,
                ids,
                edit,
                &ApplyOptions { apply: *apply },
            )?)?
        }
        ProfileOperation::PreviewDatabaseRepairs => {
            serde_json::to_value(db_repair::preview_database_repairs(&profile)?)?
        }
        ProfileOperation::ApplyDatabaseRepairs { options } => {
            serde_json::to_value(db_repair::apply_database_repairs(&profile, options)?)?
        }
        ProfileOperation::DetectCodexRunning => {
            serde_json::to_value(!safety::detect_codex_processes()?.is_empty())?
        }
        ProfileOperation::ApplyDatabaseSyncFromLocal => {
            serde_json::to_value(db_repair::apply_database_sync_from_local(&profile)?)?
        }
    };
    Ok(value)
}

fn validate_session_edit(ids: &[String], edit: &SessionEdit) -> Result<()> {
    if ids.is_empty() {
        bail!("please select at least one session");
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
        bail!("please enter a provider, project, title, or title prefix to edit");
    }
    Ok(())
}
