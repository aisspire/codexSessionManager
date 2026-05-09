use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::migrate::{self, ApplyOptions};
use crate::path_map::PathMap;
use crate::profile::CodexProfile;
use crate::scan;
use crate::session_list::{self, ArchivedFilter, SessionListFilter};
use crate::session_ops::{self, SessionApplyOptions};
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

    /// Validate SQLite integrity and JSONL/thread consistency.
    Validate,

    /// List sessions by project, provider, model, and archive state.
    List {
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        source: Option<String>,
        #[arg(long, default_value = "all")]
        archived: ArchivedArg,
        #[arg(long)]
        search: Option<String>,
    },

    /// Migrate SQLite and JSONL session metadata provider values.
    MigrateProvider {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        apply: bool,
    },

    /// Migrate rollout_path/cwd values using configured path maps.
    MigratePaths {
        #[arg(long)]
        apply: bool,
    },

    /// Append missing visible user sessions to session_index.jsonl.
    RepairSessionIndex {
        #[arg(long)]
        apply: bool,
    },

    /// Mark ordinary cli/vscode sessions with titles/messages as user sessions.
    RepairHasUserEvent {
        #[arg(long)]
        apply: bool,
    },

    /// Archive selected sessions.
    Archive {
        #[arg(long = "id", required = true)]
        ids: Vec<String>,
        #[arg(long)]
        apply: bool,
    },

    /// Mark selected archived sessions as active.
    Active {
        #[arg(long = "id", required = true)]
        ids: Vec<String>,
        #[arg(long)]
        apply: bool,
    },

    /// Move selected sessions to this tool's trash and archive their threads.
    Delete {
        #[arg(long = "id", required = true)]
        ids: Vec<String>,
        #[arg(long)]
        apply: bool,
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
        Command::Validate => {
            let report = validate::validate_profile(&profile)?;
            println!("{}", report.to_text());
            if !report.is_ok() {
                std::process::exit(2);
            }
        }
        Command::List {
            project,
            provider,
            model,
            source,
            archived,
            search,
        } => {
            let sessions = session_list::list_sessions(
                &profile,
                &SessionListFilter {
                    project: project.clone(),
                    provider: provider.clone(),
                    model: model.clone(),
                    source: source.clone(),
                    archived: (*archived).into(),
                    search: search.clone(),
                },
            )?;
            print_session_list(&sessions);
        }
        Command::MigrateProvider {
            from,
            to,
            apply,
        } => {
            let report = migrate::migrate_provider(
                &profile,
                from,
                to,
                &ApplyOptions { apply: *apply },
            )?;
            println!("{}", report.to_text());
        }
        Command::MigratePaths { apply } => {
            let report = migrate::migrate_paths(
                &profile,
                &ApplyOptions { apply: *apply },
            )?;
            println!("{}", report.to_text());
        }
        Command::RepairSessionIndex { apply } => {
            let report = migrate::repair_session_index(
                &profile,
                &ApplyOptions { apply: *apply },
            )?;
            println!("{}", report.to_text());
        }
        Command::RepairHasUserEvent { apply } => {
            let report = migrate::repair_has_user_event(
                &profile,
                &ApplyOptions { apply: *apply },
            )?;
            println!("{}", report.to_text());
        }
        Command::Archive { ids, apply } => {
            let report = session_ops::archive_sessions(
                &profile,
                ids,
                &SessionApplyOptions { apply: *apply },
            )?;
            println!("{}", report.to_text());
        }
        Command::Active { ids, apply } => {
            let report = session_ops::active_sessions(
                &profile,
                ids,
                &SessionApplyOptions { apply: *apply },
            )?;
            println!("{}", report.to_text());
        }
        Command::Delete { ids, apply } => {
            let report = session_ops::delete_sessions(
                &profile,
                ids,
                &SessionApplyOptions { apply: *apply },
            )?;
            println!("{}", report.to_text());
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ArchivedArg {
    Active,
    Archived,
    All,
}

impl From<ArchivedArg> for ArchivedFilter {
    fn from(value: ArchivedArg) -> Self {
        match value {
            ArchivedArg::Active => ArchivedFilter::Active,
            ArchivedArg::Archived => ArchivedFilter::Archived,
            ArchivedArg::All => ArchivedFilter::All,
        }
    }
}

fn print_session_list(sessions: &[session_list::SessionSummary]) {
    println!("sessions: {}", sessions.len());
    println!("updated_at\tarchived\tprovider\tmodel\tproject\ttitle\tid");
    for session in sessions {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            session.updated_at.as_deref().unwrap_or(""),
            session.archived,
            session.provider.as_deref().unwrap_or(""),
            session.model.as_deref().unwrap_or(""),
            session.project.as_deref().unwrap_or(""),
            session.title.as_deref().unwrap_or(""),
            session.id
        );
    }
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
