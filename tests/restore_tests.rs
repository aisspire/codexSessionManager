use std::fs;

use codex_session_manager::backup::create_backup;
use codex_session_manager::profile::CodexProfile;
use codex_session_manager::restore::restore_from_manifest_with_guard;
use tempfile::tempdir;

#[test]
fn restores_files_from_backup_manifest() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    fs::write(profile.config_path(), "model = \"old\"\n").unwrap();
    fs::write(profile.session_index_path(), "{\"id\":\"thread-1\"}\n").unwrap();
    let backup = create_backup(&profile, false).unwrap();

    fs::write(profile.config_path(), "model = \"new\"\n").unwrap();

    let report =
        restore_from_manifest_with_guard(&backup.manifest_path.unwrap(), &[], true, || Ok(()))
            .unwrap();

    assert!(report.applied);
    assert_eq!(report.files_restored, 2);
    assert_eq!(
        fs::read_to_string(profile.config_path()).unwrap(),
        "model = \"old\"\n"
    );
}

#[test]
fn refuses_manifest_restore_when_codex_is_running() {
    let dir = tempdir().unwrap();
    let profile = CodexProfile::new("test", dir.path(), None, None, Vec::new()).unwrap();
    fs::write(profile.config_path(), "model = \"old\"\n").unwrap();
    let backup = create_backup(&profile, false).unwrap();
    fs::write(profile.config_path(), "model = \"new\"\n").unwrap();

    let result =
        restore_from_manifest_with_guard(&backup.manifest_path.unwrap(), &[], true, || {
            anyhow::bail!("Codex appears to be running")
        });

    assert!(result.is_err());
    assert_eq!(
        fs::read_to_string(profile.config_path()).unwrap(),
        "model = \"new\"\n"
    );
}
