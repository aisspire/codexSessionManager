use std::fs;
use std::path::Path;

use codex_session_manager::instance_registry::{
    delete_instance_sync_plan, list_instance_sync_plans, list_managed_instances,
    managed_instance_path, permanently_ignore_managed_instance, register_wsl_instance,
    rename_managed_instance, save_instance_sync_plan, scan_and_register,
    soft_delete_managed_instance, InstanceAvailability, InstanceRuntime, InstanceSyncPlanDraft,
    WslInstanceRegistration,
};
use rusqlite::{params, Connection};
use tempfile::tempdir;

fn write_config(directory: &Path) {
    fs::create_dir_all(directory).unwrap();
    fs::write(directory.join("config.toml"), "model = \"gpt-5\"\n").unwrap();
}

fn registered_path(directory: &Path) -> String {
    let path = fs::canonicalize(directory)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    #[cfg(windows)]
    {
        if let Some(unc_path) = path.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{unc_path}");
        }
        if let Some(normal_path) = path.strip_prefix(r"\\?\") {
            return normal_path.to_string();
        }
    }
    path
}

#[test]
fn scan_registers_config_parent_directories_without_duplicates() {
    let dir = tempdir().unwrap();
    let scan_root = dir.path().join("instances");
    let first_instance = scan_root.join("alpha");
    let second_instance = scan_root.join("nested").join("beta");
    let database_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&first_instance);
    write_config(&second_instance);

    let first_scan = scan_and_register(&database_path, &scan_root).unwrap();
    let second_scan = scan_and_register(&database_path, &scan_root).unwrap();
    let instances = list_managed_instances(&database_path).unwrap();

    assert_eq!(first_scan.added, 2);
    assert_eq!(first_scan.already_managed, 0);
    assert_eq!(second_scan.added, 0);
    assert_eq!(second_scan.already_managed, 2);
    assert_eq!(instances.len(), 2);
    assert!(instances
        .iter()
        .all(|instance| instance.availability == InstanceAvailability::Available));
    assert!(instances
        .iter()
        .any(|instance| instance.path == registered_path(&first_instance)));
    assert!(instances
        .iter()
        .any(|instance| instance.path == registered_path(&second_instance)));
}

#[test]
fn saves_sync_plan_without_session_choices_or_configuration_values() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let target_directory = dir.path().join("target");
    let database_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&source_directory);
    write_config(&target_directory);
    scan_and_register(&database_path, dir.path()).unwrap();
    let instances = list_managed_instances(&database_path).unwrap();
    let source = instances
        .iter()
        .find(|instance| instance.path == registered_path(&source_directory))
        .unwrap();
    let target = instances
        .iter()
        .find(|instance| instance.path == registered_path(&target_directory))
        .unwrap();

    let saved = save_instance_sync_plan(
        &database_path,
        &InstanceSyncPlanDraft {
            id: None,
            name: "办公室同步".to_string(),
            source_instance_id: source.id,
            target_instance_ids: vec![target.id],
            config_paths: vec![
                vec!["model".to_string()],
                vec!["model_providers".to_string(), "office".to_string()],
            ],
            project_selections: Vec::new(),
        },
    )
    .unwrap();

    assert_eq!(saved.name, "办公室同步");
    assert_eq!(saved.source_instance_id, source.id);
    assert_eq!(saved.target_instance_ids, vec![target.id]);
    assert_eq!(
        saved.config_paths,
        vec![
            vec!["model".to_string()],
            vec!["model_providers".to_string(), "office".to_string()]
        ]
    );
    assert!(saved.created_at_unix > 0);
    assert_eq!(
        list_instance_sync_plans(&database_path).unwrap(),
        vec![saved]
    );
}

