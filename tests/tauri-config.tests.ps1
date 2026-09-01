Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$ConfigPath = Join-Path $RepoRoot "src-tauri\tauri.conf.json"
$Config = Get-Content -Raw $ConfigPath | ConvertFrom-Json
$WindowsConfigPath = Join-Path $RepoRoot "src-tauri\tauri.windows.conf.json"
$WindowsConfig = Get-Content -Raw $WindowsConfigPath | ConvertFrom-Json
$ReleaseWorkflowPath = Join-Path $RepoRoot ".github\workflows\release.yml"
$ReleaseWorkflow = Get-Content -Raw $ReleaseWorkflowPath
$WslHelperBuildPath = Join-Path $RepoRoot "build-wsl-helper.ps1"
$WslHelperBuild = Get-Content -Raw $WslHelperBuildPath

if (-not ($Config.PSObject.Properties.Name -contains "bundle")) {
    throw "Expected src-tauri\tauri.conf.json to contain a top-level bundle section"
}

if ($Config.bundle.active -ne $true) {
    throw "Expected bundle.active to be true so release builds create uploadable artifacts"
}

if (-not ($Config.bundle.PSObject.Properties.Name -contains "targets")) {
    throw "Expected bundle.targets to be set"
}

if ($Config.bundle.targets -ne "all") {
    throw "Expected bundle.targets to be all"
}

if (-not ($Config.bundle.PSObject.Properties.Name -contains "createUpdaterArtifacts")) {
    throw "Expected bundle.createUpdaterArtifacts to be set for Tauri updater artifacts"
}

if ($Config.bundle.createUpdaterArtifacts -ne $true) {
    throw "Expected bundle.createUpdaterArtifacts to be true"
}

if (-not ($Config.PSObject.Properties.Name -contains "plugins")) {
    throw "Expected src-tauri\tauri.conf.json to contain a top-level plugins section"
}

if (-not ($Config.plugins.PSObject.Properties.Name -contains "updater")) {
    throw "Expected plugins.updater to be configured"
}

$Updater = $Config.plugins.updater

if (-not ($Updater.PSObject.Properties.Name -contains "pubkey")) {
    throw "Expected plugins.updater.pubkey to be configured"
}

if ([string]::IsNullOrWhiteSpace($Updater.pubkey)) {
    throw "Expected plugins.updater.pubkey to be non-empty"
}

if (-not ($Updater.PSObject.Properties.Name -contains "endpoints")) {
    throw "Expected plugins.updater.endpoints to be configured"
}

$ExpectedEndpoint = "https://github.com/aisspire/codexSessionManager/releases/latest/download/latest.json"
if (-not ($Updater.endpoints -contains $ExpectedEndpoint)) {
    throw "Expected updater endpoint to include $ExpectedEndpoint"
}

if (-not ($Updater.PSObject.Properties.Name -contains "windows")) {
    throw "Expected plugins.updater.windows to be configured"
}

if ($Updater.windows.installMode -ne "passive") {
    throw "Expected Windows updater installMode to be passive"
}

if (-not ($WindowsConfig.bundle.resources -contains "resources/wsl/*")) {
    throw "Expected Windows Tauri config to bundle only the WSL helper resource directory"
}

if ($ReleaseWorkflow -notmatch "(?m)^  wsl-helper:") {
    throw "Expected release workflow to contain the WSL helper build job"
}

$buildJobMatch = [regex]::Match($ReleaseWorkflow, '(?ms)^  build:\r?\n(?<body>.*?)(?=^  [A-Za-z0-9_-]+:\s*$|\z)')
$windowsBuildJobMatch = [regex]::Match($ReleaseWorkflow, '(?ms)^  build-windows:\r?\n(?<body>.*?)(?=^  [A-Za-z0-9_-]+:\s*$|\z)')
if (-not $buildJobMatch.Success) {
    throw "Expected a shared macOS/Linux build job"
}
if (-not $windowsBuildJobMatch.Success) {
    throw "Expected an independent Windows build job"
}
$buildJob = $buildJobMatch.Groups['body'].Value
$windowsBuildJob = $windowsBuildJobMatch.Groups['body'].Value
if ($buildJob -match '(?m)^    needs:\s*') {
    throw "The macOS/Linux build job must not depend on the WSL helper job"
}
if ($buildJob -match 'windows-latest|wsl-helper|Download .*WSL bridge|Verify Windows bundle resources') {
    throw "The shared macOS/Linux build job must not contain Windows WSL helper steps"
}
if ($buildJob -notmatch 'macos-latest' -or $buildJob -notmatch 'ubuntu-22\.04') {
    throw "The shared build job must keep macOS and Linux release targets"
}
if ($windowsBuildJob -notmatch '(?m)^    needs: wsl-helper\s*$') {
    throw "Only the independent Windows build job should depend on the WSL helper job"
}
if ($windowsBuildJob -notmatch '(?m)^    runs-on: windows-latest\s*$') {
    throw "The WSL helper resources must be consumed by the Windows build job"
}
if ($windowsBuildJob -notmatch 'Download x86_64 WSL bridge' -or $windowsBuildJob -notmatch 'Download aarch64 WSL bridge') {
    throw "Expected the Windows build job to download both WSL helper architectures"
}
if ($windowsBuildJob -notmatch 'Install both WSL bridge resources' -or $windowsBuildJob -notmatch 'Verify Windows bundle resources') {
    throw "Expected the Windows build job to install and verify WSL helper resources"
}

if ($ReleaseWorkflow -notmatch "static-pie linked") {
    throw "Expected WSL helper validation to accept static PIE binaries"
}

foreach ($architecture in @("x86_64", "aarch64")) {
    if ($ReleaseWorkflow -notmatch "codex-session-manager-wsl-bridge-$architecture") {
        throw "Expected release workflow to publish the $architecture WSL helper resource"
    }
}

foreach ($target in @("x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl")) {
    if ($ReleaseWorkflow -notmatch [regex]::Escape($target)) {
        throw "Expected release workflow to build the $target helper"
    }
}

if ($ReleaseWorkflow -notmatch "messense/rust-musl-cross:[^\s]+@sha256:[0-9a-f]{64}") {
    throw "Expected musl helper build images to use immutable digests"
}

if ($ReleaseWorkflow -notmatch "--identity") {
    throw "Expected release workflow to validate WSL helper identity output"
}

if ($ReleaseWorkflow -notmatch "Verify Windows bundle resources") {
    throw "Expected release workflow to verify both helper resources before Tauri bundling"
}

if ($WslHelperBuild.Contains('$1')) {
    throw "Expected WSL helper build script to pass build values through named environment variables"
}

Write-Host "tauri.conf.json bundle configuration test passed"
