use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::backup;
use crate::migrate::{self, ApplyOptions};
use crate::path_map::PathMap;
use crate::profile::CodexProfile;
use crate::scan;
use crate::validate;

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Cli {
    /// Codex home directory, for example /mnt/c/Users/14139/.codex.
    #[arg(long, global = true, default_value = "~/.codex")]
    pub codex_home: String,

    /// Human-readable profile name used in reports.
    #[arg(long, global = true, default_value = "default")]
    pub profile: String,

    /// Active model provider key, for example cm or openai.
    #[arg(long, global = true)]
    pub provider: Option<String>,

    /// Active model name, for example gpt-5.5.
    #[arg(long, global = true)]
    pub model: Option<String>,

    /// Explicit path conversion rule in FROM=TO format. Repeatable.
    #[arg(long = "path-map", global = true)]
    pub path_maps: Vec<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Scan Codex files and print a read-only report.
    Scan,

    /// Back up key Codex state files.
    Backup {
        /// Include sessions/ in the backup. This can be large.
        #[arg(long)]
        include_sessions: bool,
    },

    /// Validate SQLite integrity and JSONL/thread consistency.
    Validate,

    /// Migrate SQLite and JSONL session metadata provider values.
    MigrateProvider {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        no_backup: bool,
        #[arg(long)]
        include_sessions_backup: bool,
    },

    /// Migrate rollout_path/cwd values using configured path maps.
    MigratePaths {
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        no_backup: bool,
        #[arg(long)]
        include_sessions_backup: bool,
    },

    /// Append missing visible user sessions to session_index.jsonl.
    RepairSessionIndex {
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        no_backup: bool,
    },

    /// Mark ordinary cli/vscode sessions with titles/messages as user sessions.
    RepairHasUserEvent {
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        no_backup: bool,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let profile = build_profile(&cli)?;

    match &cli.command {
        Command::Scan => {
            let report = scan::scan_profile(&profile)?;
            println!("{}", report.to_text());
        }
        Command::Backup { include_sessions } => {
            let result = backup::create_backup(&profile, *include_sessions)?;
            println!("backup: {}", result.backup_dir.display());
            println!("copied entries: {}", result.copied_files.len());
        }
        Command::Validate => {
            let report = validate::validate_profile(&profile)?;
            println!("{}", report.to_text());
            if !report.is_ok() {
                std::process::exit(2);
            }
        }
        Command::MigrateProvider {
            from,
            to,
            apply,
            no_backup,
            include_sessions_backup,
        } => {
            let report = migrate::migrate_provider(
                &profile,
                from,
                to,
                &ApplyOptions {
                    apply: *apply,
                    backup: !no_backup,
                    include_sessions_backup: *include_sessions_backup,
                },
            )?;
            println!("{}", report.to_text());
        }
        Command::MigratePaths {
            apply,
            no_backup,
            include_sessions_backup,
        } => {
            let report = migrate::migrate_paths(
                &profile,
                &ApplyOptions {
                    apply: *apply,
                    backup: !no_backup,
                    include_sessions_backup: *include_sessions_backup,
                },
            )?;
            println!("{}", report.to_text());
        }
        Command::RepairSessionIndex { apply, no_backup } => {
            let report = migrate::repair_session_index(
                &profile,
                &ApplyOptions {
                    apply: *apply,
                    backup: !no_backup,
                    include_sessions_backup: false,
                },
            )?;
            println!("{}", report.to_text());
        }
        Command::RepairHasUserEvent { apply, no_backup } => {
            let report = migrate::repair_has_user_event(
                &profile,
                &ApplyOptions {
                    apply: *apply,
                    backup: !no_backup,
                    include_sessions_backup: false,
                },
            )?;
            println!("{}", report.to_text());
        }
    }

    Ok(())
}

fn build_profile(cli: &Cli) -> Result<CodexProfile> {
    let codex_home = expand_home(&cli.codex_home);
    let path_maps = cli
        .path_maps
        .iter()
        .map(|spec| PathMap::parse(spec))
        .collect::<Result<Vec<_>>>()?;

    CodexProfile::new(
        cli.profile.clone(),
        codex_home,
        cli.provider.clone(),
        cli.model.clone(),
        path_maps,
    )
}

fn expand_home(value: &str) -> PathBuf {
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(value)
}
