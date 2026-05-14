use std::fs;

use codex_session_manager::favorites::{
    favorite_ids, favorites_path, read_favorites, set_favorite, toggle_favorite,
};
use codex_session_manager::profile::CodexProfile;
use tempfile::tempdir;

#[test]
fn missing_favorites_file_returns_empty_file() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();

    let favorites = read_favorites(&profile).unwrap();

    assert_eq!(favorites.version, 1);
    assert!(favorites.favorites.is_empty());
    assert!(favorite_ids(&profile).unwrap().is_empty());
}

#[test]
fn toggle_favorite_adds_and_removes_session_id() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();

    let after_add = toggle_favorite(&profile, "session-1").unwrap();
    assert_eq!(after_add.favorites.len(), 1);
    assert_eq!(after_add.favorites[0].session_id, "session-1");
    assert!(after_add.favorites[0].starred_at_unix > 0);

    let after_remove = toggle_favorite(&profile, "session-1").unwrap();
    assert!(after_remove.favorites.is_empty());
}

#[test]
fn duplicate_ids_are_normalized_when_reading() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    let path = favorites_path(&profile);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        r#"{
  "version": 1,
  "favorites": [
    { "session_id": "session-1", "starred_at_unix": 10 },
    { "session_id": "session-1", "starred_at_unix": 20 },
    { "session_id": "session-2", "starred_at_unix": 30 }
  ]
}"#,
    )
    .unwrap();

    let favorites = read_favorites(&profile).unwrap();

    assert_eq!(favorites.favorites.len(), 2);
    assert_eq!(favorites.favorites[0].session_id, "session-1");
    assert_eq!(favorites.favorites[0].starred_at_unix, 10);
    assert_eq!(favorites.favorites[1].session_id, "session-2");
}

#[test]
fn saving_creates_project_owned_favorites_file() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();

    set_favorite(&profile, "session-1", true).unwrap();

    let path = favorites_path(&profile);
    assert_eq!(
        path,
        profile
            .codex_home
            .join("codex-session-manager")
            .join("favorites.json")
    );
    assert!(path.exists());
    assert!(favorite_ids(&profile).unwrap().contains("session-1"));
}
