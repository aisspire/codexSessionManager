use std::cell::Cell;
use std::fs;
use std::path::Path;

use codex_session_manager::instance_registry::{list_managed_instances, scan_and_register};
use codex_session_manager::instance_sync::{
    execute_instance_sync_with_guard, list_instance_sync_source_data, preview_instance_sync,
    preview_instance_sync_config_diff, select_instance_sync_non_root_config_differences,
    ConfigPathNode, InstanceSyncConfigDiffRequest, InstanceSyncConfigDiffStatus,
    InstanceSyncNonRootConfigDifferenceRequest, InstanceSyncRequest,
};
use codex_session_manager::state_db::StateDb;
use rusqlite::{params, Connection};
use tempfile::tempdir;

#[test]
fn imports_a_selected_archived_session_without_changing_its_state() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let target_directory = dir.path().join("target");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&source_directory, "model = \"source\"\n");
    write_config(&target_directory, "model = \"target\"\n");
    write_rollout(
        &source_directory
            .join("archived_sessions")
            .join("thread-one.jsonl"),
        "thread-one",
    );

    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let target = instance_id_at(&instances, &target_directory);

    let report = execute_instance_sync_with_guard(
        &registry_path,
        &InstanceSyncRequest {
            source_instance_id: source,
            target_instance_ids: vec![target],
            session_ids: vec!["thread-one".to_string()],
            config_paths: Vec::new(),
        },
        || Ok(()),
    )
    .unwrap();

    assert_eq!(report.targets.len(), 1);
    assert_eq!(report.targets[0].sessions_added, vec!["thread-one"]);
    assert!(target_directory
        .join("archived_sessions")
        .join("thread-one.jsonl")
        .is_file());
    assert!(!target_directory
        .join("sessions")
        .join("thread-one.jsonl")
        .exists());
    assert!(source_directory
        .join("archived_sessions")
        .join("thread-one.jsonl")
        .is_file());
}

#[test]
fn applies_only_selected_config_paths_and_keeps_target_only_values() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let target_directory = dir.path().join("target");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(
        &source_directory,
        r#"
model = "source-model"

[model_providers.office]
base_url = "https://source.example/v1"
api_key = "source-secret"
"#,
    );
    write_config(
        &target_directory,
        r#"
model = "target-model"
target_only = true

[model_providers.office]
base_url = "https://target.example/v1"
api_key = "target-secret"
"#,
    );

    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let target = instance_id_at(&instances, &target_directory);

    let report = execute_instance_sync_with_guard(
        &registry_path,
        &InstanceSyncRequest {
            source_instance_id: source,
            target_instance_ids: vec![target],
            session_ids: Vec::new(),
            config_paths: vec![
                vec!["model".to_string()],
                vec![
                    "model_providers".to_string(),
                    "office".to_string(),
                    "base_url".to_string(),
                ],
            ],
        },
        || Ok(()),
    )
    .unwrap();

    assert_eq!(report.targets[0].config_paths_applied, 2);
    let target_config = fs::read_to_string(target_directory.join("config.toml")).unwrap();
    assert!(target_config.contains("model = \"source-model\""));
    assert!(target_config.contains("base_url = \"https://source.example/v1\""));
    assert!(target_config.contains("api_key = \"target-secret\""));
    assert!(target_config.contains("target_only = true"));
}