#[test]
fn migrates_legacy_sync_plans_with_empty_project_selections() {
    let dir = tempdir().unwrap();
    let database_path = dir.path().join("instances.sqlite");
    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute_batch(
            r#"
            CREATE TABLE managed_instances (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                display_name TEXT,
                added_at_unix INTEGER NOT NULL,
                last_seen_at_unix INTEGER NOT NULL
            );
            CREATE TABLE instance_sync_plans (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                source_instance_id INTEGER NOT NULL,
                target_instance_ids_json TEXT NOT NULL,
                config_paths_json TEXT NOT NULL,
                created_at_unix INTEGER NOT NULL,
                updated_at_unix INTEGER NOT NULL
            );
            INSERT INTO instance_sync_plans (
                id,
                name,
                source_instance_id,
                target_instance_ids_json,
                config_paths_json,
                created_at_unix,
                updated_at_unix
            )
            VALUES (1, '旧方案', 1, '[2]', '[["model"]]', 1, 2);
            "#,
        )
        .unwrap();
    drop(connection);

    let plans = list_instance_sync_plans(&database_path).unwrap();

    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].name, "旧方案");
    assert_eq!(
        serde_json::to_value(&plans[0])
            .unwrap()
            .get("project_selections")
            .cloned(),
        Some(serde_json::json!([]))
    );
    let connection = Connection::open(&database_path).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT project_selections_json FROM instance_sync_plans WHERE id = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        "[]"
    );
}

#[test]
fn saves_and_reads_normalized_project_selections() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let target_directory = dir.path().join("target");
    let database_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&source_directory);
    write_config(&target_directory);
    scan_and_register(&database_path, dir.path()).unwrap();
    let instances = list_managed_instances(&database_path).unwrap();
    let source = instance_at(&instances, &source_directory);
    let target = instance_at(&instances, &target_directory);
    let draft = serde_json::from_value::<InstanceSyncPlanDraft>(serde_json::json!({
        "id": null,
        "name": "项目同步",
        "source_instance_id": source.id,
        "target_instance_ids": [target.id],
        "config_paths": [],
        "project_selections": ["E:\\work\\office\\", null]
    }))
    .unwrap();

    let saved = save_instance_sync_plan(&database_path, &draft).unwrap();

    assert_eq!(
        saved.project_selections,
        vec![Some("e:/work/office".to_string()), None]
    );
    assert_eq!(
        list_instance_sync_plans(&database_path).unwrap(),
        vec![saved]
    );
}

#[test]
fn updates_and_deletes_a_saved_sync_plan() {
    let dir = tempdir().unwrap();
    let source_directory = dir.path().join("source");
    let first_target_directory = dir.path().join("first-target");
    let second_target_directory = dir.path().join("second-target");
    let database_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&source_directory);
    write_config(&first_target_directory);
    write_config(&second_target_directory);
    scan_and_register(&database_path, dir.path()).unwrap();
    let instances = list_managed_instances(&database_path).unwrap();
    let source = instance_at(&instances, &source_directory);
    let first_target = instance_at(&instances, &first_target_directory);
    let second_target = instance_at(&instances, &second_target_directory);
    let saved = save_instance_sync_plan(
        &database_path,
        &InstanceSyncPlanDraft {
            id: None,
            name: "初始方案".to_string(),
            source_instance_id: source.id,
            target_instance_ids: vec![first_target.id],
            config_paths: Vec::new(),
            project_selections: Vec::new(),
        },
    )
    .unwrap();

    let updated = save_instance_sync_plan(
        &database_path,
        &InstanceSyncPlanDraft {
            id: Some(saved.id),
            name: "更新后的方案".to_string(),
            source_instance_id: source.id,
            target_instance_ids: vec![second_target.id],
            config_paths: vec![vec!["model".to_string()]],
            project_selections: Vec::new(),
        },
    )
    .unwrap();

    assert_eq!(updated.id, saved.id);
    assert_eq!(updated.name, "更新后的方案");
    assert_eq!(updated.target_instance_ids, vec![second_target.id]);
    assert_eq!(updated.config_paths, vec![vec!["model".to_string()]]);

    delete_instance_sync_plan(&database_path, updated.id).unwrap();
    assert!(list_instance_sync_plans(&database_path).unwrap().is_empty());
}

fn instance_at<'a>(
    instances: &'a [codex_session_manager::instance_registry::ManagedInstance],
    directory: &Path,
) -> &'a codex_session_manager::instance_registry::ManagedInstance {
    instances
        .iter()
        .find(|instance| instance.path == registered_path(directory))
        .unwrap()
}

