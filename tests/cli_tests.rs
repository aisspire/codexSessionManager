use clap::CommandFactory;
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