#[test]
fn merges_index_and_upserts_only_the_imported_session_thread() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let target_directory = dir.path().join("target");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&source_directory, "model = \"source\"\n");
    write_config(&target_directory, "model = \"target\"\n");
    let source_rollout = source_directory
        .join("sessions")
        .join("thread-import.jsonl");
    write_rollout(&source_rollout, "thread-import");
    fs::write(
        source_directory.join("session_index.jsonl"),
        r#"{"id":"thread-import","thread_name":"导入会话","updated_at":"2026-07-01T00:00:00Z","extra":"preserved"}"#,
    )
    .unwrap();
    create_state_db(&source_directory.join("state_5.sqlite"));
    insert_thread(
        &source_directory.join("state_5.sqlite"),
        "thread-import",
        source_rollout.to_str().unwrap(),
        false,
        "source title",
    );
    insert_thread(
        &source_directory.join("state_5.sqlite"),
        "source-unselected",
        "source-unselected.jsonl",
        false,
        "must not copy",
    );

    fs::write(
        target_directory.join("session_index.jsonl"),
        r#"{"id":"target-existing","thread_name":"保留会话"}"#,
    )
    .unwrap();
    create_state_db(&target_directory.join("state_5.sqlite"));
    insert_thread(
        &target_directory.join("state_5.sqlite"),
        "target-existing",
        "target-existing.jsonl",
        false,
        "keep target",
    );

    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let target = instance_id_at(&instances, &target_directory);

    let report = execute_instance_sync_with_guard(
        &registry_path,
        &InstanceSyncRequest {
            source_instance_id: source,
            target_instance_ids: vec![target],
            session_ids: vec!["thread-import".to_string()],
            config_paths: Vec::new(),
        },
        || Ok(()),
    )
    .unwrap();

    assert_eq!(report.targets[0].index_entries, 1);
    assert_eq!(report.targets[0].sqlite_rows, 1);
    let index = fs::read_to_string(target_directory.join("session_index.jsonl")).unwrap();
    assert!(index.contains("target-existing"));
    assert!(index.contains("thread-import"));
    assert!(index.contains("\"extra\":\"preserved\""));

    let target_db = StateDb::open(&target_directory.join("state_5.sqlite")).unwrap();
    let threads = target_db.read_threads().unwrap();
    assert!(threads.iter().any(|thread| {
        thread.id == "thread-import"
            && thread
                .rollout_path
                .as_deref()
                .is_some_and(|path| path.ends_with("sessions\\thread-import.jsonl"))
            && thread.title.as_deref() == Some("source title")
    }));
    assert!(threads.iter().any(|thread| thread.id == "target-existing"));
    assert!(!threads
        .iter()
        .any(|thread| thread.id == "source-unselected"));
}

#[test]
fn skips_identical_sessions_and_preserves_conflicting_target_sessions() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let target_directory = dir.path().join("target");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&source_directory, "model = \"source\"\n");
    write_config(&target_directory, "model = \"target\"\n");
    let source_same = source_directory.join("sessions").join("thread-same.jsonl");
    let source_conflict = source_directory
        .join("sessions")
        .join("thread-conflict.jsonl");
    write_rollout(&source_same, "thread-same");
    write_rollout(&source_conflict, "thread-conflict");
    fs::create_dir_all(target_directory.join("sessions")).unwrap();
    fs::copy(
        &source_same,
        target_directory.join("sessions").join("thread-same.jsonl"),
    )
    .unwrap();
    let target_conflict = target_directory
        .join("sessions")
        .join("thread-conflict.jsonl");
    write_rollout_with_provider(&target_conflict, "thread-conflict", "different-provider");
    let before_conflict = fs::read(&target_conflict).unwrap();

    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let target = instance_id_at(&instances, &target_directory);

    let report = execute_instance_sync_with_guard(
        &registry_path,
        &InstanceSyncRequest {
            source_instance_id: source,
            target_instance_ids: vec![target],
            session_ids: vec!["thread-same".to_string(), "thread-conflict".to_string()],
            config_paths: Vec::new(),
        },
        || Ok(()),
    )
    .unwrap();

    assert_eq!(report.targets[0].sessions_skipped, vec!["thread-same"]);
    assert_eq!(report.targets[0].session_conflicts.len(), 1);
    assert_eq!(
        report.targets[0].session_conflicts[0].session_id,
        "thread-conflict"
    );
    assert_eq!(fs::read(&target_conflict).unwrap(), before_conflict);
    assert!(report.targets[0]
        .backup_dir
        .as_deref()
        .is_some_and(|path| Path::new(path).is_dir()));
}

