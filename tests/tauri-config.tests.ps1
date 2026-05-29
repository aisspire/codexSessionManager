Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$ConfigPath = Join-Path $RepoRoot "src-tauri\tauri.conf.json"
$Config = Get-Content -Raw $ConfigPath | ConvertFrom-Json

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

Write-Host "tauri.conf.json bundle configuration test passed"