#[test]
fn registry_persists_alias_and_marks_missing_config_as_unavailable() {
    let dir = tempdir().unwrap();
    let instance_directory = dir.path().join("instance");
    let database_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&instance_directory);

    scan_and_register(&database_path, dir.path()).unwrap();
    let instance = list_managed_instances(&database_path)
        .unwrap()
        .pop()
        .unwrap();
    let renamed = rename_managed_instance(&database_path, instance.id, "办公账号").unwrap();
    fs::remove_file(instance_directory.join("config.toml")).unwrap();
    let persisted = list_managed_instances(&database_path)
        .unwrap()
        .pop()
        .unwrap();

    assert_eq!(renamed.display_name.as_deref(), Some("办公账号"));
    assert_eq!(persisted.display_name.as_deref(), Some("办公账号"));
    assert_eq!(persisted.availability, InstanceAvailability::Unavailable);
}

#[test]
fn logical_delete_hides_instance_and_rescan_reactivates_it_with_its_alias() {
    let dir = tempdir().unwrap();
    let instance_directory = dir.path().join("instance");
    let database_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&instance_directory);

    scan_and_register(&database_path, dir.path()).unwrap();
    let instance = list_managed_instances(&database_path)
        .unwrap()
        .pop()
        .unwrap();
    rename_managed_instance(&database_path, instance.id, "保留的实例名称").unwrap();

    soft_delete_managed_instance(&database_path, instance.id).unwrap();

    assert!(list_managed_instances(&database_path).unwrap().is_empty());
    assert!(managed_instance_path(&database_path, instance.id).is_err());
    assert!(rename_managed_instance(&database_path, instance.id, "不应保存").is_err());

    let rescan = scan_and_register(&database_path, dir.path()).unwrap();
    let reactivated = list_managed_instances(&database_path)
        .unwrap()
        .pop()
        .unwrap();

    assert_eq!(rescan.added, 0);
    assert_eq!(rescan.reactivated, 1);
    assert_eq!(reactivated.id, instance.id);
    assert_eq!(reactivated.display_name.as_deref(), Some("保留的实例名称"));
    assert_eq!(reactivated.path, registered_path(&instance_directory));
}