#[test]
fn continues_with_later_targets_when_one_target_fails_preflight() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let healthy_target_directory = dir.path().join("healthy-target");
    let invalid_target_directory = dir.path().join("invalid-target");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&source_directory, "model = \"source\"\n");
    write_config(&healthy_target_directory, "model = \"healthy\"\n");
    write_config(&invalid_target_directory, "model = [\n");
    write_rollout(
        &source_directory.join("sessions").join("thread-one.jsonl"),
        "thread-one",
    );
    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let healthy_target = instance_id_at(&instances, &healthy_target_directory);
    let invalid_target = instance_id_at(&instances, &invalid_target_directory);

    let report = execute_instance_sync_with_guard(
        &registry_path,
        &InstanceSyncRequest {
            source_instance_id: source,
            target_instance_ids: vec![healthy_target, invalid_target],
            session_ids: vec!["thread-one".to_string()],
            config_paths: vec![vec!["model".to_string()]],
        },
        || Ok(()),
    )
    .unwrap();

    assert!(report.targets[0].error.is_none());
    assert!(healthy_target_directory
        .join("sessions")
        .join("thread-one.jsonl")
        .is_file());
    assert!(report.targets[1].error.is_some());
    assert!(!invalid_target_directory
        .join("sessions")
        .join("thread-one.jsonl")
        .exists());
    assert!(report.targets[1].backup_dir.is_none());
}

#[test]
fn lists_source_sessions_and_selectable_config_leaf_paths() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(
        &source_directory,
        r#"
model = "source"

[model_providers.office]
name = "Office"
base_url = "https://source.example/v1"
"#,
    );
    write_rollout(
        &source_directory.join("sessions").join("active.jsonl"),
        "thread-active",
    );
    write_rollout(
        &source_directory
            .join("archived_sessions")
            .join("archived.jsonl"),
        "thread-archived",
    );
    scan_and_register(&registry_path, dir.path()).unwrap();
    let source = instance_id_at(
        &list_managed_instances(&registry_path).unwrap(),
        &source_directory,
    );

    let data = list_instance_sync_source_data(&registry_path, source).unwrap();

    assert_eq!(data.sessions.len(), 2);
    assert!(data
        .sessions
        .iter()
        .any(|session| session.id == "thread-active" && !session.archived));
    assert!(data
        .sessions
        .iter()
        .any(|session| session.id == "thread-archived" && session.archived));
    let paths = flatten_config_paths(&data.config_paths);
    assert!(paths.contains(&(vec!["model".to_string()], true)));
    assert!(paths.contains(&(
        vec![
            "model_providers".to_string(),
            "office".to_string(),
            "name".to_string()
        ],
        true
    )));
    assert!(paths.contains(&(vec!["model_providers".to_string()], false)));
}

#[test]
fn lists_source_session_hover_metadata_without_reading_session_contents() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    let rollout_path = source_directory.join("sessions").join("metadata.jsonl");
    write_config(&source_directory, "model = \"source\"\n");
    write_rollout_with_details(
        &rollout_path,
        "thread-metadata",
        "openai",
        "E:\\project\\metadata",
    );
    fs::write(
        source_directory.join("session_index.jsonl"),
        r#"{"id":"thread-metadata","updated_at":"2026-07-02T03:04:05Z"}"#,
    )
    .unwrap();
    create_state_db(&source_directory.join("state_5.sqlite"));
    insert_thread(
        &source_directory.join("state_5.sqlite"),
        "thread-metadata",
        rollout_path.to_str().unwrap(),
        false,
        "完整的会话标题",
    );
    set_thread_model(
        &source_directory.join("state_5.sqlite"),
        "thread-metadata",
        "gpt-5",
    );

    scan_and_register(&registry_path, dir.path()).unwrap();
    let source = instance_id_at(
        &list_managed_instances(&registry_path).unwrap(),
        &source_directory,
    );

    let data = list_instance_sync_source_data(&registry_path, source).unwrap();
    let session = data
        .sessions
        .iter()
        .find(|session| session.id == "thread-metadata")
        .unwrap();

    assert_eq!(session.title.as_deref(), Some("完整的会话标题"));
    assert_eq!(session.project.as_deref(), Some("E:\\project\\metadata"));
    assert!(!session.archived);
    assert_eq!(session.source.as_deref(), Some("cli"));
    assert_eq!(session.model_provider.as_deref(), Some("openai"));
    assert_eq!(session.model.as_deref(), Some("gpt-5"));
    assert_eq!(session.updated_at.as_deref(), Some("2026-07-02T03:04:05Z"));
    assert_eq!(session.source_path, rollout_path.display().to_string());
}

