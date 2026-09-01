use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use codex_session_manager::instance_registry::canonical_wsl_host_path;
use codex_session_manager::profile_operation::{
    BridgeIdentity, BridgeRequest, BridgeResponse, ProfileOperation, WSL_BRIDGE_PROTOCOL_VERSION,
    WSL_BRIDGE_RESPONSE_MARKER,
};
use codex_session_manager::wsl::{
    normalize_architecture, parse_wsl_list_output, validate_distribution,
    validate_linux_absolute_path, validate_user,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::Manager;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::timeout;

const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const PROBE_MARKER: &str = "__CSM_WSL_PROBE__";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BridgeFailureKind {
    ProtocolRejected,
    ResponseProtocolRejected,
    DeploymentRejected,
    MissingResponseMarker,
    OperationFailed,
    ResponseDecodeFailed,
    Timeout,
    OperationResultUnknown,
}

#[derive(Debug)]
struct BridgeFailure {
    kind: BridgeFailureKind,
    message: String,
}

impl Display for BridgeFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for BridgeFailure {}

#[derive(Debug)]
struct WslCommandTimeout {
    stage: String,
    duration: Duration,
}

impl Display for WslCommandTimeout {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "WSL command timed out during {} after {} ms; child process was terminated",
            self.stage,
            self.duration.as_millis()
        )
    }
}

impl Error for WslCommandTimeout {}

fn classify_command_error(error: anyhow::Error) -> anyhow::Error {
    if error
        .chain()
        .any(|cause| cause.downcast_ref::<WslCommandTimeout>().is_some())
    {
        return bridge_failure(BridgeFailureKind::Timeout, format!("{error:#}"));
    }
    error
}

fn bridge_failure(kind: BridgeFailureKind, message: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(BridgeFailure {
        kind,
        message: message.into(),
    })
}

fn bridge_failure_kind(error: &anyhow::Error) -> Option<BridgeFailureKind> {
    error.chain().find_map(|cause| {
        cause
            .downcast_ref::<BridgeFailure>()
            .map(|failure| failure.kind)
    })
}

fn classify_deployment_error(error: anyhow::Error) -> anyhow::Error {
    if bridge_failure_kind(&error).is_some() {
        return error;
    }
    bridge_failure(
        BridgeFailureKind::DeploymentRejected,
        format!("WSL helper deployment was rejected before the profile operation: {error:#}"),
    )
}

fn should_retry_after_failure(operation: &ProfileOperation, error: &anyhow::Error) -> bool {
    match bridge_failure_kind(error) {
        Some(BridgeFailureKind::ProtocolRejected | BridgeFailureKind::DeploymentRejected) => true,
        Some(BridgeFailureKind::MissingResponseMarker) => {
            operation.can_retry_after_missing_response_marker()
        }
        _ => false,
    }
}

fn preserve_ambiguous_write_result(
    operation: &ProfileOperation,
    error: anyhow::Error,
) -> anyhow::Error {
    if bridge_failure_kind(&error) == Some(BridgeFailureKind::MissingResponseMarker)
        && !operation.can_retry_after_missing_response_marker()
    {
        return bridge_failure(
            BridgeFailureKind::OperationResultUnknown,
            format!(
                "WSL helper operation result is unknown; not retried to avoid duplicate writes: {error:#}"
            ),
        );
    }
    error
}

async fn invoke_with_recovery<Invoke, InvokeFuture, Redeploy, RedeployFuture>(
    operation: &ProfileOperation,
    mut invoke: Invoke,
    mut redeploy: Redeploy,
) -> Result<serde_json::Value>
where
    Invoke: FnMut() -> InvokeFuture,
    InvokeFuture: Future<Output = Result<serde_json::Value>>,
    Redeploy: FnMut() -> RedeployFuture,
    RedeployFuture: Future<Output = Result<()>>,
{
    let first = invoke().await;
    match first {
        Err(error) if should_retry_after_failure(operation, &error) => {
            redeploy().await?;
            match invoke().await {
                Err(error) => Err(preserve_ambiguous_write_result(operation, error)),
                Ok(result) => Ok(result),
            }
        }
        Err(error) => Err(preserve_ambiguous_write_result(operation, error)),
        Ok(result) => Ok(result),
    }
}

const PROBE_SCRIPT: &str = r#"set -eu
requested_home=$1
actual_user=$(id -un)
passwd_entry=$(getent passwd "$actual_user" 2>/dev/null || true)
if [ -z "$passwd_entry" ] && [ -r /etc/passwd ]; then
  passwd_entry=$(awk -F: -v requested_user="$actual_user" '$1 == requested_user { print; exit }' /etc/passwd)
fi
actual_home=$(printf '%s\n' "$passwd_entry" | awk -F: 'NF >= 7 { print $6; exit }')
if [ -z "$actual_home" ]; then
  actual_home=${HOME:?}