#[test]
fn logical_delete_migrates_an_existing_registry_without_losing_metadata() {
    let dir = tempdir().unwrap();
    let instance_directory = dir.path().join("instance");
    let database_path = dir.path().join("instances.sqlite");
    write_config(&instance_directory);
    let stored_path = registered_path(&instance_directory);

    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute_batch(
            r#"
            CREATE TABLE managed_instances (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                display_name TEXT,
                added_at_unix INTEGER NOT NULL,
                last_seen_at_unix INTEGER NOT NULL
            );
            "#,
        )
        .unwrap();
    connection
        .execute(
            r#"
            INSERT INTO managed_instances (path, display_name, added_at_unix, last_seen_at_unix)
            VALUES (?1, ?2, 1, 2)
            "#,
            params![stored_path, "旧实例名称"],
        )
        .unwrap();
    drop(connection);

    soft_delete_managed_instance(&database_path, 1).unwrap();

    assert!(list_managed_instances(&database_path).unwrap().is_empty());
    let connection = Connection::open(&database_path).unwrap();
    let (path, display_name, deleted_at): (String, Option<String>, Option<i64>) = connection
        .query_row(
            "SELECT path, display_name, deleted_at_unix FROM managed_instances WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(path, registered_path(&instance_directory));
    assert_eq!(display_name.as_deref(), Some("旧实例名称"));
    assert!(deleted_at.is_some());
}

#[test]
fn permanent_ignore_hides_instance_and_prevents_rescan_without_losing_metadata() {
    let dir = tempdir().unwrap();
    let instance_directory = dir.path().join("instance");
    let database_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&instance_directory);

    scan_and_register(&database_path, dir.path()).unwrap();
    let instance = list_managed_instances(&database_path)
        .unwrap()
        .pop()
        .unwrap();
    rename_managed_instance(&database_path, instance.id, "永久忽略的实例名称").unwrap();

    permanently_ignore_managed_instance(&database_path, instance.id).unwrap();

    assert!(list_managed_instances(&database_path).unwrap().is_empty());
    assert!(managed_instance_path(&database_path, instance.id).is_err());
    assert!(rename_managed_instance(&database_path, instance.id, "不应保存").is_err());

    let rescan = scan_and_register(&database_path, dir.path()).unwrap();

    assert_eq!(rescan.added, 0);
    assert_eq!(rescan.reactivated, 0);
    assert_eq!(rescan.ignored, 1);
    assert_eq!(rescan.already_managed, 0);
    assert!(list_managed_instances(&database_path).unwrap().is_empty());

    let connection = Connection::open(&database_path).unwrap();
    let (path, display_name, ignored_at): (String, Option<String>, Option<i64>) = connection
        .query_row(
            "SELECT path, display_name, ignored_at_unix FROM managed_instances WHERE id = ?1",
            [instance.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        path,
        fs::canonicalize(&instance_directory)
            .unwrap()
            .to_string_lossy()
            .into_owned()
    );
    assert_eq!(display_name.as_deref(), Some("永久忽略的实例名称"));
    assert!(ignored_at.is_some());
}

#[test]
fn permanent_ignore_migrates_logical_delete_registry_without_losing_metadata() {
    let dir = tempdir().unwrap();
    let instance_directory = dir.path().join("instance");
    let database_path = dir.path().join("instances.sqlite");
    write_config(&instance_directory);
    let stored_path = registered_path(&instance_directory);

    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute_batch(
            r#"
            CREATE TABLE managed_instances (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                display_name TEXT,
                added_at_unix INTEGER NOT NULL,
                last_seen_at_unix INTEGER NOT NULL,
                deleted_at_unix INTEGER
            );
            "#,
        )
        .unwrap();
    connection
        .execute(
            r#"
            INSERT INTO managed_instances (path, display_name, added_at_unix, last_seen_at_unix)
            VALUES (?1, ?2, 1, 2)
            "#,
            params![stored_path, "已有逻辑删除迁移的实例"],
        )
        .unwrap();
    drop(connection);

    permanently_ignore_managed_instance(&database_path, 1).unwrap();

    assert!(list_managed_instances(&database_path).unwrap().is_empty());
    let connection = Connection::open(&database_path).unwrap();
    let (path, display_name, ignored_at): (String, Option<String>, Option<i64>) = connection
        .query_row(
            "SELECT path, display_name, ignored_at_unix FROM managed_instances WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(path, registered_path(&instance_directory));
    assert_eq!(display_name.as_deref(), Some("已有逻辑删除迁移的实例"));
    assert!(ignored_at.is_some());
}

#[test]
fn opening_a_missing_instance_path_is_rejected() {
    let dir = tempdir().unwrap();
    let instance_directory = dir.path().join("instance");
    let database_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&instance_directory);

    scan_and_register(&database_path, dir.path()).unwrap();
    let instance = list_managed_instances(&database_path)
        .unwrap()
        .pop()
        .unwrap();
    fs::remove_dir_all(&instance_directory).unwrap();

    assert!(managed_instance_path(&database_path, instance.id).is_err());
}

#[test]
fn scanning_a_non_directory_returns_an_error() {
    let dir = tempdir().unwrap();
    let database_path = dir.path().join("app-data").join("instances.sqlite");
    let file_path = dir.path().join("not-a-directory.txt");
    fs::write(&file_path, "not a directory").unwrap();

    assert!(scan_and_register(&database_path, &file_path).is_err());
}

#[cfg(windows)]
#[test]
fn scan_stores_windows_paths_without_extended_length_prefix() {
    let dir = tempdir().unwrap();
    let instance_directory = dir.path().join("instance");
    let database_path = dir.path().join("app-data").join("instances.sqlite");
    write_config(&instance_directory);

    scan_and_register(&database_path, dir.path()).unwrap();
    let instance = list_managed_instances(&database_path)
        .unwrap()
        .pop()
        .unwrap();

    assert!(!instance.path.starts_with(r"\\?\"));
    assert!(managed_instance_path(&database_path, instance.id)
        .unwrap()
        .is_dir());
}

fn wsl_registration(distribution: &str, host_prefix: &str) -> WslInstanceRegistration {
    WslInstanceRegistration {
        distribution: distribution.to_string(),
        user: "dev".to_string(),
        codex_home: "/home/dev/.codex".to_string(),
        host_path: format!(r"{host_prefix}\{distribution}\home\dev\.codex"),
        architecture: "x86_64".to_string(),
    }
}

#[test]
fn existing_registry_rows_migrate_to_native_runtime() {
    let dir = tempdir().unwrap();
    let instance_directory = dir.path().join("instance");
    let database_path = dir.path().join("instances.sqlite");
    write_config(&instance_directory);
    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute_batch(
            r#"
            CREATE TABLE managed_instances (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                display_name TEXT,
                added_at_unix INTEGER NOT NULL,
                last_seen_at_unix INTEGER NOT NULL
            );
            "#,
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO managed_instances (path, added_at_unix, last_seen_at_unix) VALUES (?1, 1, 2)",
            [registered_path(&instance_directory)],
        )
        .unwrap();
    drop(connection);

    let instance = list_managed_instances(&database_path)
        .unwrap()
        .pop()
        .unwrap();

    assert_eq!(instance.runtime, InstanceRuntime::Native);
    assert_eq!(instance.availability, InstanceAvailability::Available);
}

#[test]
fn registers_and_deduplicates_wsl_runtime_metadata() {
    let dir = tempdir().unwrap();
    let database_path = dir.path().join("instances.sqlite");
    let first = register_wsl_instance(
        &database_path,
        &wsl_registration("Ubuntu", r"\\wsl.localhost"),
    )
    .unwrap();
    let second = register_wsl_instance(
        &database_path,
        &wsl_registration("ubuntu", r"\\wsl.localhost"),
    )
    .unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(list_managed_instances(&database_path).unwrap().len(), 1);
    assert_eq!(
        list_managed_instances(&database_path)
            .unwrap()
            .pop()
            .unwrap()
            .availability,
        InstanceAvailability::Unknown
    );
    assert!(matches!(
        second.runtime,
        InstanceRuntime::Wsl {
            ref distribution,
            ref user,
            ref codex_home,
            ref architecture,
            ..
        } if distribution == "ubuntu"
            && user == "dev"
            && codex_home == "/home/dev/.codex"
            && architecture == "x86_64"
    ));
}

#[test]
fn successful_wsl_probe_upgrades_legacy_unc_record_and_keeps_alias() {
    let dir = tempdir().unwrap();
    let database_path = dir.path().join("instances.sqlite");
    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute_batch(
            r#"
            CREATE TABLE managed_instances (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                display_name TEXT,
                added_at_unix INTEGER NOT NULL,
                last_seen_at_unix INTEGER NOT NULL
            );
            INSERT INTO managed_instances (
                id, path, display_name, added_at_unix, last_seen_at_unix
            ) VALUES (
                7, '\\wsl$\Ubuntu\home\dev\.codex', '旧 WSL 实例', 1, 2
            );
            "#,
        )
        .unwrap();
    drop(connection);

    let before = list_managed_instances(&database_path)
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(before.availability, InstanceAvailability::Unavailable);
    assert!(before.availability_error.is_some());

    let upgraded = register_wsl_instance(
        &database_path,
        &wsl_registration("Ubuntu", r"\\wsl.localhost"),
    )
    .unwrap();

    assert_eq!(upgraded.id, 7);
    assert_eq!(upgraded.display_name.as_deref(), Some("旧 WSL 实例"));
    assert!(matches!(upgraded.runtime, InstanceRuntime::Wsl { .. }));
}

#[test]
fn treats_extended_wsl_unc_as_a_legacy_wsl_path() {
    assert_eq!(
        codex_session_manager::instance_registry::legacy_wsl_path(
            r"\\?\UNC\wsl.localhost\Ubuntu\home\dev\.codex",
        ),
        Some(("Ubuntu".to_string(), "/home/dev/.codex".to_string()))
    );
    assert!(codex_session_manager::wsl::is_wsl_mounted_path("/mnt"));
}

#[test]
fn rejects_wsl_instances_in_sync_plans() {
    let dir = tempdir().unwrap();
    let native_directory = dir.path().join("native");
    let database_path = dir.path().join("instances.sqlite");
    write_config(&native_directory);
    scan_and_register(&database_path, dir.path()).unwrap();
    let native = list_managed_instances(&database_path)
        .unwrap()
        .pop()
        .unwrap();
    let wsl = register_wsl_instance(
        &database_path,
        &wsl_registration("Ubuntu", r"\\wsl.localhost"),
    )
    .unwrap();

    let error = save_instance_sync_plan(
        &database_path,
        &InstanceSyncPlanDraft {
            id: None,
            name: "不支持的 WSL 同步".to_string(),
            source_instance_id: native.id,
            target_instance_ids: vec![wsl.id],
            config_paths: vec![vec!["model".to_string()]],
            project_selections: Vec::new(),
        },
    )
    .unwrap_err();

    assert!(format!("{error:?}").contains("WSL instances cannot be used"));
}
