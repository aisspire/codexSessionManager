use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::profile::CodexProfile;
use crate::safety;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactOptions {
    pub apply: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactReport {
    pub action: String,
    pub applied: bool,
    pub session_id: String,
    pub backup_manifest: String,
    pub command: Vec<String>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CompactReport {
    pub fn to_text(&self) -> String {
        let mut lines = vec![
            format!("action: {}", self.action),
            format!("mode: {}", if self.applied { "applied" } else { "dry-run" }),
            format!("session id: {}", self.session_id),
        ];
        if !self.command.is_empty() {
            lines.push(format!("command: {}", self.command.join(" ")));
        }
        if !self.backup_manifest.is_empty() {
            lines.push(format!("backup manifest: {}", self.backup_manifest));
        }
        if let Some(exit_code) = self.exit_code {
            lines.push(format!("exit code: {exit_code}"));
        }
        lines.join("\n")
    }
}

pub fn compact_session(
    profile: &CodexProfile,
    session_id: &str,
    options: &CompactOptions,
) -> Result<CompactReport> {
    compact_session_with_guard_and_runner(
        profile,
        session_id,
        options,
        safety::ensure_codex_not_running,
        || Ok(()),
    )
}

pub fn compact_session_with_guard_and_runner<G, R>(
    _profile: &CodexProfile,
    session_id: &str,
    options: &CompactOptions,
    _guard: G,
    _runner: R,
) -> Result<CompactReport>
where
    G: FnOnce() -> Result<()>,
    R: FnOnce() -> Result<()>,
{
    let report = CompactReport {
        action: "compact session context".to_string(),
        applied: options.apply,
        session_id: session_id.to_string(),
        backup_manifest: String::new(),
        command: Vec::new(),
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
    };

    if !options.apply {
        return Ok(report);
    }

    bail!(
        "cannot compact {session_id}: Codex CLI does not expose a non-interactive compact command. /compact must be run inside an interactive Codex session. Open the target session in Codex from its project directory, then run /compact manually."
    )
}