fi
architecture=$(uname -m)
login_shell=$(printf '%s\n' "$passwd_entry" | awk -F: 'NF >= 7 { print $7; exit }')
case "$login_shell" in
  /*) ;;
  *) login_shell=/bin/sh ;;
esac
if [ ! -x "$login_shell" ]; then
  login_shell=/bin/sh
fi
if [ "$requested_home" = "__DEFAULT__" ]; then
  codex_home="$actual_home/.codex"
else
  codex_home=$requested_home
fi
if [ ! -f "$codex_home/config.toml" ]; then
  printf '%s%s\t%s\t%s\t%s\tmissing\t\n' '__CSM_WSL_PROBE__' "$actual_user" "$actual_home" "$architecture" "$codex_home"
  exit 44
fi
codex_cli=$("$login_shell" -lc 'command -v codex 2>/dev/null || true' 2>/dev/null | tail -n 1 | tr -d '\r' || true)
printf '%s%s\t%s\t%s\t%s\tavailable\t%s\n' '__CSM_WSL_PROBE__' "$actual_user" "$actual_home" "$architecture" "$codex_home" "$codex_cli""#;
const LOGIN_BRIDGE_SCRIPT: &str = r#"set -eu
app_version=$1
architecture=$2
actual_user=$(id -un)
passwd_entry=$(getent passwd "$actual_user" 2>/dev/null || true)
if [ -z "$passwd_entry" ] && [ -r /etc/passwd ]; then
  passwd_entry=$(awk -F: -v requested_user="$actual_user" '$1 == requested_user { print; exit }' /etc/passwd)
fi
user_home=$(printf '%s\n' "$passwd_entry" | awk -F: 'NF >= 7 { print $6; exit }')
user_home=${user_home:-${HOME:?}}
helper="$user_home/.cache/codex-session-manager/$app_version/$architecture/codex-session-manager-wsl-bridge"
login_shell=$(printf '%s\n' "$passwd_entry" | awk -F: 'NF >= 7 { print $7; exit }')
case "$login_shell" in
  /*) ;;
  *) login_shell=/bin/sh ;;
esac
if [ ! -x "$login_shell" ]; then
  login_shell=/bin/sh
fi
export CSM_WSL_HELPER_PATH="$helper"
exec "$login_shell" -lc 'exec "$CSM_WSL_HELPER_PATH"'"#;
const LOGIN_IDENTITY_SCRIPT: &str = r#"set -eu
app_version=$1
architecture=$2
actual_user=$(id -un)
passwd_entry=$(getent passwd "$actual_user" 2>/dev/null || true)
if [ -z "$passwd_entry" ] && [ -r /etc/passwd ]; then
  passwd_entry=$(awk -F: -v requested_user="$actual_user" '$1 == requested_user { print; exit }' /etc/passwd)
fi
user_home=$(printf '%s\n' "$passwd_entry" | awk -F: 'NF >= 7 { print $6; exit }')
user_home=${user_home:-${HOME:?}}
helper="$user_home/.cache/codex-session-manager/$app_version/$architecture/codex-session-manager-wsl-bridge"
login_shell=$(printf '%s\n' "$passwd_entry" | awk -F: 'NF >= 7 { print $7; exit }')
case "$login_shell" in
  /*) ;;
  *) login_shell=/bin/sh ;;
esac
if [ ! -x "$login_shell" ]; then
  login_shell=/bin/sh
fi
export CSM_WSL_HELPER_PATH="$helper"
exec "$login_shell" -lc 'exec "$CSM_WSL_HELPER_PATH" --identity'"#;
const DEPLOY_SCRIPT: &str = r#"set -eu
app_version=$1
architecture=$2
actual_user=$(id -un)
passwd_entry=$(getent passwd "$actual_user" 2>/dev/null || true)
if [ -z "$passwd_entry" ] && [ -r /etc/passwd ]; then
  passwd_entry=$(awk -F: -v requested_user="$actual_user" '$1 == requested_user { print; exit }' /etc/passwd)
fi
user_home=$(printf '%s\n' "$passwd_entry" | awk -F: 'NF >= 7 { print $6; exit }')
user_home=${user_home:-${HOME:?}}
cache_dir="$user_home/.cache/codex-session-manager/$app_version/$architecture"
helper_path="$cache_dir/codex-session-manager-wsl-bridge"
umask 077
mkdir -p "$cache_dir"
temporary_path="$helper_path.tmp.$$"
trap 'rm -f "$temporary_path"' EXIT HUP INT TERM
cat > "$temporary_path"
chmod 700 "$temporary_path"
mv -f "$temporary_path" "$helper_path"
trap - EXIT HUP INT TERM"#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WslStatus {
    pub supported: bool,
    pub installed: bool,
    pub distributions: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WslProbe {
    pub distribution: String,
    pub user: String,
    pub home: String,
    pub codex_home: String,
    pub host_path: String,
    pub architecture: String,
    pub codex_cli: Option<String>,
    pub available: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WslDiscoveryError {
    pub distribution: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WslDiscoveryReport {
    pub instances: Vec<WslProbe>,
    pub errors: Vec<WslDiscoveryError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct WslRegistrationInput {
    pub distribution: String,
    pub user: Option<String>,
    pub codex_home: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct VerifiedHelperKey {
    distribution: String,
    user: String,
    architecture: String,
    app_version: String,
    bundled_helper_fingerprint: String,
}

#[derive(Default)]
pub struct WslBridgeManager {
    locks: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
    verified: Mutex<HashSet<VerifiedHelperKey>>,
}

impl WslBridgeManager {
    pub async fn invoke(
        &self,
        app: &tauri::AppHandle,
        distribution: &str,
        user: &str,
        architecture: &str,
        request: &BridgeRequest,
    ) -> Result<serde_json::Value> {
        validate_distribution(distribution)?;
        validate_user(user)?;
        let architecture = normalize_architecture(architecture)?;
        let source_path = bundled_helper_path(app, &architecture)?;
        let bundled_helper_fingerprint = bundled_helper_fingerprint(&source_path).await?;
        let app_version = APP_VERSION.to_string();
        let cache_key = VerifiedHelperKey {
            distribution: distribution.to_ascii_lowercase(),
            user: user.to_string(),
            architecture: architecture.clone(),
            app_version: app_version.clone(),
            bundled_helper_fingerprint,
        };
        let lock_key = format!(
            "{}\0{}\0{}",
            distribution.to_ascii_lowercase(),
            user,
            architecture
        );
        let lock = {
            let mut locks = self
                .locks
                .lock()
                .map_err(|_| anyhow::anyhow!("WSL bridge lock registry is poisoned"))?;
            locks
                .entry(lock_key)
                .or_insert_with(|| Arc::new(AsyncMutex::new(())))
                .clone()
        };
        let _guard = lock.lock().await;
        let expected_identity = expected_identity(&architecture)?;
        let identity = match helper_identity(distribution, user, &architecture).await {
            Ok(identity) => Some(identity),
            Err(error)
                if matches!(
                    bridge_failure_kind(&error),
                    Some(
                        BridgeFailureKind::ProtocolRejected | BridgeFailureKind::DeploymentRejected
                    )
                ) =>
            {
                None
            }
            Err(error) => return Err(error),
        };
        let already_verified = identity
            .as_ref()
            .is_some_and(|identity| identity == &expected_identity)
            && self
                .verified
                .lock()
                .map_err(|_| anyhow::anyhow!("WSL helper verification cache is poisoned"))?
                .contains(&cache_key);
        if !already_verified {
            ensure_helper(distribution, user, &architecture, &source_path, true)
                .await
                .map_err(classify_deployment_error)?;
            self.verified
                .lock()
                .map_err(|_| anyhow::anyhow!("WSL helper verification cache is poisoned"))?
                .insert(cache_key.clone());
        }

        invoke_with_recovery(
            &request.operation,
            || {
                invoke_helper(
                    distribution,
                    user,
                    &architecture,
                    request,
                    request.operation.wsl_timeout(),
                )
            },
            || async {
                ensure_helper(distribution, user, &architecture, &source_path, true)
                    .await
                    .map_err(classify_deployment_error)?;
                self.verified
                    .lock()
                    .map_err(|_| anyhow::anyhow!("WSL helper verification cache is poisoned"))?
                    .insert(cache_key.clone());
                Ok(())
            },
        )
        .await
    }
}

/*
 * Keep this separate from the cache lookup above: a cache entry only says that
 * this exact bundled resource was verified once. The remote identity is still
 * checked before every cache hit so an externally replaced helper cannot be
 * trusted just because the desktop process has seen it before.
 */
