#Requires -Version 5.1
<#
.SYNOPSIS
  Build a portable DialKey release zip with checksums and third-party notices.

.DESCRIPTION
  Steps (spec §11):
    1. cargo deny check (licenses / advisories)
    2. cargo about → THIRD_PARTY_LICENSES.html
    3. cargo build --release
    4. Stage zip contents
    5. SHA256SUMS.txt
    6. Print VirusTotal reminder (manual upload)

  Does not publish to GitHub or upload to VirusTotal — those stay manual.
#>
param(
    [string]$OutDir = "dist",
    [switch]$SkipDeny,
    [switch]$SkipAbout
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $Root

function Require-Cmd([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command not found: $Name"
    }
}

Require-Cmd cargo

$Manifest = Get-Content (Join-Path $Root "Cargo.toml") -Raw
if ($Manifest -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
    throw "Could not read version from Cargo.toml"
}
$Version = $Matches[1]
$ZipName = "DialKey-$Version-windows-x64.zip"
$Dist = Join-Path $Root $OutDir
$Stage = Join-Path $Dist "stage"
$ZipPath = Join-Path $Dist $ZipName
$SumsPath = Join-Path $Dist "SHA256SUMS.txt"

Write-Host "==> DialKey release $Version" -ForegroundColor Cyan

if (-not $SkipDeny) {
    Write-Host "==> cargo deny check"
    if (-not (Get-Command cargo-deny -ErrorAction SilentlyContinue) -and
        -not (Get-Command "cargo-deny.exe" -ErrorAction SilentlyContinue)) {
        # cargo deny is installed as a cargo subcommand
        cargo deny --version 2>$null | Out-Null
        if ($LASTEXITCODE -ne 0) {
            throw "cargo-deny is not installed. Run: cargo install --locked cargo-deny"
        }
    }
    cargo deny check
    if ($LASTEXITCODE -ne 0) { throw "cargo deny check failed" }
} else {
    Write-Host "==> skipping cargo deny (-SkipDeny)" -ForegroundColor Yellow
}

if (Test-Path $Stage) { Remove-Item $Stage -Recurse -Force }
New-Item -ItemType Directory -Path $Dist -Force | Out-Null
New-Item -ItemType Directory -Path $Stage -Force | Out-Null

$AboutOut = Join-Path $Stage "THIRD_PARTY_LICENSES.html"
if (-not $SkipAbout) {
    Write-Host "==> cargo about generate"
    cargo about --version 2>$null | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "cargo-about is not installed. Run: cargo install --locked cargo-about"
    }
    cargo about generate (Join-Path $Root "about.hbs") -o $AboutOut
    if ($LASTEXITCODE -ne 0) { throw "cargo about generate failed" }
} else {
    Write-Host "==> skipping cargo about (-SkipAbout)" -ForegroundColor Yellow
    Set-Content -Path $AboutOut -Value "<p>Third-party license generation was skipped.</p>" -Encoding utf8
}

Write-Host "==> cargo build --release"
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build --release failed" }

# Respect CARGO_TARGET_DIR / sandbox redirects (do not assume ./target).
$metaJson = cargo metadata --no-deps --format-version 1
if ($LASTEXITCODE -ne 0) { throw "cargo metadata failed" }
$meta = $metaJson | ConvertFrom-Json
$TargetDir = $meta.target_directory
$Exe = Join-Path $TargetDir "release\dialkey.exe"
if (-not (Test-Path $Exe)) { throw "Missing $Exe (target_directory=$TargetDir)" }
Write-Host "    binary: $Exe"

Copy-Item $Exe $Stage
Copy-Item (Join-Path $Root "LICENSE") $Stage
Copy-Item (Join-Path $Root "README.md") $Stage
$settingsDir = Join-Path $Stage "settings"
New-Item -ItemType Directory -Path $settingsDir -Force | Out-Null
Copy-Item (Join-Path $Root "config\settings.json") $settingsDir
Copy-Item (Join-Path $Root "config\slots.example.json") $settingsDir
# Copy folder *contents* into stage\<name>\ (not the folder itself — avoids lang\lang\ nesting).
function Copy-DirContents([string]$Src, [string]$DstName) {
    if (-not (Test-Path $Src)) { return }
    $dst = Join-Path $Stage $DstName
    New-Item -ItemType Directory -Path $dst -Force | Out-Null
    Copy-Item -Path (Join-Path $Src "*") -Destination $dst -Recurse -Force
}
Copy-DirContents (Join-Path $Root "lang") "lang"
Copy-DirContents (Join-Path $Root "samples") "samples"
if (Test-Path (Join-Path $Stage "lang\lang")) {
    throw "Staging bug: nested lang\lang - refuse to ship"
}
if (Test-Path (Join-Path $Stage "samples\samples")) {
    throw "Staging bug: nested samples\samples - refuse to ship"
}

# Portable layout is flat next to the exe (see docs/install.md). Never ship personal slots.
$required = @(
    "dialkey.exe",
    "LICENSE",
    "README.md",
    "settings\settings.json",
    "settings\slots.example.json",
    "THIRD_PARTY_LICENSES.html",
    "lang\ja.toml",
    "lang\vi.toml"
)
foreach ($name in $required) {
    if (-not (Test-Path (Join-Path $Stage $name))) {
        throw "Staging incomplete: missing $name"
    }
}
if (Test-Path (Join-Path $Stage "slots.json")) {
    throw "Refusing to ship personal slots.json - remove it from the stage folder"
}
if (Test-Path (Join-Path $Stage "settings\slots.json")) {
    throw "Refusing to ship personal slots.json - remove it from the stage folder"
}

if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }
Write-Host "==> zip $ZipName"
Compress-Archive -Path (Join-Path $Stage "*") -DestinationPath $ZipPath -Force

$hash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
$line = "$hash  $ZipName"
Set-Content -Path $SumsPath -Value $line -Encoding ascii
# Also drop a copy next to the zip naming used in docs
Set-Content -Path (Join-Path $Dist "$ZipName.sha256") -Value $line -Encoding ascii

Write-Host ""
Write-Host "Release artifacts:" -ForegroundColor Green
Write-Host "  $ZipPath"
Write-Host "  $SumsPath"
Write-Host "  SHA256: $hash"
Write-Host ""
Write-Host "Next (manual):" -ForegroundColor Yellow
Write-Host "  1. Upload the zip to https://www.virustotal.com/gui/home/upload"
Write-Host "  2. If clean enough, create a GitHub Release and attach:"
Write-Host "       $ZipName"
Write-Host "       SHA256SUMS.txt"
Write-Host "  3. Paste the VirusTotal link into the release notes"
Write-Host "  4. Refresh the docs site if needed (GitHub Pages from /docs)"
