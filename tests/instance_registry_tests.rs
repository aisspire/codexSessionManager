use std::fs;
use std::path::Path;

use codex_session_manager::instance_registry::{
    list_managed_instances, managed_instance_path, rename_managed_instance, scan_and_register,
};
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
    assert!(instances.iter().all(|instance| instance.available));
    assert!(instances
        .iter()
        .any(|instance| instance.path == registered_path(&first_instance)));
    assert!(instances
        .iter()
        .any(|instance| instance.path == registered_path(&second_instance)));
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
    assert!(!persisted.available);
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
