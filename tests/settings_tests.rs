use std::fs;

use codex_session_manager::profile::CodexProfile;
use codex_session_manager::settings::{
    load_settings, save_settings, settings_path, AppSettings, DatabaseSyncMode,
};
use tempfile::tempdir;

#[test]
fn missing_settings_file_returns_defaults() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();

    let settings = load_settings(&profile).unwrap();

    assert_eq!(settings, AppSettings::default());
    assert_eq!(settings.backup.minimum_free_bytes, 536_870_912);
}

#[test]
fn corrupt_settings_json_returns_error() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let path = settings_path(&profile);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "{not json").unwrap();

    let error = load_settings(&profile).unwrap_err();

    assert!(error.to_string().contains("settings"));
}

#[test]
fn save_settings_creates_project_owned_settings_file() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let mut settings = AppSettings::default();
    settings.backup.max_count = Some(7);
    settings.database_sync.mode = DatabaseSyncMode::AutoWhenCodexStops;

    save_settings(&profile, &settings).unwrap();

    let path = settings_path(&profile);
    assert_eq!(
        path,
        profile
            .codex_home
            .join("codex-session-manager")
            .join("settings.json")
    );
    assert!(path.exists());
    assert_eq!(load_settings(&profile).unwrap(), settings);
}

#[test]
fn partial_json_fills_missing_fields_with_defaults() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let path = settings_path(&profile);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        r#"{
  "backup": {
    "max_count": 3
  }
}"#,
    )
    .unwrap();

    let settings = load_settings(&profile).unwrap();

    assert_eq!(settings.backup.max_count, Some(3));
    assert_eq!(settings.backup.max_bytes, None);
    assert_eq!(settings.backup.max_age_days, None);
    assert!(settings.backup.skip_unique_archive_on_auto_prune);
    assert_eq!(settings.backup.minimum_free_bytes, 536_870_912);
    assert_eq!(settings.database_sync.mode, DatabaseSyncMode::Never);
}
