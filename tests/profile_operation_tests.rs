use codex_session_manager::db_repair::DatabaseRepairOptions;
use codex_session_manager::instance_sync::InstanceSyncRequest;
use codex_session_manager::migrate::SessionEdit;
use codex_session_manager::profile_operation::{
    BridgeRequest, BridgeResponse, InstanceSyncBridgeAction, InstanceSyncBridgeTarget,
    ProfileOperation, ProfileSpec, WSL_BRIDGE_PROTOCOL_VERSION,
};
use codex_session_manager::restore::RestoreSessionOptions;
use codex_session_manager::session_list::SessionListFilter;
use codex_session_manager::settings::AppSettings;
use std::time::Duration;

fn instance_sync_operation(action: InstanceSyncBridgeAction) -> ProfileOperation {
    ProfileOperation::InstanceSync {
        source_instance_id: 1,
        targets: vec![InstanceSyncBridgeTarget {
            instance_id: 2,
            codex_home: "/home/dev/.codex-work".to_string(),
        }],
        action,
    }
}

fn instance_sync_request() -> InstanceSyncRequest {
    InstanceSyncRequest {
        source_instance_id: 1,
        target_instance_ids: vec![2],
        session_ids: vec!["thread-1".to_string()],
        project_selections: Vec::new(),
        config_paths: Vec::new(),
    }
}

#[test]
fn every_profile_operation_round_trips_through_bridge_json() {
    let operations = vec![
        ProfileOperation::ListSessions {
            filter: SessionListFilter::default(),
        },
        ProfileOperation::LoadSettings,
        ProfileOperation::SaveSettings {
            settings: AppSettings::default(),
        },
        ProfileOperation::ListSessionBackups,
        ProfileOperation::PreviewRestoreSessionBackup {
            backup_id: "backup-1".to_string(),
        },
        ProfileOperation::RestoreSessionBackup {
            backup_id: "backup-1".to_string(),
            options: RestoreSessionOptions {
                apply: true,
                overwrite_existing: false,
                restore_favorite: true,
            },
        },
        ProfileOperation::DeleteSessionBackup {
            backup_id: "backup-1".to_string(),
            confirmed_last_archive: false,
        },
        ProfileOperation::DeleteSessionBackupGroups {
            session_ids: vec!["thread-1".to_string()],
            confirmed_last_archives: false,
        },
        ProfileOperation::ToggleFavorite {
            session_id: "thread-1".to_string(),
        },
        ProfileOperation::SetFavorite {
            session_id: "thread-1".to_string(),
            favorite: true,
        },
        ProfileOperation::ArchiveSessions {
            ids: vec!["thread-1".to_string()],
            apply: true,
        },
        ProfileOperation::ActiveSessions {
            ids: vec!["thread-1".to_string()],
            apply: true,
        },
        ProfileOperation::DeleteSessions {
            ids: vec!["thread-1".to_string()],
            apply: true,
        },
        ProfileOperation::RefreshSessionUpdatedAt {
            ids: vec!["thread-1".to_string()],
            apply: true,
        },
        ProfileOperation::CompactSession {
            id: "thread-1".to_string(),
            apply: true,
        },
        ProfileOperation::CompactSessionWithLocalProviderFallback {
            id: "thread-1".to_string(),
            apply: true,
        },
        ProfileOperation::EditSelectedSessions {
            ids: vec!["thread-1".to_string()],
            edit: SessionEdit {
                title: Some("Title".to_string()),
                ..SessionEdit::default()
            },
            apply: true,
        },
        ProfileOperation::PreviewDatabaseRepairs,
        ProfileOperation::ApplyDatabaseRepairs {
            options: DatabaseRepairOptions {
                selected: vec!["repair-1".to_string()],
            },
        },
        ProfileOperation::DetectCodexRunning,
        ProfileOperation::ApplyDatabaseSyncFromLocal,
        instance_sync_operation(InstanceSyncBridgeAction::Preview {
            request: instance_sync_request(),
        }),
    ];

    for operation in operations {
        let request = BridgeRequest::new(
            ProfileSpec {
                name: "desktop".to_string(),
                codex_home: "/home/dev/.codex".to_string(),
                provider: None,
                model: None,
                path_maps: Vec::new(),
            },
            operation,
        );
        let encoded = serde_json::to_string(&request).unwrap();
        let decoded: BridgeRequest = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.protocol_version, WSL_BRIDGE_PROTOCOL_VERSION);
    }
}