#[test]
fn previews_config_diff_for_each_target_without_writing_configuration() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let changed_directory = dir.path().join("changed");
    let same_directory = dir.path().join("same");
    let missing_directory = dir.path().join("missing");
    let unreadable_directory = dir.path().join("unreadable");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&source_directory, "model = \"source-model\"\n");
    write_config(&changed_directory, "model = \"target-model\"\n");
    write_config(&same_directory, "model = \"source-model\"\n");
    write_config(&missing_directory, "other = true\n");
    write_config(&unreadable_directory, "model = [\n");

    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let changed = instance_id_at(&instances, &changed_directory);
    let same = instance_id_at(&instances, &same_directory);
    let missing = instance_id_at(&instances, &missing_directory);
    let unreadable = instance_id_at(&instances, &unreadable_directory);

    let diff = preview_instance_sync_config_diff(
        &registry_path,
        &InstanceSyncConfigDiffRequest {
            source_instance_id: source,
            target_instance_ids: vec![changed, same, missing, unreadable],
            config_path: vec!["model".to_string()],
        },
    )
    .unwrap();

    assert_eq!(diff.source_value, "\"source-model\"");
    assert_eq!(diff.targets.len(), 4);
    assert_eq!(diff.targets[0].target_instance_id, changed);
    assert_eq!(
        diff.targets[0].status,
        InstanceSyncConfigDiffStatus::Changed
    );
    assert_eq!(
        diff.targets[0].original_value.as_deref(),
        Some("\"target-model\"")
    );
    assert_eq!(diff.targets[1].status, InstanceSyncConfigDiffStatus::Same);
    assert_eq!(
        diff.targets[1].original_value.as_deref(),
        Some("\"source-model\"")
    );
    assert_eq!(
        diff.targets[2].status,
        InstanceSyncConfigDiffStatus::Missing
    );
    assert_eq!(diff.targets[2].original_value, None);
    assert_eq!(
        diff.targets[3].status,
        InstanceSyncConfigDiffStatus::ReadError
    );
    assert!(diff.targets[3]
        .error
        .as_deref()
        .is_some_and(|error| error.contains("parse")));
    assert_eq!(
        fs::read_to_string(changed_directory.join("config.toml")).unwrap(),
        "model = \"target-model\"\n",
    );
}

#[test]
fn config_diff_rejects_an_unavailable_registered_target() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let target_directory = dir.path().join("target");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&source_directory, "model = \"source-model\"\n");
    write_config(&target_directory, "model = \"target-model\"\n");
    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let target = instance_id_at(&instances, &target_directory);
    fs::remove_file(target_directory.join("config.toml")).unwrap();

    let error = preview_instance_sync_config_diff(
        &registry_path,
        &InstanceSyncConfigDiffRequest {
            source_instance_id: source,
            target_instance_ids: vec![target],
            config_path: vec!["model".to_string()],
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("not available"));
}

#[test]
fn select_instance_sync_non_root_config_differences_excludes_root_and_preserves_target_configs() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let changed_directory = dir.path().join("changed");
    let missing_directory = dir.path().join("missing");
    let unreadable_directory = dir.path().join("unreadable");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(
        &source_directory,
        r#"model = "source-model"

[model_providers.office]
api_key = "source-key"
timeout = 30

[features]
enabled = true
"#,
    );
    let changed_config = r#"model = "target-model"

[model_providers.office]
api_key = "target-key"
timeout = 30

[features]
enabled = false
"#;
    write_config(&changed_directory, changed_config);
    let missing_config = r#"model = "source-model"

[model_providers.office]
api_key = "source-key"

