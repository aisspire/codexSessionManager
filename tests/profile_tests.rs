use std::path::PathBuf;

use codex_session_manager::profile::CodexProfile;

#[test]
fn expands_tilde_codex_home_for_profile_paths() {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .expect("test requires HOME or USERPROFILE");

    let profile = CodexProfile::new("default", "~/.codex", None, None, Vec::new()).unwrap();

    assert_eq!(profile.codex_home, home.join(".codex"));
    assert_eq!(profile.state_db_path(), home.join(".codex").join("state_5.sqlite"));
}