#[test]
fn bridge_error_envelope_is_versioned_and_explicit() {
    let response = BridgeResponse::failure("database is locked");
    let value = serde_json::to_value(response).unwrap();

    assert_eq!(value["protocol_version"], WSL_BRIDGE_PROTOCOL_VERSION);
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"], "database is locked");
    assert!(value.get("result").is_none());
}

#[test]
fn instance_sync_bridge_v2_serializes_registered_ids_and_linux_targets() {
    let request = BridgeRequest::new(
        ProfileSpec {
            name: "managed-instance-1".to_string(),
            codex_home: "/home/dev/.codex".to_string(),
            provider: None,
            model: None,
            path_maps: Vec::new(),
        },
        instance_sync_operation(InstanceSyncBridgeAction::Preview {
            request: instance_sync_request(),
        }),
    );
    let value = serde_json::to_value(request).unwrap();

    assert_eq!(value["protocol_version"], 2);
    assert_eq!(value["operation"]["operation"], "instance_sync");
    assert_eq!(value["operation"]["source_instance_id"], 1);
    assert_eq!(value["operation"]["targets"][0]["instance_id"], 2);
    assert_eq!(
        value["operation"]["targets"][0]["codex_home"],
        "/home/dev/.codex-work"
    );
    assert_eq!(value["operation"]["action"]["action"], "preview");
}

#[test]
fn profile_operations_have_a_complete_stop_guard_matrix() {
    let operation = |operation: ProfileOperation, expected: bool| {
        assert_eq!(
            operation.requires_codex_stopped(),
            expected,
            "unexpected stop guard for {operation:?}"
        );
    };

    operation(
        ProfileOperation::ListSessions {
            filter: SessionListFilter::default(),
        },
        false,
    );
    operation(ProfileOperation::LoadSettings, false);
    operation(
        ProfileOperation::SaveSettings {
            settings: AppSettings::default(),
        },
        true,
    );
    operation(ProfileOperation::ListSessionBackups, false);
    operation(
        ProfileOperation::PreviewRestoreSessionBackup {
            backup_id: "backup-1".to_string(),
        },
        false,
    );
    operation(
        ProfileOperation::RestoreSessionBackup {
            backup_id: "backup-1".to_string(),
            options: RestoreSessionOptions {
                apply: false,
                overwrite_existing: true,
                restore_favorite: true,
            },
        },
        false,
    );
    operation(
        ProfileOperation::RestoreSessionBackup {
            backup_id: "backup-1".to_string(),
            options: RestoreSessionOptions {
                apply: true,
                overwrite_existing: true,
                restore_favorite: true,
            },
        },
        true,
    );
    operation(
        ProfileOperation::DeleteSessionBackup {
            backup_id: "backup-1".to_string(),
            confirmed_last_archive: false,
        },
        true,
    );
    operation(
        ProfileOperation::DeleteSessionBackupGroups {
            session_ids: vec!["thread-1".to_string()],
            confirmed_last_archives: false,
        },
        true,
    );
    operation(
        ProfileOperation::ToggleFavorite {
            session_id: "thread-1".to_string(),
        },
        true,
    );
    operation(
        ProfileOperation::SetFavorite {
            session_id: "thread-1".to_string(),
            favorite: true,
        },
        true,
    );

    let operation_factories: Vec<(&str, Box<dyn Fn(bool) -> ProfileOperation>)> = vec![
        (
            "archive",
            Box::new(|apply| ProfileOperation::ArchiveSessions {
                ids: vec!["thread-1".to_string()],
                apply,
            }),
        ),
        (
            "active",
            Box::new(|apply| ProfileOperation::ActiveSessions {
                ids: vec!["thread-1".to_string()],
                apply,
            }),
        ),
        (
            "delete",
            Box::new(|apply| ProfileOperation::DeleteSessions {
                ids: vec!["thread-1".to_string()],
                apply,
            }),
        ),
        (
            "refresh updated at",
            Box::new(|apply| ProfileOperation::RefreshSessionUpdatedAt {
                ids: vec!["thread-1".to_string()],
                apply,
            }),
        ),
        (
            "compact",
            Box::new(|apply| ProfileOperation::CompactSession {
                id: "thread-1".to_string(),
                apply,
            }),
        ),
        (
            "compact fallback",
            Box::new(
                |apply| ProfileOperation::CompactSessionWithLocalProviderFallback {
                    id: "thread-1".to_string(),
                    apply,
                },
            ),
        ),
        (
            "edit",
            Box::new(|apply| ProfileOperation::EditSelectedSessions {
                ids: vec!["thread-1".to_string()],
                edit: SessionEdit {
                    title: Some("Title".to_string()),
                    ..SessionEdit::default()
                },
                apply,
            }),
        ),
    ];
    for (name, operation_factory) in operation_factories {
        assert_eq!(
            operation_factory(false).requires_codex_stopped(),
            false,
            "dry-run {name} must not require Codex to stop"
        );
        assert_eq!(
            operation_factory(true).requires_codex_stopped(),
            true,
            "apply {name} must require Codex to stop"
        );
    }

    operation(ProfileOperation::PreviewDatabaseRepairs, false);
    operation(
        ProfileOperation::ApplyDatabaseRepairs {
            options: DatabaseRepairOptions {
                selected: vec!["repair-1".to_string()],
            },
        },
        true,
    );
    operation(ProfileOperation::DetectCodexRunning, false);
    operation(ProfileOperation::ApplyDatabaseSyncFromLocal, true);
    operation(
        instance_sync_operation(InstanceSyncBridgeAction::Preview {
            request: instance_sync_request(),
        }),
        false,
    );
    operation(
        instance_sync_operation(InstanceSyncBridgeAction::Execute {
            request: instance_sync_request(),
        }),
        true,
    );
    operation(
        instance_sync_operation(InstanceSyncBridgeAction::DetectCodexRunning),
        false,
    );
}

