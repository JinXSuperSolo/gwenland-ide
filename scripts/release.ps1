#requires -Version 5.1
<#
.SYNOPSIS
  Local CD: build the GwenLand IDE release installers (MSI + NSIS) and,
  optionally, publish them to a GitHub Release.

.DESCRIPTION
  The GitHub Actions CD path cannot run the full Tauri bundle for us, so the
  release build is done locally here. This script:
    1. Verifies the toolchain (cargo, tauri-cli, pnpm).
    2. Checks that the git tree is clean and the tag matches tauri.conf.json.
    3. Runs `cargo tauri build` from frontend/ (which runs `pnpm build` first via
       beforeBuildCommand), producing MSI + NSIS installers.
    4. Verifies the bare exe is under the binary budget.
    5. Optionally creates the git tag and a GitHub Release with the installers
       attached (needs the `gh` CLI, authenticated).

  Run from anywhere; paths are resolved relative to the repo root.

.PARAMETER Version
  Release version WITHOUT the leading 'v', e.g. "0.1.0". Must match
  frontend/tauri.conf.json. Defaults to the version read from that file.

.PARAMETER Publish
  Also create the git tag (vVERSION) and a GitHub Release via `gh`, uploading the
  installers. Without this the script only builds locally.

.PARAMETER SkipChecks
  Skip the clean-tree / version-match guards (use only for local test builds).

.EXAMPLE
  ./scripts/release.ps1
  # Build installers for the version in tauri.conf.json, no publish.

.EXAMPLE
  ./scripts/release.ps1 -Version 0.1.0 -Publish
  # Build, tag v0.1.0, and publish a GitHub Release with the installers.
#>
[CmdletBinding()]
param(
    [string]$Version,
    [switch]$Publish,
    [switch]$SkipChecks
)

$ErrorActionPreference = 'Stop'

# --- Locate the repo root (this script lives in <root>/scripts) -------------
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$FrontendDir = Join-Path $RepoRoot 'frontend'
$ConfPath = Join-Path $FrontendDir 'tauri.conf.json'
$BudgetBytes = 6815744  # 6.5 MB binary budget

function Info($msg) { Write-Host "==> $msg" -ForegroundColor Cyan }
function Ok($msg)   { Write-Host "OK  $msg" -ForegroundColor Green }
function Fail($msg) { Write-Host "ERR $msg" -ForegroundColor Red; exit 1 }

# --- 1. Toolchain ----------------------------------------------------------
Info 'Checking toolchain'
foreach ($tool in 'cargo', 'pnpm') {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { Fail "$tool not found on PATH" }
}
# tauri-cli is a cargo subcommand.
cargo tauri --version *> $null
if ($LASTEXITCODE -ne 0) { Fail 'cargo tauri not found. Install with: cargo install tauri-cli --version "^2"' }
Ok 'cargo, pnpm, cargo tauri present'

# --- 2. Version + tree guards ----------------------------------------------
$conf = Get-Content $ConfPath -Raw | ConvertFrom-Json
$confVersion = $conf.version
if (-not $Version) { $Version = $confVersion }
Info "Target version: $Version (tauri.conf.json: $confVersion)"

if (-not $SkipChecks) {
    if ($Version -ne $confVersion) {
        Fail "Version mismatch: -Version $Version != tauri.conf.json $confVersion. Bump the config first."
    }
    $status = git -C $RepoRoot status --porcelain
    if ($status) { Fail "Working tree is not clean. Commit or stash before releasing.`n$status" }
    Ok 'Version matches and tree is clean'
} else {
    Write-Host 'WARN skipping clean-tree / version guards (-SkipChecks)' -ForegroundColor Yellow
}

# --- 3. Build the installers -----------------------------------------------
Info 'Building release bundle (cargo tauri build) — this runs pnpm build first'
Push-Location $FrontendDir
try {
    cargo tauri build
    if ($LASTEXITCODE -ne 0) { Fail 'cargo tauri build failed' }
} finally {
    Pop-Location
}

# --- 4. Binary budget check ------------------------------------------------
$exe = Join-Path $RepoRoot 'target/release/gwenland.exe'
if (Test-Path $exe) {
    $size = (Get-Item $exe).Length
    $sizeMb = [math]::Round($size / 1MB, 2)
    if ($size -gt $BudgetBytes) {
        Write-Host "WARN gwenland.exe is $sizeMb MB, over the 6.5 MB budget" -ForegroundColor Yellow
    } else {
        Ok "gwenland.exe is $sizeMb MB (under the 6.5 MB budget)"
    }
}

# --- Collect the produced installers ---------------------------------------
$bundleDir = Join-Path $RepoRoot 'target/release/bundle'
$artifacts = @()
$artifacts += Get-ChildItem -Path (Join-Path $bundleDir 'msi')  -Filter '*.msi'      -ErrorAction SilentlyContinue
$artifacts += Get-ChildItem -Path (Join-Path $bundleDir 'nsis') -Filter '*-setup.exe' -ErrorAction SilentlyContinue
if ($artifacts.Count -eq 0) { Fail "No installers found under $bundleDir" }

Info 'Built installers:'
$artifacts | ForEach-Object { Write-Host "    $($_.FullName)" }

# --- 5. Optional publish to a GitHub Release -------------------------------
if ($Publish) {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
        Fail 'gh CLI not found — install GitHub CLI and `gh auth login`, or drop -Publish and upload manually.'
    }
    $tag = "v$Version"
    Info "Publishing GitHub Release $tag"

    # Create the tag if it does not already exist.
    $existingTag = git -C $RepoRoot tag --list $tag
    if (-not $existingTag) {
        git -C $RepoRoot tag -a $tag -m "Release $tag"
        git -C $RepoRoot push origin $tag
        Ok "Created and pushed tag $tag"
    } else {
        Write-Host "WARN tag $tag already exists; reusing it" -ForegroundColor Yellow
    }

    $files = $artifacts | ForEach-Object { $_.FullName }
    gh release create $tag @files --title "GwenLand IDE $tag" --generate-notes --repo (git -C $RepoRoot remote get-url origin)
    if ($LASTEXITCODE -ne 0) { Fail 'gh release create failed' }
    Ok "Published $tag with $($artifacts.Count) installer(s)"
} else {
    Info 'Local build only. Re-run with -Publish to tag + upload via gh, or attach the installers above to a release manually.'
}

Ok 'Done.'
