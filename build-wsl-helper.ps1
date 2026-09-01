param(
    [string]$Distribution,
    [string]$User
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ArtifactName = "codex-session-manager-wsl-bridge"
$ProtocolVersion = "2"
$ResourceDirectory = Join-Path $ProjectRoot "src-tauri\resources\wsl"
$Targets = @(
    [pscustomobject]@{
        Architecture = "x86_64"
        Target = "x86_64-unknown-linux-musl"
        Compiler = "musl-gcc"
        FilePattern = "x86-64"
    },
    [pscustomobject]@{
        Architecture = "aarch64"
        Target = "aarch64-unknown-linux-musl"
        Compiler = "aarch64-linux-musl-gcc"
        FilePattern = "ARM aarch64"
    }
)

$wslArguments = @()
if ($Distribution) {
    $wslArguments += @("--distribution", $Distribution)
}
if ($User) {
    $wslArguments += @("--user", $User)
}
$wslHomeArguments = $wslArguments + @("--cd", "~")

$linuxProjectRoot = (& wsl.exe @wslHomeArguments --exec wslpath -a -u $ProjectRoot).Trim()
if (-not $linuxProjectRoot.StartsWith("/")) {
    throw "wslpath did not return an absolute Linux project path: $linuxProjectRoot"
}

$runtimeArchitecture = (& wsl.exe @wslHomeArguments --exec uname -m).Trim().ToLowerInvariant()
$runtimeArchitecture = switch ($runtimeArchitecture) {
    "x86_64" { "x86_64"; break }
    "amd64" { "x86_64"; break }
    "aarch64" { "aarch64"; break }
    "arm64" { "aarch64"; break }
    default { throw "Unsupported WSL runtime architecture: $runtimeArchitecture" }
}

$buildScript = @'
set -eu
project_root=${CSM_BUILD_PROJECT_ROOT:?}
target=${CSM_BUILD_TARGET:?}
compiler=${CSM_BUILD_COMPILER:?}
cd "$project_root" || { echo "WSL cannot access the translated project path: $project_root" >&2; exit 19; }
command -v rustup >/dev/null 2>&1 || { echo "rustup is required inside WSL" >&2; exit 20; }
command -v "$compiler" >/dev/null 2>&1 || { echo "the requested musl compiler is missing: $compiler" >&2; exit 21; }
rustup target add "$target"
target_env=$(printf '%s' "$target" | tr '-' '_')
target_upper=$(printf '%s' "$target" | tr '[:lower:]-' '[:upper:]_')
export "CC_${target_env}=$compiler"
export "CARGO_TARGET_${target_upper}_LINKER=$compiler"
cargo build --locked --release --target "$target" --bin codex-session-manager-wsl-bridge
'@
$encodedBuildScript = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($buildScript))

foreach ($target in $Targets) {
    $buildEnvironment = @(
        "CSM_BUILD_PROJECT_ROOT=$linuxProjectRoot",
        "CSM_BUILD_TARGET=$($target.Target)",
        "CSM_BUILD_COMPILER=$($target.Compiler)"
    )
    $runnerScript = "printf '%s' '$encodedBuildScript' | base64 -d | /bin/sh"
    & wsl.exe @wslHomeArguments --exec env @buildEnvironment /bin/sh -c $runnerScript
    if ($LASTEXITCODE -ne 0) {
        throw "WSL helper build failed for $($target.Target) with exit code $LASTEXITCODE"
    }

    $linuxArtifact = "$linuxProjectRoot/target/$($target.Target)/release/$ArtifactName"
    $inspection = (& wsl.exe @wslHomeArguments --exec file $linuxArtifact).Trim()
    if ($LASTEXITCODE -ne 0 -or $inspection -notmatch $target.FilePattern -or $inspection -notmatch "(statically linked|static-pie linked)") {
        throw "WSL helper validation failed for $($target.Architecture): $inspection"
    }

    if ($runtimeArchitecture -eq $target.Architecture) {
        $reportedProtocol = (& wsl.exe @wslHomeArguments --exec $linuxArtifact --protocol-version).Trim()
        if ($LASTEXITCODE -ne 0 -or $reportedProtocol -ne $ProtocolVersion) {
            throw "WSL helper protocol mismatch for $($target.Architecture): expected $ProtocolVersion, got $reportedProtocol"
        }

        $identityJson = (& wsl.exe @wslHomeArguments --exec $linuxArtifact --identity).Trim()
        if ($LASTEXITCODE -ne 0) {
            throw "WSL helper identity command failed for $($target.Architecture): $identityJson"
        }
        $identity = $identityJson | ConvertFrom-Json
        if ($identity.protocol_version -ne [int]$ProtocolVersion -or $identity.target_architecture -ne $target.Architecture) {
            throw "WSL helper identity mismatch for $($target.Architecture): $identityJson"
        }
    } else {
        Write-Warning "Skipping protocol/identity execution for $($target.Architecture) on $runtimeArchitecture WSL; release CI validates both architectures under their matching containers."
    }

    New-Item -ItemType Directory -Force -Path $ResourceDirectory | Out-Null
    $windowsArtifact = Join-Path $ProjectRoot "target\$($target.Target)\release\$ArtifactName"
    $resourcePath = Join-Path $ResourceDirectory "$ArtifactName-$($target.Architecture)"
    Copy-Item -LiteralPath $windowsArtifact -Destination $resourcePath -Force
    Write-Host "WSL helper ready: $resourcePath"
}
