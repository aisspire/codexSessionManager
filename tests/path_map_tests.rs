use codex_session_manager::path_map::{path_buf_for_current_os, PathMap};

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

#[cfg(windows)]
#[test]
fn converts_wsl_mount_path_to_windows_path_on_windows() {
    assert_eq!(
        path_buf_for_current_os("/mnt/c/Users/14139/.codex/sessions/thread.jsonl")
            .display()
            .to_string(),
        r"C:\Users\14139\.codex\sessions\thread.jsonl"
    );
}
