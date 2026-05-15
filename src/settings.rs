use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::profile::CodexProfile;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub backup: BackupSettings,
    pub database_sync: DatabaseSyncSettings,
    pub codex_cli: CodexCliSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackupSettings {
    pub max_bytes: Option<u64>,
    pub max_age_days: Option<u64>,
    pub max_count: Option<usize>,
    pub skip_unique_archive_on_auto_prune: bool,
    pub minimum_free_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DatabaseSyncSettings {
    pub mode: DatabaseSyncMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CodexCliSettings {
    pub command_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DatabaseSyncMode {
    Never,
    AutoWhenCodexStops,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            backup: BackupSettings::default(),
            database_sync: DatabaseSyncSettings::default(),
            codex_cli: CodexCliSettings::default(),
        }
    }
}

impl Default for BackupSettings {
    fn default() -> Self {
        Self {
            max_bytes: None,
            max_age_days: None,
            max_count: None,
            skip_unique_archive_on_auto_prune: true,
            minimum_free_bytes: 536_870_912,
        }
    }
}

impl Default for DatabaseSyncSettings {
    fn default() -> Self {
        Self {
            mode: DatabaseSyncMode::Never,
        }
    }
}

impl Default for DatabaseSyncMode {
    fn default() -> Self {
        Self::Never
    }
}

pub fn settings_path(profile: &CodexProfile) -> PathBuf {
    profile
        .codex_home
        .join("codex-session-manager")
        .join("settings.json")
}

pub fn load_settings(profile: &CodexProfile) -> Result<AppSettings> {
    let path = settings_path(profile);
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read settings file {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("failed to parse settings file {}", path.display()))
}

pub fn save_settings(profile: &CodexProfile, settings: &AppSettings) -> Result<()> {
    let path = settings_path(profile);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create settings directory {}", parent.display()))?;
    }
    let content = serde_json::to_string_pretty(settings).context("failed to serialize settings")?;
    fs::write(&path, content)
        .with_context(|| format!("failed to write settings file {}", path.display()))
}
