use std::path::PathBuf;

use anyhow::{bail, Result};

use crate::path_map::PathMap;

/// Runtime description for one Codex data directory.
///
/// Profiles make dangerous migration context explicit: which Codex home is
/// being touched, which provider/model is active, and which path conversions
/// are allowed.
#[derive(Debug, Clone)]
pub struct CodexProfile {
    pub name: String,
    pub codex_home: PathBuf,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub path_maps: Vec<PathMap>,
}

impl CodexProfile {
    pub fn new(
        name: impl Into<String>,
        codex_home: impl Into<PathBuf>,
        provider: Option<String>,
        model: Option<String>,
        path_maps: Vec<PathMap>,
    ) -> Result<Self> {
        let name = name.into();
        let codex_home = expand_home(codex_home.into());

        if name.trim().is_empty() {
            bail!("profile name cannot be empty");
        }
        if codex_home.as_os_str().is_empty() {
            bail!("codex home cannot be empty");
        }

        Ok(Self {
            name,
            codex_home,
            provider,
            model,
            path_maps,
        })
    }

    pub fn state_db_path(&self) -> PathBuf {
        self.codex_home.join("state_5.sqlite")
    }

    pub fn session_index_path(&self) -> PathBuf {
        self.codex_home.join("session_index.jsonl")
    }

    pub fn config_path(&self) -> PathBuf {
        self.codex_home.join("config.toml")
    }

    pub fn global_state_path(&self) -> PathBuf {
        self.codex_home.join(".codex-global-state.json")
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.codex_home.join("sessions")
    }

    pub fn archived_sessions_dir(&self) -> PathBuf {
        self.codex_home.join("archived_sessions")
    }
}

fn expand_home(path: PathBuf) -> PathBuf {
    let Some(value) = path.to_str() else {
        return path;
    };
    let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) else {
        return path;
    };
    let home = PathBuf::from(home);

    if value == "~" {
        return home;
    }
    if let Some(rest) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        return home.join(rest);
    }

    path
}
