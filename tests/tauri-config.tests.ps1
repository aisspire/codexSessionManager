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

Write-Host "tauri.conf.json bundle configuration test passed"