impl WslBridgeManager {
    #[cfg(test)]
    fn verified_key_for_test(
        distribution: &str,
        user: &str,
        architecture: &str,
        app_version: &str,
        bundled_helper_fingerprint: &str,
    ) -> VerifiedHelperKey {
        VerifiedHelperKey {
            distribution: distribution.to_ascii_lowercase(),
            user: user.to_string(),
            architecture: architecture.to_string(),
            app_version: app_version.to_string(),
            bundled_helper_fingerprint: bundled_helper_fingerprint.to_string(),
        }
    }
}

pub async fn get_status() -> Result<WslStatus> {
    #[cfg(not(target_os = "windows"))]
    {
        return Ok(WslStatus {
            supported: false,
            installed: false,
            distributions: Vec::new(),
            error: Some("WSL discovery is available only on Windows".to_string()),
        });
    }

    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("wsl.exe");
        command.args(["--list", "--quiet"]);
        hide_child_console(&mut command);
        let output = match run_command(command, None, PROBE_TIMEOUT).await {
            Ok(output) => output,
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
            {
                return Ok(WslStatus {
                    supported: true,
                    installed: false,
                    distributions: Vec::new(),
                    error: Some("wsl.exe was not found".to_string()),
                });
            }
            Err(error) => return Err(error),
        };
        if !output.status.success() {
            let stderr = decode_console_output(&output.stderr);
            return Ok(WslStatus {
                supported: true,
                installed: false,
                distributions: Vec::new(),
                error: Some(format!(
                    "wsl.exe --list --quiet failed with exit code {:?}: {}",
                    output.status.code(),
                    stderr.trim()
                )),
            });
        }
        Ok(WslStatus {
            supported: true,
            installed: true,
            distributions: parse_wsl_list_output(&output.stdout),
            error: None,
        })
    }
}

pub async fn discover() -> Result<WslDiscoveryReport> {
    let status = get_status().await?;
    if !status.supported || !status.installed {
        bail!(
            "WSL is not available: {}",
            status.error.as_deref().unwrap_or("unknown WSL status")
        );
    }
    let mut instances = Vec::new();
    let mut errors = Vec::new();
    for distribution in status.distributions {
        match probe(&distribution, None, None).await {
            Ok(probe) if probe.available => instances.push(probe),
            Ok(probe) => errors.push(WslDiscoveryError {
                distribution,
                error: probe
                    .error
                    .unwrap_or_else(|| "default user has no ~/.codex/config.toml".to_string()),
            }),
            Err(error) => errors.push(WslDiscoveryError {
                distribution,
                error: format!("{error:?}"),
            }),
        }
    }
    Ok(WslDiscoveryReport { instances, errors })
}