[features]
enabled = true
"#;
    write_config(&missing_directory, missing_config);
    write_config(&unreadable_directory, "model = [\n");

    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let changed = instance_id_at(&instances, &changed_directory);
    let missing = instance_id_at(&instances, &missing_directory);
    let unreadable = instance_id_at(&instances, &unreadable_directory);

    let selection = select_instance_sync_non_root_config_differences(
        &registry_path,
        &InstanceSyncNonRootConfigDifferenceRequest {
            source_instance_id: source,
            target_instance_ids: vec![changed, missing, unreadable],
        },
    )
    .unwrap();

    assert_eq!(selection.source_instance_id, source);
    assert_eq!(
        selection.config_paths,
        vec![
            vec![
                "model_providers".to_string(),
                "office".to_string(),
                "api_key".to_string(),
            ],
            vec![
                "model_providers".to_string(),
                "office".to_string(),
                "timeout".to_string(),
            ],
            vec!["features".to_string(), "enabled".to_string()],
        ],
    );
    assert_eq!(selection.unreadable_target_instance_ids, vec![unreadable]);
    assert!(!selection.config_paths.contains(&vec!["model".to_string()]));
    assert_eq!(
        fs::read_to_string(changed_directory.join("config.toml")).unwrap(),
        changed_config,
    );
    assert_eq!(
        fs::read_to_string(missing_directory.join("config.toml")).unwrap(),
        missing_config,
    );
}

#[test]
fn select_instance_sync_non_root_config_differences_rejects_an_unavailable_target() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let target_directory = dir.path().join("target");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(
        &source_directory,
        "[model_providers.office]\napi_key = \"source-key\"\n",
    );
    write_config(
        &target_directory,
        "[model_providers.office]\napi_key = \"target-key\"\n",
    );
    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let target = instance_id_at(&instances, &target_directory);
    fs::remove_file(target_directory.join("config.toml")).unwrap();

    let error = select_instance_sync_non_root_config_differences(
        &registry_path,
        &InstanceSyncNonRootConfigDifferenceRequest {
            source_instance_id: source,
            target_instance_ids: vec![target],
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("not available"));
}

#[test]
fn previews_new_skipped_and_conflicting_sessions_without_writing() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let target_directory = dir.path().join("target");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&source_directory, "model = \"source\"\n");
    write_config(&target_directory, "model = \"target\"\n");
    let source_same = source_directory.join("sessions").join("thread-same.jsonl");
    let source_conflict = source_directory
        .join("sessions")
        .join("thread-conflict.jsonl");
    let source_new = source_directory.join("sessions").join("thread-new.jsonl");
    write_rollout(&source_same, "thread-same");
    write_rollout(&source_conflict, "thread-conflict");
    write_rollout(&source_new, "thread-new");
    fs::create_dir_all(target_directory.join("sessions")).unwrap();
    fs::copy(
        &source_same,
        target_directory.join("sessions").join("thread-same.jsonl"),
    )
    .unwrap();
    write_rollout_with_provider(
        &target_directory
            .join("sessions")
            .join("thread-conflict.jsonl"),
        "thread-conflict",
        "target-provider",
    );
    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let target = instance_id_at(&instances, &target_directory);

    let preview = preview_instance_sync(
        &registry_path,
        &InstanceSyncRequest {
            source_instance_id: source,
            target_instance_ids: vec![target],
            session_ids: vec![
                "thread-same".to_string(),
                "thread-conflict".to_string(),
                "thread-new".to_string(),
            ],
            config_paths: Vec::new(),
        },
    )
    .unwrap();

    assert_eq!(preview.targets.len(), 1);
    assert_eq!(preview.targets[0].sessions_to_add, vec!["thread-new"]);
    assert_eq!(preview.targets[0].sessions_to_skip, vec!["thread-same"]);
    assert_eq!(preview.targets[0].session_conflicts.len(), 1);
    assert_eq!(
        preview.targets[0].session_conflicts[0].session_id,
        "thread-conflict"
    );
    assert!(!preview.targets[0].project_path_warnings.is_empty());
    assert!(!target_directory
        .join("sessions")
        .join("thread-new.jsonl")
        .exists());
}

#[test]
fn refuses_every_target_before_writing_when_codex_is_running() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let target_directory = dir.path().join("target");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&source_directory, "model = \"source\"\n");
    write_config(&target_directory, "model = \"target\"\n");
    write_rollout(
        &source_directory.join("sessions").join("thread-one.jsonl"),
        "thread-one",
    );
    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let target = instance_id_at(&instances, &target_directory);

    let error = execute_instance_sync_with_guard(
        &registry_path,
        &InstanceSyncRequest {
            source_instance_id: source,
            target_instance_ids: vec![target],
            session_ids: vec!["thread-one".to_string()],
            config_paths: Vec::new(),
        },
        || Err(anyhow::anyhow!("Codex appears to be running")),
    )
    .unwrap_err();

    assert!(error.to_string().contains("Codex appears to be running"));
    assert!(!target_directory
        .join("sessions")
        .join("thread-one.jsonl")
        .exists());
    assert!(!target_directory.join("backups").exists());
}