#[test]
fn bridge_identity_round_trips_and_normalizes_architecture_aliases() {
    let identity =
        codex_session_manager::profile_operation::BridgeIdentity::for_architecture("arm64")
            .unwrap();
    let encoded = serde_json::to_string(&identity).unwrap();
    let decoded: codex_session_manager::profile_operation::BridgeIdentity =
        serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded, identity);
    assert_eq!(decoded.protocol_version, WSL_BRIDGE_PROTOCOL_VERSION);
    assert_eq!(WSL_BRIDGE_PROTOCOL_VERSION, 2);
    assert_eq!(decoded.target_architecture, "aarch64");
    assert!(!decoded.app_version.is_empty());
}

#[test]
fn profile_operations_use_the_declared_wsl_timeout_tiers() {
    assert_eq!(
        ProfileOperation::ListSessions {
            filter: SessionListFilter::default()
        }
        .wsl_timeout(),
        Duration::from_secs(150)
    );
    assert_eq!(
        ProfileOperation::CompactSession {
            id: "thread-1".to_string(),
            apply: false,
        }
        .wsl_timeout(),
        Duration::from_secs(600)
    );
    assert_eq!(
        ProfileOperation::RestoreSessionBackup {
            backup_id: "backup-1".to_string(),
            options: RestoreSessionOptions {
                apply: true,
                overwrite_existing: false,
                restore_favorite: false,
            },
        }
        .wsl_timeout(),
        Duration::from_secs(600)
    );
    assert_eq!(
        ProfileOperation::ArchiveSessions {
            ids: vec!["thread-1".to_string()],
            apply: true,
        }
        .wsl_timeout(),
        Duration::from_secs(600)
    );
    assert_eq!(
        ProfileOperation::ArchiveSessions {
            ids: vec!["thread-1".to_string()],
            apply: false,
        }
        .wsl_timeout(),
        Duration::from_secs(150)
    );
    assert_eq!(
        ProfileOperation::ApplyDatabaseSyncFromLocal.wsl_timeout(),
        Duration::from_secs(600)
    );
    assert_eq!(
        instance_sync_operation(InstanceSyncBridgeAction::Preview {
            request: instance_sync_request(),
        })
        .wsl_timeout(),
        Duration::from_secs(150)
    );
    assert_eq!(
        instance_sync_operation(InstanceSyncBridgeAction::Execute {
            request: instance_sync_request(),
        })
        .wsl_timeout(),
        Duration::from_secs(600)
    );
}