pub async fn probe(
    distribution: &str,
    user: Option<&str>,
    codex_home: Option<&str>,
) -> Result<WslProbe> {
    validate_distribution(distribution)?;
    if let Some(user) = user {
        validate_user(user)?;
    }
    if let Some(codex_home) = codex_home {
        validate_linux_absolute_path("Codex home", codex_home)?;
    }
    let requested_home = codex_home.unwrap_or("__DEFAULT__");
    let mut command = wsl_command(distribution, user);
    command.args([
        "--exec",
        "/bin/sh",
        "-c",
        PROBE_SCRIPT,
        "csm-probe",
        requested_home,
    ]);
    let output = run_command(command, None, PROBE_TIMEOUT).await?;
    let stdout = decode_console_output(&output.stdout);
    let stderr = decode_console_output(&output.stderr);
    parse_probe_response(
        distribution,
        &stdout,
        &stderr,
        output.status.success(),
        output.status.code(),
    )
}

fn parse_probe_response(
    distribution: &str,
    stdout: &str,
    stderr: &str,
    status_success: bool,
    status_code: Option<i32>,
) -> Result<WslProbe> {
    let marker_line = marked_line(stdout, PROBE_MARKER).with_context(|| {
        format!(
            "WSL probe returned no response marker (exit code {status_code:?}); stdout: {}; stderr: {}",
            stdout.trim(),
            stderr.trim()
        )
    })?;
    let fields = marker_line.split('\t').collect::<Vec<_>>();
    if fields.len() < 6 {
        bail!("WSL probe response is malformed: {marker_line}");
    }
    let actual_user = fields[0].to_string();
    let home = fields[1].to_string();
    let architecture = normalize_architecture(fields[2])?;
    let codex_home = fields[3].to_string();
    validate_user(&actual_user)?;
    validate_linux_absolute_path("user home", &home)?;
    validate_linux_absolute_path("Codex home", &codex_home)?;
    let available = fields[4] == "available" && status_success;
    let codex_cli = (!fields[5].trim().is_empty()).then(|| fields[5].trim().to_string());
    Ok(WslProbe {
        distribution: distribution.to_string(),
        user: actual_user,
        home,
        host_path: canonical_wsl_host_path(distribution, &codex_home),
        codex_home,
        architecture,
        codex_cli,
        available,
        error: (!available).then(|| {
            if status_code == Some(44) {
                "config.toml does not exist in the requested WSL Codex home".to_string()
            } else {
                format!(
                    "WSL probe failed with exit code {status_code:?}: {}",
                    stderr.trim()
                )
            }
        }),
    })
}

pub async fn translate_windows_path(
    distribution: &str,
    user: &str,
    windows_path: &str,
) -> Result<String> {
    validate_distribution(distribution)?;
    validate_user(user)?;
    if windows_path.trim().is_empty() || windows_path.contains('\0') {
        bail!("path to translate cannot be empty");
    }
    let mut command = wsl_command(distribution, Some(user));
    command.args(["--exec", "wslpath", "-a", "-u", windows_path]);
    let output = run_command(command, None, PROBE_TIMEOUT).await?;
    if !output.status.success() {
        bail!(
            "wslpath failed with exit code {:?}: {}",
            output.status.code(),
            decode_console_output(&output.stderr).trim()
        );
    }
    let translated = decode_console_output(&output.stdout).trim().to_string();
    validate_linux_absolute_path("translated path", &translated)?;
    Ok(translated)
}

async fn ensure_helper(
    distribution: &str,
    user: &str,
    architecture: &str,
    source_path: &Path,
    force: bool,
) -> Result<()> {
    let expected = expected_identity(architecture)?;
    if !force {
        if matches!(
            helper_identity(distribution, user, architecture).await,
            Ok(ref value) if value == &expected
        ) {
            return Ok(());
        }
    }
    let bytes = tokio::fs::read(source_path).await.with_context(|| {
        format!(
            "failed to read bundled WSL helper: {}",
            source_path.display()
        )
    })?;
    let mut command = wsl_command(distribution, Some(user));
    command.args([
        "--exec",
        "/bin/sh",
        "-c",
        DEPLOY_SCRIPT,
        "csm-deploy",
        APP_VERSION,
        architecture,
    ]);
    let output = run_command(command, Some(&bytes), PROBE_TIMEOUT)
        .await
        .map_err(classify_command_error)?;
    if !output.status.success() {
        return Err(bridge_failure(
            BridgeFailureKind::DeploymentRejected,
            format!(
                "failed to deploy WSL helper (exit code {:?}); stdout: {}; stderr: {}",
                output.status.code(),
                decode_console_output(&output.stdout).trim(),
                decode_console_output(&output.stderr).trim()
            ),
        ));
    }
    let helper_identity = helper_identity(distribution, user, architecture).await?;
    if helper_identity != expected {
        return Err(bridge_failure(
            BridgeFailureKind::ProtocolRejected,
            format!(
                "WSL helper identity mismatch after deployment: expected {}, got {}",
                serde_json::to_string(&expected)?,
                serde_json::to_string(&helper_identity)?
            ),
        ));
    }
    Ok(())
}

fn expected_identity(architecture: &str) -> Result<BridgeIdentity> {
    Ok(BridgeIdentity {
        protocol_version: WSL_BRIDGE_PROTOCOL_VERSION,
        app_version: APP_VERSION.to_string(),
        target_architecture: normalize_architecture(architecture)?,
    })
}

