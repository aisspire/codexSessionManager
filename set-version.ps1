param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidatePattern('^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$')]
    [string]$Version,

    [switch]$WhatIf
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$Utf8NoBom = [System.Text.UTF8Encoding]::new($false)

function Get-ProjectPath {
    param([Parameter(Mandatory = $true)][string]$RelativePath)
    Join-Path -Path $Root -ChildPath $RelativePath
}

function Read-TextFile {
    param([Parameter(Mandatory = $true)][string]$Path)
    [System.IO.File]::ReadAllText($Path, $Utf8NoBom)
}

function Write-TextFile {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Text
    )

    if ($WhatIf) {
        Write-Host "Would update $Path"
        return
    }

    [System.IO.File]::WriteAllText($Path, $Text, $Utf8NoBom)
    Write-Host "Updated $Path"
}

function Update-CargoPackageVersion {
    param([Parameter(Mandatory = $true)][string]$RelativePath)

    $path = Get-ProjectPath $RelativePath
    $text = Read-TextFile $path
    $regex = [regex]::new(
        '(?ms)(^\[package\]\s*.*?^version\s*=\s*")[^"]+(")',
        [System.Text.RegularExpressions.RegexOptions]::Multiline
    )
    $updated = $regex.Replace($text, "`${1}$Version`${2}", 1)

    if ($updated -eq $text) {
        throw "Could not find [package] version in $RelativePath"
    }

    Write-TextFile -Path $path -Text $updated
}

function Update-CargoLockPackageVersion {
    param(
        [Parameter(Mandatory = $true)][string]$RelativePath,
        [Parameter(Mandatory = $true)][string]$PackageName
    )

    $path = Get-ProjectPath $RelativePath
    $text = Read-TextFile $path
    $escapedPackageName = [regex]::Escape($PackageName)
    $regex = [regex]::new(
        "(?ms)(^\[\[package\]\]\s*^name\s*=\s*`"$escapedPackageName`"\s*^version\s*=\s*`")[^`"]+(`")",
        [System.Text.RegularExpressions.RegexOptions]::Multiline -bor [System.Text.RegularExpressions.RegexOptions]::Singleline
    )
    $updated = $regex.Replace($text, "`${1}$Version`${2}", 1)

    if ($updated -eq $text) {
        throw "Could not find $PackageName package version in $RelativePath"
    }

    Write-TextFile -Path $path -Text $updated
}

function Update-JsonVersion {
    param(
        [Parameter(Mandatory = $true)][string]$RelativePath,
        [switch]$UpdatePackageLockRoot
    )

    $path = Get-ProjectPath $RelativePath
    $text = Read-TextFile $path

    $topLevelVersionRegex = [regex]::new(
        '(?m)^(\s*"version"\s*:\s*")[^"]+(")',
        [System.Text.RegularExpressions.RegexOptions]::Multiline
    )
    $updated = $topLevelVersionRegex.Replace($text, "`${1}$Version`${2}", 1)

    if ($updated -eq $text) {
        throw "Could not find top-level version in $RelativePath"
    }

    if ($UpdatePackageLockRoot) {
        $rootPackageVersionRegex = [regex]::new(
            '(?ms)(^\s*"packages"\s*:\s*\{\s*^\s*""\s*:\s*\{.*?^\s*"version"\s*:\s*")[^"]+(")',
            [System.Text.RegularExpressions.RegexOptions]::Multiline -bor [System.Text.RegularExpressions.RegexOptions]::Singleline
        )
        $rootUpdated = $rootPackageVersionRegex.Replace($updated, "`${1}$Version`${2}", 1)

        if ($rootUpdated -eq $updated) {
            throw "Could not find root package version in $RelativePath"
        }

        $updated = $rootUpdated
    }

    Write-TextFile -Path $path -Text $updated
}

function Remove-TauriConfigVersion {
    param([Parameter(Mandatory = $true)][string]$RelativePath)

    $path = Get-ProjectPath $RelativePath
    $text = Read-TextFile $path
    $updated = [regex]::Replace(
        $text,
        '(?m)^\s*"version"\s*:\s*"[^"]+"\s*,\r?\n',
        "",
        1
    )

    if ($updated -eq $text) {
        Write-Host "Tauri config already uses src-tauri\Cargo.toml as the version source"
        return
    }

    Write-TextFile -Path $path -Text $updated
}

Update-CargoPackageVersion "Cargo.toml"
Update-CargoPackageVersion "src-tauri\Cargo.toml"
Update-CargoLockPackageVersion "Cargo.lock" "codex-session-manager"
Update-CargoLockPackageVersion "src-tauri\Cargo.lock" "codex-session-manager"
Update-CargoLockPackageVersion "src-tauri\Cargo.lock" "codex-session-manager-desktop"
Remove-TauriConfigVersion "src-tauri\tauri.conf.json"
Update-JsonVersion "ui\package.json"
Update-JsonVersion "ui\package-lock.json" -UpdatePackageLockRoot

Write-Host "Version set to $Version"
