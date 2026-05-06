use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::Result;
use toml_edit::DocumentMut;

use crate::path_map::apply_first_path_map;
use crate::profile::CodexProfile;
use crate::rollout::{read_all_rollout_meta, RolloutMeta};
use crate::session_index::{is_visible_user_thread, read_session_index};
use crate::state_db::{StateDb, ThreadRecord};

#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub thread_count: usize,
    pub rollout_meta_count: usize,
    pub session_index_count: usize,
    pub active_model: Option<String>,
    pub active_provider: Option<String>,
    pub provider_mismatches: usize,
    pub mapped_rollout_paths: usize,
    pub mapped_cwds: usize,
    pub missing_rollout_files: Vec<String>,
    pub meta_id_mismatches: Vec<String>,
    pub missing_index_entries: usize,
    pub warnings: Vec<String>,
}

impl ScanReport {
    pub fn to_text(&self) -> String {
        let mut lines = vec![
            "Codex session scan report".to_string(),
            format!("threads: {}", self.thread_count),
            format!("rollout metadata files: {}", self.rollout_meta_count),
            format!("session_index entries: {}", self.session_index_count),
            format!(
                "active provider: {}",
                self.active_provider.as_deref().unwrap_or("<unknown>")
            ),
            format!(
                "active model: {}",
                self.active_model.as_deref().unwrap_or("<unknown>")
            ),
            format!(
                "threads with non-active provider: {}",
                self.provider_mismatches
            ),
            format!(
                "rollout_path values matching path maps: {}",
                self.mapped_rollout_paths
            ),
            format!("cwd values matching path maps: {}", self.mapped_cwds),
            format!(
                "missing rollout files: {}",
                self.missing_rollout_files.len()
            ),
            format!(
                "JSONL/SQLite id mismatches: {}",
                self.meta_id_mismatches.len()
            ),
            format!(
                "visible user threads missing from session_index: {}",
                self.missing_index_entries
            ),
        ];

        if !self.warnings.is_empty() {
            lines.push("warnings:".to_string());
            lines.extend(self.warnings.iter().map(|warning| format!("- {warning}")));
        }

        lines.join("\n")
    }
}

pub fn scan_profile(profile: &CodexProfile) -> Result<ScanReport> {
    let mut report = ScanReport::default();
    let config = read_config_summary(&profile.config_path())?;
    report.active_model = profile.model.clone().or(config.model);
    report.active_provider = profile.provider.clone().or(config.provider);

    let db = StateDb::open(&profile.state_db_path())?;
    let threads = db.read_threads()?;
    let rollout_metas = read_all_rollout_meta(&profile.sessions_dir())?;
    let index_entries = read_session_index(&profile.session_index_path())?;

    report.thread_count = threads.len();
    report.rollout_meta_count = rollout_metas.len();
    report.session_index_count = index_entries.len();
    report.provider_mismatches =
        provider_mismatch_count(&threads, report.active_provider.as_deref());
    report.mapped_rollout_paths = mapped_field_count(
        threads
            .iter()
            .filter_map(|thread| thread.rollout_path.as_deref()),
        &profile.path_maps,
    );
    report.mapped_cwds = mapped_field_count(
        threads.iter().filter_map(|thread| thread.cwd.as_deref()),
        &profile.path_maps,
    );
    report.missing_rollout_files = missing_rollout_files(&threads);
    report.meta_id_mismatches = meta_id_mismatches(&threads, &rollout_metas);
    report.missing_index_entries = missing_index_count(&threads, &index_entries);

    if profile.path_maps.is_empty() {
        report
            .warnings
            .push("no path maps configured; path migration will not change anything".to_string());
    }
    if report.active_provider.is_none() {
        report
            .warnings
            .push("no active provider found in profile or config.toml".to_string());
    }

    Ok(report)
}

#[derive(Default)]
struct ConfigSummary {
    model: Option<String>,
    provider: Option<String>,
}

fn read_config_summary(path: &Path) -> Result<ConfigSummary> {
    if !path.exists() {
        return Ok(ConfigSummary::default());
    }

    let text = fs::read_to_string(path)?;
    let doc = text.parse::<DocumentMut>()?;
    Ok(ConfigSummary {
        model: doc["model"].as_str().map(ToOwned::to_owned),
        provider: doc["model_provider"].as_str().map(ToOwned::to_owned),
    })
}

fn provider_mismatch_count(threads: &[ThreadRecord], active_provider: Option<&str>) -> usize {
    let Some(active_provider) = active_provider else {
        return 0;
    };

    threads
        .iter()
        .filter(|thread| {
            thread
                .model_provider
                .as_deref()
                .is_some_and(|provider| provider != active_provider)
        })
        .count()
}

fn mapped_field_count<'a>(
    values: impl Iterator<Item = &'a str>,
    maps: &[crate::path_map::PathMap],
) -> usize {
    values
        .filter(|value| apply_first_path_map(value, maps).is_some())
        .count()
}

fn missing_rollout_files(threads: &[ThreadRecord]) -> Vec<String> {
    threads
        .iter()
        .filter_map(|thread| {
            let path = thread.rollout_path.as_ref()?;
            (!Path::new(path).exists()).then(|| format!("{} -> {}", thread.id, path))
        })
        .collect()
}

fn meta_id_mismatches(threads: &[ThreadRecord], metas: &[RolloutMeta]) -> Vec<String> {
    let by_path: HashMap<&Path, &RolloutMeta> = metas
        .iter()
        .map(|meta| (meta.path.as_path(), meta))
        .collect();

    threads
        .iter()
        .filter_map(|thread| {
            let rollout_path = thread.rollout_path.as_ref()?;
            let meta = by_path.get(Path::new(rollout_path))?;
            (meta.id.as_deref() != Some(thread.id.as_str())).then(|| {
                format!(
                    "{} -> JSONL id {}",
                    thread.id,
                    meta.id.as_deref().unwrap_or("<missing>")
                )
            })
        })
        .collect()
}

fn missing_index_count(
    threads: &[ThreadRecord],
    entries: &[crate::session_index::SessionIndexEntry],
) -> usize {
    let indexed: HashSet<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
    threads
        .iter()
        .filter(|thread| is_visible_user_thread(thread))
        .filter(|thread| !indexed.contains(thread.id.as_str()))
        .count()
}