async fn helper_identity(
    distribution: &str,
    user: &str,
    architecture: &str,
) -> Result<BridgeIdentity> {
    let mut command = wsl_command(distribution, Some(user));
    command.args([
        "--exec",
        "/bin/sh",
        "-lc",
        LOGIN_IDENTITY_SCRIPT,
        "csm-outer",
        APP_VERSION,
        architecture,
    ]);
    let output = run_command(command, None, PROBE_TIMEOUT)
        .await
        .map_err(classify_command_error)
        .map_err(classify_deployment_error)?;
    if !output.status.success() {
        return Err(bridge_failure(
            BridgeFailureKind::DeploymentRejected,
            format!(
                "cached WSL helper identity is unavailable (exit code {:?}): {}",
                output.status.code(),
                decode_console_output(&output.stderr).trim()
            ),
        ));
    }
    let stdout = decode_console_output(&output.stdout);
    let identity = stdout
        .lines()
        .rev()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find_map(|line| serde_json::from_str::<BridgeIdentity>(line).ok())
        .ok_or_else(|| {
            bridge_failure(
                BridgeFailureKind::ProtocolRejected,
                format!(
                    "WSL helper identity command returned no parseable JSON; stdout: {}",
                    stdout.trim()
                ),
            )
        })?;
    Ok(identity)
}

async fn invoke_helper(
    distribution: &str,
    user: &str,
    architecture: &str,
    request: &BridgeRequest,
    operation_timeout: Duration,
) -> Result<serde_json::Value> {
    let request = serde_json::to_vec(request).context("failed to encode WSL bridge request")?;
    let mut command = wsl_command(distribution, Some(user));
    command.args([
        "--exec",
        "/bin/sh",
        "-lc",
        LOGIN_BRIDGE_SCRIPT,
        "csm-outer",
        APP_VERSION,
        architecture,
    ]);
    let output = run_command(command, Some(&request), operation_timeout)
        .await
        .map_err(classify_command_error)?;
    parse_bridge_response(
        &decode_console_output(&output.stdout),
        &decode_console_output(&output.stderr),
        output.status.success(),
        output.status.code(),
    )
}

fn parse_bridge_response(
    stdout: &str,
    stderr: &str,
    status_success: bool,
    status_code: Option<i32>,
) -> Result<serde_json::Value> {
    let encoded = marked_line(stdout, WSL_BRIDGE_RESPONSE_MARKER).ok_or_else(|| {
        bridge_failure(
            BridgeFailureKind::MissingResponseMarker,
            format!(
                "WSL helper returned no response marker (exit code {:?}); stdout: {}; stderr: {}",
                status_code,
                stdout.trim(),
                stderr.trim()
            ),
        )
    })?;
    let response: BridgeResponse = serde_json::from_str(encoded).map_err(|error| {
        bridge_failure(
            BridgeFailureKind::ResponseDecodeFailed,
            format!(
                "failed to decode WSL helper response (exit code {:?}): {error}; stdout: {}; stderr: {}",
                status_code,
                stdout.trim(),
                stderr.trim()
            ),
        )
    })?;
    if response.protocol_version != WSL_BRIDGE_PROTOCOL_VERSION {
        return Err(bridge_failure(
            BridgeFailureKind::ResponseProtocolRejected,
            format!(
                "WSL bridge protocol mismatch: desktop expects {}, helper returned {}",
                WSL_BRIDGE_PROTOCOL_VERSION, response.protocol_version
            ),
        ));
    }
    if !response.ok || !status_success {
        return Err(bridge_failure(
            BridgeFailureKind::OperationFailed,
            format!(
                "WSL helper operation failed (exit code {:?}): {}\nstdout:\n{}\nstderr:\n{}",
                status_code,
                response
                    .error
                    .as_deref()
                    .unwrap_or("helper returned an unsuccessful response"),
                stdout.trim(),
                stderr.trim()
            ),
        ));
    }
    response.result.ok_or_else(|| {
        bridge_failure(
            BridgeFailureKind::ResponseDecodeFailed,
            "WSL helper returned a successful response without a result",
        )
    })
}

async fn bundled_helper_fingerprint(path: &Path) -> Result<String> {
    let bytes = tokio::fs::read(path)
        .await
        .with_context(|| format!("failed to read bundled WSL helper: {}", path.display()))?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}

fn bundled_helper_path(app: &tauri::AppHandle, architecture: &str) -> Result<PathBuf> {
    let file_name = bundled_helper_file_name(architecture)?;
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| anyhow::anyhow!("failed to resolve application resources: {error}"))?;
    let candidates = [
        resource_dir.join("resources").join("wsl").join(&file_name),
        resource_dir.join("wsl").join(&file_name),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("wsl")
            .join(&file_name),
    ];
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "bundled WSL helper is missing; rebuild the Windows package with the WSL helper resource"
            )
        })
}

fn bundled_helper_file_name(architecture: &str) -> Result<String> {
    let architecture = normalize_architecture(architecture)?;
    Ok(format!("codex-session-manager-wsl-bridge-{architecture}"))
}

fn marked_line<'a>(stdout: &'a str, marker: &str) -> Option<&'a str> {
    stdout.lines().rev().find_map(|line| {
        line.trim_start()
            .strip_prefix(marker)
            .map(|line| line.trim_end_matches('\r'))
    })
}

fn wsl_command(distribution: &str, user: Option<&str>) -> Command {
    let mut command = Command::new("wsl.exe");
    command.args(["--distribution", distribution]);
    if let Some(user) = user {
        command.args(["--user", user]);
    }
    command.args(["--cd", "~"]);
    hide_child_console(&mut command);
    command
}