#[test]
fn keeps_project_path_verbatim_and_warns_when_it_is_missing_on_the_target_machine() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let target_directory = dir.path().join("target");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    let missing_project = dir.path().join("not-created-project");
    write_config(&source_directory, "model = \"source\"\n");
    write_config(&target_directory, "model = \"target\"\n");
    write_rollout_with_details(
        &source_directory.join("sessions").join("thread-one.jsonl"),
        "thread-one",
        "openai",
        &missing_project.to_string_lossy(),
    );
    create_state_db(&target_directory.join("state_5.sqlite"));
    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let target = instance_id_at(&instances, &target_directory);

    let report = execute_instance_sync_with_guard(
        &registry_path,
        &InstanceSyncRequest {
            source_instance_id: source,
            target_instance_ids: vec![target],
            session_ids: vec!["thread-one".to_string()],
            config_paths: Vec::new(),
        },
        || Ok(()),
    )
    .unwrap();

    assert!(report.targets[0]
        .warnings
        .iter()
        .any(|warning| warning.contains("项目路径")));
    let target_thread = StateDb::open(&target_directory.join("state_5.sqlite"))
        .unwrap()
        .read_threads()
        .unwrap()
        .into_iter()
        .find(|thread| thread.id == "thread-one")
        .unwrap();
    assert_eq!(
        target_thread.cwd.as_deref(),
        Some(missing_project.to_string_lossy().as_ref())
    );
}

#[test]
fn treats_any_different_duplicate_target_session_as_a_conflict() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let target_directory = dir.path().join("target");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&source_directory, "model = \"source\"\n");
    write_config(&target_directory, "model = \"target\"\n");
    let source_rollout = source_directory.join("sessions").join("thread-one.jsonl");
    write_rollout(&source_rollout, "thread-one");
    fs::create_dir_all(target_directory.join("sessions")).unwrap();
    fs::copy(
        &source_rollout,
        target_directory.join("sessions").join("thread-one.jsonl"),
    )
    .unwrap();
    write_rollout_with_provider(
        &target_directory
            .join("archived_sessions")
            .join("thread-one.jsonl"),
        "thread-one",
        "different-provider",
    );
    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let target = instance_id_at(&instances, &target_directory);

    let report = execute_instance_sync_with_guard(
        &registry_path,
        &InstanceSyncRequest {
            source_instance_id: source,
            target_instance_ids: vec![target],
            session_ids: vec!["thread-one".to_string()],
            config_paths: Vec::new(),
        },
        || Ok(()),
    )
    .unwrap();

    assert!(report.targets[0].sessions_skipped.is_empty());
    assert_eq!(report.targets[0].session_conflicts.len(), 1);
}

#[test]
fn stops_remaining_targets_when_codex_starts_between_target_preflights() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let first_target_directory = dir.path().join("first-target");
    let second_target_directory = dir.path().join("second-target");
    let third_target_directory = dir.path().join("third-target");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&source_directory, "model = \"source\"\n");
    write_config(&first_target_directory, "model = \"first\"\n");
    write_config(&second_target_directory, "model = \"second\"\n");
    write_config(&third_target_directory, "model = \"third\"\n");
    write_rollout(
        &source_directory.join("sessions").join("thread-one.jsonl"),
        "thread-one",
    );
    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let first_target = instance_id_at(&instances, &first_target_directory);
    let second_target = instance_id_at(&instances, &second_target_directory);
    let third_target = instance_id_at(&instances, &third_target_directory);
    let guard_calls = Cell::new(0);

    let report = execute_instance_sync_with_guard(
        &registry_path,
        &InstanceSyncRequest {
            source_instance_id: source,
            target_instance_ids: vec![first_target, second_target, third_target],
            session_ids: vec!["thread-one".to_string()],
            config_paths: Vec::new(),
        },
        || {
            let call = guard_calls.get() + 1;
            guard_calls.set(call);
            if call == 2 {
                Err(anyhow::anyhow!("Codex appears to be running"))
            } else {
                Ok(())
            }
        },
    )
    .unwrap();

    assert_eq!(guard_calls.get(), 2);
    assert!(first_target_directory
        .join("sessions")
        .join("thread-one.jsonl")
        .is_file());
    for (target, target_directory) in report
        .targets
        .iter()
        .skip(1)
        .zip([&second_target_directory, &third_target_directory])
    {
        assert!(target
            .error
            .as_deref()
            .is_some_and(|error| error.contains("Codex appears to be running")));
        assert!(!target_directory
            .join("sessions")
            .join("thread-one.jsonl")
            .exists());
    }
}