#[test]
fn only_safe_profile_operations_retry_after_a_missing_bridge_marker() {
    let retryable = vec![
        ProfileOperation::ListSessions {
            filter: SessionListFilter::default(),
        },
        ProfileOperation::LoadSettings,
        ProfileOperation::ListSessionBackups,
        ProfileOperation::PreviewRestoreSessionBackup {
            backup_id: "backup-1".to_string(),
        },
        ProfileOperation::RestoreSessionBackup {
            backup_id: "backup-1".to_string(),
            options: RestoreSessionOptions {
                apply: false,
                overwrite_existing: true,
                restore_favorite: true,
            },
        },
        ProfileOperation::ArchiveSessions {
            ids: vec!["thread-1".to_string()],
            apply: false,
        },
        ProfileOperation::ActiveSessions {
            ids: vec!["thread-1".to_string()],
            apply: false,
        },
        ProfileOperation::DeleteSessions {
            ids: vec!["thread-1".to_string()],
            apply: false,
        },
        ProfileOperation::RefreshSessionUpdatedAt {
            ids: vec!["thread-1".to_string()],
            apply: false,
        },
        ProfileOperation::CompactSession {
            id: "thread-1".to_string(),
            apply: false,
        },
        ProfileOperation::CompactSessionWithLocalProviderFallback {
            id: "thread-1".to_string(),
            apply: false,
        },
        ProfileOperation::EditSelectedSessions {
            ids: vec!["thread-1".to_string()],
            edit: SessionEdit::default(),
            apply: false,
        },
        ProfileOperation::PreviewDatabaseRepairs,
        ProfileOperation::DetectCodexRunning,
        instance_sync_operation(InstanceSyncBridgeAction::Preview {
            request: instance_sync_request(),
        }),
    ];
    for operation in retryable {
        assert!(
            operation.can_retry_after_missing_response_marker(),
            "read-only or dry-run operation must be retryable: {operation:?}"
        );
    }

    let non_retryable = vec![
        ProfileOperation::SaveSettings {
            settings: AppSettings::default(),
        },
        ProfileOperation::DeleteSessionBackup {
            backup_id: "backup-1".to_string(),
            confirmed_last_archive: false,
        },
        ProfileOperation::DeleteSessionBackupGroups {
            session_ids: vec!["thread-1".to_string()],
            confirmed_last_archives: false,
        },
        ProfileOperation::ToggleFavorite {
            session_id: "thread-1".to_string(),
        },
        ProfileOperation::SetFavorite {
            session_id: "thread-1".to_string(),
            favorite: true,
        },
        ProfileOperation::RestoreSessionBackup {
            backup_id: "backup-1".to_string(),
            options: RestoreSessionOptions {
                apply: true,
                overwrite_existing: true,
                restore_favorite: true,
            },
        },
        ProfileOperation::ArchiveSessions {
            ids: vec!["thread-1".to_string()],
            apply: true,
        },
        ProfileOperation::ActiveSessions {
            ids: vec!["thread-1".to_string()],
            apply: true,
        },
        ProfileOperation::DeleteSessions {
            ids: vec!["thread-1".to_string()],
            apply: true,
        },
        ProfileOperation::RefreshSessionUpdatedAt {
            ids: vec!["thread-1".to_string()],
            apply: true,
        },
        ProfileOperation::CompactSession {
            id: "thread-1".to_string(),
            apply: true,
        },
        ProfileOperation::CompactSessionWithLocalProviderFallback {
            id: "thread-1".to_string(),
            apply: true,
        },
        ProfileOperation::EditSelectedSessions {
            ids: vec!["thread-1".to_string()],
            edit: SessionEdit::default(),
            apply: true,
        },
        ProfileOperation::ApplyDatabaseRepairs {
            options: DatabaseRepairOptions {
                selected: vec!["repair-1".to_string()],
            },
        },
        ProfileOperation::ApplyDatabaseSyncFromLocal,
        instance_sync_operation(InstanceSyncBridgeAction::Execute {
            request: instance_sync_request(),
        }),
    ];
    for operation in non_retryable {
        assert!(
            !operation.can_retry_after_missing_response_marker(),
            "potentially writing operation must not be retried: {operation:?}"
        );
    }
}