async fn run_command(
    mut command: Command,
    stdin: Option<&[u8]>,
    duration: Duration,
) -> Result<std::process::Output> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }
    command.kill_on_drop(true);
    let started_at = Instant::now();
    let mut child = command.spawn().context("failed to start wsl.exe")?;
    if started_at.elapsed() >= duration {
        terminate_child(&mut child).await;
        return Err(command_timeout("process spawn", duration));
    }

    let stdout = child
        .stdout
        .take()
        .context("failed to capture WSL stdout")?;
    let stderr = child
        .stderr
        .take()
        .context("failed to capture WSL stderr")?;
    let mut stdout_task = Some(tokio::spawn(read_all(stdout)));
    let mut stderr_task = Some(tokio::spawn(read_all(stderr)));
    let mut phase = if stdin.is_some() {
        "stdin write_all"
    } else {
        "wait_with_output"
    };
    let remaining = remaining_timeout(started_at, duration);
    let result = timeout(remaining, async {
        if let Some(input) = stdin {
            let mut child_stdin = child.stdin.take().context("failed to open WSL stdin")?;
            phase = "stdin write_all";
            child_stdin
                .write_all(input)
                .await
                .context("failed to send data to WSL")?;
            phase = "stdin close";
            child_stdin
                .shutdown()
                .await
                .context("failed to close WSL stdin")?;
            drop(child_stdin);
        }
        phase = "wait_with_output";
        let status = child.wait().await.context("failed to wait for wsl.exe")?;
        let stdout = stdout_task
            .take()
            .expect("stdout reader task must be present")
            .await
            .context("stdout reader task failed")??;
        let stderr = stderr_task
            .take()
            .expect("stderr reader task must be present")
            .await
            .context("stderr reader task failed")??;
        Ok::<std::process::Output, anyhow::Error>(std::process::Output {
            status,
            stdout,
            stderr,
        })
    })
    .await;
    match result {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => {
            abort_reader_tasks(&mut stdout_task, &mut stderr_task);
            terminate_child(&mut child).await;
            Err(error.context(format!("WSL command failed during {phase}")))
        }
        Err(_) => {
            abort_reader_tasks(&mut stdout_task, &mut stderr_task);
            terminate_child(&mut child).await;
            Err(command_timeout(phase, duration))
        }
    }
}

fn remaining_timeout(started_at: Instant, duration: Duration) -> Duration {
    duration.saturating_sub(started_at.elapsed())
}

fn command_timeout(stage: &str, duration: Duration) -> anyhow::Error {
    anyhow::Error::new(WslCommandTimeout {
        stage: stage.to_string(),
        duration,
    })
}

async fn terminate_child(child: &mut Child) {
    let _ = child.kill().await;
    let _ = timeout(Duration::from_secs(1), child.wait()).await;
}

fn abort_reader_tasks(
    stdout_task: &mut Option<tokio::task::JoinHandle<Result<Vec<u8>>>>,
    stderr_task: &mut Option<tokio::task::JoinHandle<Result<Vec<u8>>>>,
) {
    if let Some(task) = stdout_task.take() {
        task.abort();
    }
    if let Some(task) = stderr_task.take() {
        task.abort();
    }
}

async fn read_all<R: AsyncRead + Unpin>(mut reader: R) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .await
        .context("failed to read WSL command output")?;
    Ok(bytes)
}