#[test]
fn treats_a_different_target_session_index_entry_as_a_conflict() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let target_directory = dir.path().join("target");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&source_directory, "model = \"source\"\n");
    write_config(&target_directory, "model = \"target\"\n");
    write_rollout(
        &source_directory.join("sessions").join("thread-one.jsonl"),
        "thread-one",
    );
    fs::write(
        source_directory.join("session_index.jsonl"),
        r#"{"id":"thread-one","thread_name":"源端标题"}"#,
    )
    .unwrap();
    let target_index = target_directory.join("session_index.jsonl");
    fs::write(
        &target_index,
        r#"{"id":"thread-one","thread_name":"目标旧标题"}"#,
    )
    .unwrap();
    let before_index = fs::read_to_string(&target_index).unwrap();
    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let target = instance_id_at(&instances, &target_directory);

    let preview = preview_instance_sync(
        &registry_path,
        &InstanceSyncRequest {
            source_instance_id: source,
            target_instance_ids: vec![target],
            session_ids: vec!["thread-one".to_string()],
            config_paths: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(preview.targets[0].session_conflicts.len(), 1);

    let report = execute_instance_sync_with_guard(
        &registry_path,
        &InstanceSyncRequest {
            source_instance_id: source,
            target_instance_ids: vec![target],
            session_ids: vec!["thread-one".to_string()],
            config_paths: Vec::new(),
        },
        || Ok(()),
    )
    .unwrap();

    assert!(report.targets[0].sessions_added.is_empty());
    assert_eq!(report.targets[0].session_conflicts.len(), 1);
    assert_eq!(
        report.targets[0].session_conflicts[0].session_id,
        "thread-one"
    );
    assert!(!target_directory
        .join("sessions")
        .join("thread-one.jsonl")
        .exists());
    assert_eq!(fs::read_to_string(target_index).unwrap(), before_index);
}

#[test]
fn retains_backup_and_completed_session_details_when_index_write_fails() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let target_directory = dir.path().join("target");
    let registry_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&source_directory, "model = \"source\"\n");
    write_config(&target_directory, "model = \"target\"\n");
    write_rollout(
        &source_directory.join("sessions").join("thread-one.jsonl"),
        "thread-one",
    );
    fs::write(
        source_directory.join("session_index.jsonl"),
        r#"{"id":"thread-one","thread_name":"源端标题"}"#,
    )
    .unwrap();
    let target_index = target_directory.join("session_index.jsonl");
    fs::write(
        &target_index,
        r#"{"id":"target-existing","thread_name":"保留标题"}"#,
    )
    .unwrap();
    let original_permissions = fs::metadata(&target_index).unwrap().permissions();
    let mut read_only_permissions = original_permissions.clone();
    read_only_permissions.set_readonly(true);
    fs::set_permissions(&target_index, read_only_permissions).unwrap();
    scan_and_register(&registry_path, dir.path()).unwrap();
    let instances = list_managed_instances(&registry_path).unwrap();
    let source = instance_id_at(&instances, &source_directory);
    let target = instance_id_at(&instances, &target_directory);

    let result = execute_instance_sync_with_guard(
        &registry_path,
        &InstanceSyncRequest {
            source_instance_id: source,
            target_instance_ids: vec![target],
            session_ids: vec!["thread-one".to_string()],
            config_paths: Vec::new(),
        },
        || Ok(()),
    );
    fs::set_permissions(&target_index, original_permissions).unwrap();
    let report = result.unwrap();

    assert!(report.targets[0].error.is_some());
    assert!(report.targets[0]
        .backup_dir
        .as_deref()
        .is_some_and(|path| Path::new(path).is_dir()));
    assert_eq!(report.targets[0].sessions_added, vec!["thread-one"]);
    assert!(target_directory
        .join("sessions")
        .join("thread-one.jsonl")
        .is_file());
}

