use codex_session_manager::path_map::PathMap;

#[test]
fn maps_windows_drive_prefix_to_wsl_prefix() {
    let map = PathMap::new(r"E:\code", "/mnt/e/code").unwrap();

    let migrated = map.apply(r"E:\code\upmqtt\.codex\session.jsonl");

    assert_eq!(
        migrated.as_deref(),
        Some("/mnt/e/code/upmqtt/.codex/session.jsonl")
    );
}

#[test]
fn maps_extended_length_windows_prefix() {
    let map = PathMap::new(r"\\?\E:\code", "/mnt/e/code").unwrap();

    let migrated = map.apply(r"\\?\E:\code\demo");

    assert_eq!(migrated.as_deref(), Some("/mnt/e/code/demo"));
}

#[test]
fn leaves_unmatched_path_unchanged() {
    let map = PathMap::new(r"E:\code", "/mnt/e/code").unwrap();

    assert_eq!(map.apply(r"D:\other\project"), None);
}
