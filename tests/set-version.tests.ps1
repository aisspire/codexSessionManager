Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$ScriptUnderTest = Join-Path $RepoRoot "set-version.ps1"
$TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("codex-session-manager-version-test-" + [guid]::NewGuid().ToString("N"))

function Write-Utf8NoBom {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Text
    )

    $directory = Split-Path -Parent $Path
    if (-not [string]::IsNullOrWhiteSpace($directory)) {
        New-Item -ItemType Directory -Force -Path $directory | Out-Null
    }

    [System.IO.File]::WriteAllText($Path, $Text, [System.Text.UTF8Encoding]::new($false))
}

function Assert-Contains {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Expected
    )

    $text = [System.IO.File]::ReadAllText($Path, [System.Text.UTF8Encoding]::new($false))
    if (-not $text.Contains($Expected)) {
        throw "Expected $Path to contain: $Expected"
    }
}

function Assert-Matches {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Pattern
    )

    $text = [System.IO.File]::ReadAllText($Path, [System.Text.UTF8Encoding]::new($false))
    if ($text -notmatch $Pattern) {
        throw "Expected $Path to match: $Pattern"
    }
}

try {
    New-Item -ItemType Directory -Force -Path $TempRoot | Out-Null
    Copy-Item -Path $ScriptUnderTest -Destination (Join-Path $TempRoot "set-version.ps1")

    Write-Utf8NoBom (Join-Path $TempRoot "Cargo.toml") @'
[package]
name = "codex-session-manager"
version = "0.1.0"
edition = "2021"
'@

    Write-Utf8NoBom (Join-Path $TempRoot "Cargo.lock") @'
[[package]]
name = "codex-session-manager"
version = "0.1.0"
dependencies = []

[[package]]
name = "unrelated"
version = "0.1.0"
'@

    Write-Utf8NoBom (Join-Path $TempRoot "src-tauri\Cargo.toml") @'
[package]
name = "codex-session-manager-desktop"
version = "0.1.0"
edition = "2021"
'@

    Write-Utf8NoBom (Join-Path $TempRoot "src-tauri\Cargo.lock") @'
[[package]]
name = "codex-session-manager"
version = "0.1.0"
dependencies = []

[[package]]
name = "codex-session-manager-desktop"
version = "0.1.0"
dependencies = []

[[package]]
name = "unrelated"
version = "0.1.0"
'@

    Write-Utf8NoBom (Join-Path $TempRoot "src-tauri\tauri.conf.json") @'
{
  "productName": "Codex Session Manager",
  "version": "0.1.0",
  "identifier": "com.aisspire.codex-session-manager"
}
'@

    Write-Utf8NoBom (Join-Path $TempRoot "ui\package.json") @'
{
  "name": "codex-session-manager-ui",
  "version": "0.1.0",
  "private": true
}
'@

    Write-Utf8NoBom (Join-Path $TempRoot "ui\package-lock.json") @'
{
  "name": "codex-session-manager-ui",
  "version": "0.1.0",
  "lockfileVersion": 3,
  "packages": {
    "": {
      "name": "codex-session-manager-ui",
      "version": "0.1.0"
    }
  }
}
'@

    powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $TempRoot "set-version.ps1") 0.2.0
    if ($LASTEXITCODE -ne 0) {
        throw "set-version.ps1 exited with code $LASTEXITCODE"
    }

    Assert-Contains (Join-Path $TempRoot "Cargo.toml") 'version = "0.2.0"'
    Assert-Matches (Join-Path $TempRoot "Cargo.lock") 'name = "codex-session-manager"\r?\nversion = "0\.2\.0"'
    Assert-Contains (Join-Path $TempRoot "src-tauri\Cargo.toml") 'version = "0.2.0"'
    Assert-Matches (Join-Path $TempRoot "src-tauri\Cargo.lock") 'name = "codex-session-manager"\r?\nversion = "0\.2\.0"'
    Assert-Matches (Join-Path $TempRoot "src-tauri\Cargo.lock") 'name = "codex-session-manager-desktop"\r?\nversion = "0\.2\.0"'
    Assert-Contains (Join-Path $TempRoot "ui\package.json") '"version": "0.2.0"'
    Assert-Contains (Join-Path $TempRoot "ui\package-lock.json") '"version": "0.2.0"'

    $tauriConfig = [System.IO.File]::ReadAllText((Join-Path $TempRoot "src-tauri\tauri.conf.json"), [System.Text.UTF8Encoding]::new($false))
    if ($tauriConfig -match '"version"\s*:') {
        throw "Expected Tauri config version to be removed"
    }

    Write-Host "set-version.ps1 test passed"
}
finally {
    if (Test-Path $TempRoot) {
        Remove-Item -Recurse -Force $TempRoot
    }
}