fn decode_console_output(bytes: &[u8]) -> String {
    let has_utf16_le_bom = bytes.starts_with(&[0xff, 0xfe]);
    let utf16_le_without_bom = bytes.len() >= 4
        && bytes.len() % 2 == 0
        && bytes.chunks_exact(2).filter(|pair| pair[1] == 0).count() * 2 >= bytes.len() / 2;
    if has_utf16_le_bom || utf16_le_without_bom {
        let offset = if has_utf16_le_bom { 2 } else { 0 };
        let units = bytes[offset..]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

#[cfg(target_os = "windows")]
fn hide_child_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn hide_child_console(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_marker_ignores_login_shell_noise() {
        let stdout = format!(
            "welcome\n{}{{\"protocol_version\":2,\"ok\":true,\"result\":true}}\n",
            WSL_BRIDGE_RESPONSE_MARKER
        );
        assert_eq!(
            marked_line(&stdout, WSL_BRIDGE_RESPONSE_MARKER),
            Some("{\"protocol_version\":2,\"ok\":true,\"result\":true}")
        );
    }

    #[test]
    fn bridge_response_failures_are_classified_without_matching_error_text() {
        let cases = [
            (
                format!(
                    "{}{{\"protocol_version\":2,\"ok\":false,\"error\":\"failed\"}}",
                    WSL_BRIDGE_RESPONSE_MARKER
                ),
                true,
                Some(1),
                BridgeFailureKind::OperationFailed,
            ),
            (
                format!(
                    "{}{{\"protocol_version\":999,\"ok\":true,\"result\":null}}",
                    WSL_BRIDGE_RESPONSE_MARKER
                ),
                true,
                Some(0),
                BridgeFailureKind::ResponseProtocolRejected,
            ),
            (
                format!("{}not-json", WSL_BRIDGE_RESPONSE_MARKER),
                true,
                Some(0),
                BridgeFailureKind::ResponseDecodeFailed,
            ),
            (
                "login shell noise".to_string(),
                false,
                Some(1),
                BridgeFailureKind::MissingResponseMarker,
            ),
        ];

        for (stdout, status_success, status_code, expected_kind) in cases {
            let error = parse_bridge_response(&stdout, "", status_success, status_code)
                .expect_err("the test response must fail");
            assert_eq!(bridge_failure_kind(&error), Some(expected_kind));
        }

        let timeout = bridge_failure(BridgeFailureKind::Timeout, "timed out");
        assert_eq!(
            bridge_failure_kind(&timeout),
            Some(BridgeFailureKind::Timeout)
        );
    }

    #[test]
    fn read_only_missing_marker_is_retried_once_after_redeployment() {
        let operation = ProfileOperation::LoadSettings;
        let mut invocations = 0;
        let mut redeployments = 0;
        let result = tauri::async_runtime::block_on(invoke_with_recovery(
            &operation,
            || {
                invocations += 1;
                let result = if invocations == 1 {
                    Err(bridge_failure(
                        BridgeFailureKind::MissingResponseMarker,
                        "missing marker",
                    ))
                } else {
                    Ok(serde_json::json!({"retried": true}))
                };
                async move { result }
            },
            || {
                redeployments += 1;
                async { Ok(()) }
            },
        ))
        .expect("read-only operation should succeed on its retry");

        assert_eq!(invocations, 2);
        assert_eq!(redeployments, 1);
        assert_eq!(result, serde_json::json!({"retried": true}));
    }

    #[test]
    fn write_missing_marker_is_not_retried_and_result_is_explicitly_unknown() {
        let operation = ProfileOperation::SaveSettings {
            settings: codex_session_manager::settings::AppSettings::default(),
        };
        let mut invocations = 0;
        let mut redeployments = 0;
        let error = tauri::async_runtime::block_on(invoke_with_recovery(
            &operation,
            || {
                invocations += 1;
                async {
                    Err(bridge_failure(
                        BridgeFailureKind::MissingResponseMarker,
                        "missing marker",
                    ))
                }
            },
            || {
                redeployments += 1;
                async { Ok(()) }
            },
        ))
        .expect_err("an ambiguous write must be returned as an error");

        assert_eq!(invocations, 1);
        assert_eq!(redeployments, 0);
        assert_eq!(
            bridge_failure_kind(&error),
            Some(BridgeFailureKind::OperationResultUnknown)
        );
        assert!(error.to_string().contains("not retried"));
    }

    #[test]
    fn timeout_and_operation_failure_are_not_retried() {
        for failure_kind in [
            BridgeFailureKind::Timeout,
            BridgeFailureKind::OperationFailed,
            BridgeFailureKind::ResponseDecodeFailed,
            BridgeFailureKind::ResponseProtocolRejected,
        ] {
            let operation = ProfileOperation::LoadSettings;
            let mut invocations = 0;
            let mut redeployments = 0;
            let error = tauri::async_runtime::block_on(invoke_with_recovery(
                &operation,
                || {
                    invocations += 1;
                    let error = bridge_failure(failure_kind, "failure");
                    async move { Err(error) }
                },
                || {
                    redeployments += 1;
                    async { Ok(()) }
                },
            ))
            .expect_err("the failure must be returned");

            assert_eq!(invocations, 1, "failure kind: {failure_kind:?}");
            assert_eq!(redeployments, 0, "failure kind: {failure_kind:?}");
            assert_eq!(bridge_failure_kind(&error), Some(failure_kind));
        }
    }

    #[test]
    fn response_protocol_rejection_is_not_retried() {
        let operation = ProfileOperation::SaveSettings {
            settings: codex_session_manager::settings::AppSettings::default(),
        };
        let mut invocations = 0;
        let mut redeployments = 0;
        let error = tauri::async_runtime::block_on(invoke_with_recovery(
            &operation,
            || {
                invocations += 1;
                async {
                    Err(bridge_failure(
                        BridgeFailureKind::ResponseProtocolRejected,
                        "response protocol mismatch",
                    ))
                }
            },
            || {
                redeployments += 1;
                async { Ok(()) }
            },
        ))
        .expect_err("a response protocol rejection must be returned");

        assert_eq!(invocations, 1);
        assert_eq!(redeployments, 0);
        assert_eq!(
            bridge_failure_kind(&error),
            Some(BridgeFailureKind::ResponseProtocolRejected)
        );
    }

    #[test]
    fn protocol_rejection_is_retried_once_after_redeployment() {
        let operation = ProfileOperation::LoadSettings;
        let mut invocations = 0;
        let mut redeployments = 0;
        let result = tauri::async_runtime::block_on(invoke_with_recovery(
            &operation,
            || {
                invocations += 1;
                let result = if invocations == 1 {
                    Err(bridge_failure(
                        BridgeFailureKind::ProtocolRejected,
                        "protocol mismatch",
                    ))
                } else {
                    Ok(serde_json::json!(true))
                };
                async move { result }
            },
            || {
                redeployments += 1;
                async { Ok(()) }
            },
        ))
        .expect("protocol rejection should be recovered by redeployment");

        assert_eq!(invocations, 2);
        assert_eq!(redeployments, 1);
        assert_eq!(result, serde_json::json!(true));
    }

    #[test]
    fn write_missing_marker_after_protocol_retry_is_explicitly_unknown() {
        let operation = ProfileOperation::SaveSettings {
            settings: codex_session_manager::settings::AppSettings::default(),
        };
        let mut invocations = 0;
        let mut redeployments = 0;
        let error = tauri::async_runtime::block_on(invoke_with_recovery(
            &operation,
            || {
                invocations += 1;
                let error = if invocations == 1 {
                    bridge_failure(BridgeFailureKind::ProtocolRejected, "protocol mismatch")
                } else {
                    bridge_failure(BridgeFailureKind::MissingResponseMarker, "missing marker")
                };
                async move { Err(error) }
            },
            || {
                redeployments += 1;
                async { Ok(()) }
            },
        ))
        .expect_err("an ambiguous write after a protocol retry must fail");

        assert_eq!(invocations, 2);
        assert_eq!(redeployments, 1);
        assert_eq!(
            bridge_failure_kind(&error),
            Some(BridgeFailureKind::OperationResultUnknown)
        );
        assert!(error.to_string().contains("not retried"));
    }

    #[test]
    fn helper_login_script_uses_a_versioned_cache_path() {
        assert!(LOGIN_BRIDGE_SCRIPT.contains("codex-session-manager/$app_version/$architecture"));
        assert!(LOGIN_IDENTITY_SCRIPT.contains("--identity"));
        assert!(DEPLOY_SCRIPT.contains("codex-session-manager/$app_version/$architecture"));
        assert!(LOGIN_BRIDGE_SCRIPT.contains("CSM_WSL_HELPER_PATH"));
        assert!(LOGIN_IDENTITY_SCRIPT.contains("CSM_WSL_HELPER_PATH"));
        assert!(!LOGIN_BRIDGE_SCRIPT.contains("exec \"$1\""));
        assert!(!LOGIN_IDENTITY_SCRIPT.contains("exec \"$1\""));
    }

    #[test]
    fn helper_cache_key_contains_all_deployment_identity_dimensions() {
        let first = WslBridgeManager::verified_key_for_test(
            "Ubuntu",
            "dev",
            "x86_64",
            "0.5.1",
            "fingerprint-a",
        );
        let same = WslBridgeManager::verified_key_for_test(
            "ubuntu",
            "dev",
            "x86_64",
            "0.5.1",
            "fingerprint-a",
        );
        let changed_version = WslBridgeManager::verified_key_for_test(
            "Ubuntu",
            "dev",
            "x86_64",
            "0.5.2",
            "fingerprint-a",
        );
        let changed_architecture = WslBridgeManager::verified_key_for_test(
            "Ubuntu",
            "dev",
            "aarch64",
            "0.5.1",
            "fingerprint-a",
        );
        let changed_fingerprint = WslBridgeManager::verified_key_for_test(
            "Ubuntu",
            "dev",
            "x86_64",
            "0.5.1",
            "fingerprint-b",
        );

        assert_eq!(first, same);
        assert_ne!(first, changed_version);
        assert_ne!(first, changed_architecture);
        assert_ne!(first, changed_fingerprint);
    }

    #[test]
    fn probe_response_parsing_handles_available_and_missing_config() {
        let available = parse_probe_response(
            "Ubuntu",
            "noise\n__CSM_WSL_PROBE__dev\t/home/dev\tARM64\t/home/dev/.codex\tavailable\t/home/dev/.local/bin/codex\n",
            "",
            true,
            Some(0),
        )
        .unwrap();
        assert_eq!(available.user, "dev");
        assert_eq!(available.architecture, "aarch64");
        assert_eq!(
            available.codex_cli.as_deref(),
            Some("/home/dev/.local/bin/codex")
        );
        assert!(available.available);

        let missing = parse_probe_response(
            "Ubuntu",
            "__CSM_WSL_PROBE__dev\t/home/dev\tx86_64\t/home/dev/.codex\tmissing\t\n",
            "",
            false,
            Some(44),
        )
        .unwrap();
        assert!(!missing.available);
        assert_eq!(
            missing.error.as_deref(),
            Some("config.toml does not exist in the requested WSL Codex home")
        );
    }

    #[test]
    fn probe_uses_passwd_login_shell_and_never_inherited_shell() {
        assert!(PROBE_SCRIPT.contains("getent passwd"));
        assert!(PROBE_SCRIPT.contains("login_shell"));
        assert!(!PROBE_SCRIPT.contains("${SHELL"));
    }

    #[test]
    fn bundled_helper_fingerprint_changes_when_resource_changes() {
        let first = Sha256::digest(b"helper-a");
        let second = Sha256::digest(b"helper-b");
        assert_ne!(first, second);
    }

    #[test]
    fn stdin_write_timeout_terminates_a_child_that_does_not_read_stdin() {
        let mut command = Command::new(if cfg!(target_os = "windows") {
            "powershell.exe"
        } else {
            "sh"
        });
        if cfg!(target_os = "windows") {
            command.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 10"]);
        } else {
            command.args(["-c", "sleep 10"]);
        }

        let input = vec![b'x'; 4 * 1024 * 1024];
        let started_at = Instant::now();
        let error = tauri::async_runtime::block_on(run_command(
            command,
            Some(&input),
            Duration::from_millis(250),
        ))
        .expect_err("a child that does not read stdin must time out");

        assert!(started_at.elapsed() < Duration::from_secs(3));
        let message = format!("{error:?}");
        assert!(message.contains("stdin write_all"), "{message}");
        assert!(message.contains("terminated"), "{message}");
    }

    #[test]
    fn helper_resource_name_is_canonical_for_both_supported_architectures() {
        assert_eq!(
            bundled_helper_file_name("amd64").unwrap(),
            "codex-session-manager-wsl-bridge-x86_64"
        );
        assert_eq!(
            bundled_helper_file_name("arm64").unwrap(),
            "codex-session-manager-wsl-bridge-aarch64"
        );
        assert!(bundled_helper_file_name("riscv64").is_err());
    }

    #[test]
    fn console_output_decodes_utf16_le_without_a_bom() {
        let bytes = "Wsl/E_ACCESSDENIED"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(decode_console_output(&bytes), "Wsl/E_ACCESSDENIED");
    }
}
