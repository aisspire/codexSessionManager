use clap::CommandFactory;
use clap::Parser;
use codex_session_manager::cli::Cli;

#[test]
fn cli_does_not_expose_app_server_probe() {
    let command = Cli::command();

    assert!(
        command
            .get_subcommands()
            .all(|subcommand| subcommand.get_name() != "app-server-probe"),
        "app-server-probe should not be exposed after removing the probe feature"
    );
}

#[test]
fn cli_exposes_backup_restore_and_sync_commands() {
    let command = Cli::command();
    let names = command
        .get_subcommands()
        .map(|subcommand| subcommand.get_name())
        .collect::<Vec<_>>();

    assert!(names.contains(&"backup-list"));
    assert!(names.contains(&"backup-create"));
    assert!(names.contains(&"restore-backup"));
    assert!(names.contains(&"sync-database"));
    assert!(names.contains(&"compact-session"));
}

#[test]
fn cli_parses_write_commands_with_apply_flags() {
    Cli::try_parse_from([
        "codex-session-manager",
        "backup-create",
        "--id",
        "thread-1",
        "--trigger",
        "manual",
        "--apply",
    ])
    .unwrap();
    Cli::try_parse_from([
        "codex-session-manager",
        "restore-backup",
        "--backup-id",
        "sessions/thread-1/1-manual",
        "--apply",
    ])
    .unwrap();
    Cli::try_parse_from(["codex-session-manager", "sync-database", "--apply"]).unwrap();
    Cli::try_parse_from([
        "codex-session-manager",
        "compact-session",
        "--id",
        "thread-1",
        "--apply",
    ])
    .unwrap();
}
