use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use walkdir::WalkDir;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RolloutMeta {
    pub path: PathBuf,
    pub id: Option<String>,
    pub cwd: Option<String>,
    pub source: Option<String>,
    pub model_provider: Option<String>,
    pub cli_version: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct SessionMetaPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli_version: Option<String>,

    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

impl<'de> Deserialize<'de> for SessionMetaPayload {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut values = serde_json::Map::<String, Value>::deserialize(deserializer)?;
        let id = take_optional_string(&mut values, "id")?;
        let cwd = take_optional_string(&mut values, "cwd")?;
        let source = take_source(&mut values);
        let model_provider = take_optional_string(&mut values, "model_provider")?;
        let cli_version = take_optional_string(&mut values, "cli_version")?;

        Ok(Self {
            id,
            cwd,
            source,
            model_provider,
            cli_version,
            extra: values,
        })
    }
}

fn take_optional_string<E>(
    values: &mut serde_json::Map<String, Value>,
    key: &'static str,
) -> std::result::Result<Option<String>, E>
where
    E: de::Error,
{
    match values.remove(key) {
        Some(Value::String(value)) => Ok(Some(value)),
        Some(Value::Null) | None => Ok(None),
        Some(value) => Err(E::custom(format!(
            "invalid `{key}` value: expected string or null, got {value}"
        ))),
    }
}

fn take_source(values: &mut serde_json::Map<String, Value>) -> Option<String> {
    match values.remove("source") {
        Some(Value::String(value)) => Some(value),
        Some(value) => {
            values.insert("source".to_string(), value);
            None
        }
        None => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionMetaLine {
    #[serde(rename = "type")]
    event_type: String,
    payload: SessionMetaPayload,

    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
}

pub fn discover_rollout_files(sessions_dir: &Path) -> Result<Vec<PathBuf>> {
    if !sessions_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(sessions_dir) {
        let entry = entry?;
        if entry.file_type().is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "jsonl")
        {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();
    Ok(files)
}

pub fn read_rollout_meta(path: &Path) -> Result<Option<RolloutMeta>> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open rollout file {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut first_line = String::new();
    if reader
        .read_line(&mut first_line)
        .with_context(|| format!("failed to read first line in {}", path.display()))?
        == 0
    {
        return Ok(None);
    }
    let first_line = first_line.trim_end_matches(['\r', '\n']);

    let line: SessionMetaLine = serde_json::from_str(first_line)
        .with_context(|| format!("failed to parse first JSONL line in {}", path.display()))?;
    if line.event_type != "session_meta" {
        return Ok(None);
    }

    Ok(Some(RolloutMeta {
        path: path.to_path_buf(),
        id: line.payload.id,
        cwd: line.payload.cwd,
        source: line.payload.source,
        model_provider: line.payload.model_provider,
        cli_version: line.payload.cli_version,
    }))
}

pub fn read_all_rollout_meta(sessions_dir: &Path) -> Result<Vec<RolloutMeta>> {
    let mut metas = Vec::new();
    for file in discover_rollout_files(sessions_dir)? {
        if let Some(meta) = read_rollout_meta(&file)? {
            metas.push(meta);
        }
    }
    Ok(metas)
}

/// Rewrites only the first JSONL line when it is a `session_meta` event.
///
/// The closure receives the typed payload and returns whether it changed the
/// payload. All later lines are copied unchanged, which avoids rewriting
/// historical command output or user messages that may contain old paths.
pub fn rewrite_session_meta<F>(path: &Path, update: F) -> Result<bool>
where
    F: FnOnce(&mut SessionMetaPayload) -> bool,
{
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read rollout file {}", path.display()))?;
    let Some(line_end) = text.find('\n') else {
        return rewrite_single_line_session_meta(path, &text, update);
    };

    let first_line = &text[..line_end];
    let rest = &text[line_end..];
    let mut line: SessionMetaLine = serde_json::from_str(first_line)
        .with_context(|| format!("failed to parse first JSONL line in {}", path.display()))?;

    if line.event_type != "session_meta" || !update(&mut line.payload) {
        return Ok(false);
    }

    let new_first_line = serde_json::to_string(&line)?;
    atomic_write(path, format!("{new_first_line}{rest}").as_bytes())?;
    Ok(true)
}

fn rewrite_single_line_session_meta<F>(path: &Path, text: &str, update: F) -> Result<bool>
where
    F: FnOnce(&mut SessionMetaPayload) -> bool,
{
    let mut line: SessionMetaLine = serde_json::from_str(text)
        .with_context(|| format!("failed to parse first JSONL line in {}", path.display()))?;

    if line.event_type != "session_meta" || !update(&mut line.payload) {
        return Ok(false);
    }

    let new_first_line = serde_json::to_string(&line)?;
    atomic_write(path, new_first_line.as_bytes())?;
    Ok(true)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp_path = path.with_extension("jsonl.tmp");
    {
        let mut file = fs::File::create(&tmp_path)
            .with_context(|| format!("failed to create temp file {}", tmp_path.display()))?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "failed to replace {} with {}",
            path.display(),
            tmp_path.display()
        )
    })?;
    Ok(())
}