fn write_config(directory: &Path, config: &str) {
    fs::create_dir_all(directory).unwrap();
    fs::write(directory.join("config.toml"), config).unwrap();
}

fn write_rollout(path: &Path, id: &str) {
    write_rollout_with_provider(path, id, "openai");
}

fn write_rollout_with_provider(path: &Path, id: &str, provider: &str) {
    let cwd = format!("E:\\project\\{id}");
    write_rollout_with_details(path, id, provider, &cwd);
}

fn write_rollout_with_details(path: &Path, id: &str, provider: &str, cwd: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let id = serde_json::to_string(id).unwrap();
    let provider = serde_json::to_string(provider).unwrap();
    let cwd = serde_json::to_string(cwd).unwrap();
    fs::write(
        path,
        format!(
            "{{\"type\":\"session_meta\",\"payload\":{{\"id\":{id},\"cwd\":{cwd},\"source\":\"cli\",\"model_provider\":{provider}}}}}\n{{\"type\":\"event\",\"payload\":{{}}}}\n"
        ),
    )
    .unwrap();
}

fn instance_id_at(
    instances: &[codex_session_manager::instance_registry::ManagedInstance],
    directory: &Path,
) -> i64 {
    let expected = fs::canonicalize(directory)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    #[cfg(windows)]
    let expected = expected
        .strip_prefix(r"\\?\UNC\")
        .map(|path| format!(r"\\{path}"))
        .or_else(|| expected.strip_prefix(r"\\?\").map(str::to_string))
        .unwrap_or(expected);
    instances
        .iter()
        .find(|instance| instance.path == expected)
        .unwrap()
        .id
}

fn create_state_db(path: &Path) {
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch(
            r#"
            CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                rollout_path TEXT,
                created_at INTEGER,
                updated_at INTEGER,
                source TEXT,
                model_provider TEXT,
                cwd TEXT,
                title TEXT,
                sandbox_policy TEXT,
                approval_mode TEXT,
                tokens_used INTEGER,
                has_user_event INTEGER,
                archived INTEGER,
                first_user_message TEXT,
                model TEXT,
                reasoning_effort TEXT,
                created_at_ms INTEGER,
                updated_at_ms INTEGER
            );
            "#,
        )
        .unwrap();
}

fn insert_thread(path: &Path, id: &str, rollout_path: &str, archived: bool, title: &str) {
    let connection = Connection::open(path).unwrap();
    connection
        .execute(
            r#"
            INSERT INTO threads (
                id,
                rollout_path,
                created_at,
                updated_at,
                source,
                model_provider,
                cwd,
                title,
                sandbox_policy,
                approval_mode,
                tokens_used,
                has_user_event,
                archived,
                first_user_message,
                model,
                reasoning_effort,
                created_at_ms,
                updated_at_ms
            )
            VALUES (?1, ?2, 1, 2, 'cli', 'openai', 'E:\\project', ?3, 'workspace-write', 'on-request', 0, 1, ?4, '', NULL, NULL, 1, 2)
            "#,
            params![id, rollout_path, title, if archived { 1 } else { 0 }],
        )
        .unwrap();
}

fn set_thread_model(path: &Path, id: &str, model: &str) {
    let connection = Connection::open(path).unwrap();
    connection
        .execute(
            "UPDATE threads SET model = ?1 WHERE id = ?2",
            params![model, id],
        )
        .unwrap();
}

fn flatten_config_paths(nodes: &[ConfigPathNode]) -> Vec<(Vec<String>, bool)> {
    let mut paths = Vec::new();
    for node in nodes {
        paths.push((node.path.clone(), node.selectable));
        paths.extend(flatten_config_paths(&node.children));
    }
    paths
}
