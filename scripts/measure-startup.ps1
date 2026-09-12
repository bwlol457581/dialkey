# Measure DialKey startup→ready (ms) and optional idle footprint.
# Copy the release exe into an isolated folder first (do not point at dist/stage).
# Writes minimal settings.json + slots.json so first-run Settings does not open.
# Usage:
#   .\scripts\measure-startup.ps1 -ExePath <isolated\dialkey.exe> -WorkDir <same folder> [-Runs 10] [-Footprint]
param(
    [Parameter(Mandatory = $true)][string]$ExePath,
    [Parameter(Mandatory = $true)][string]$WorkDir,
    [int]$Runs = 10,
    [switch]$Footprint
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'measure-stats.ps1')
$ExePath = (Resolve-Path $ExePath).Path
$exeDir = [System.IO.Path]::GetDirectoryName($ExePath)
New-Item -ItemType Directory -Force -Path $WorkDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $exeDir 'logs') | Out-Null

$workResolved = (Resolve-Path $WorkDir).Path.TrimEnd('\')
if ($workResolved -ne $exeDir.TrimEnd('\')) {
    Write-Warning "DialKey reads JSON next to the exe ($exeDir), not -WorkDir. Copy dialkey.exe into WorkDir and pass that as -ExePath."
}

function Write-MeasureJson([string]$Dir) {
    New-Item -ItemType Directory -Force -Path $Dir | Out-Null
    $settings = Join-Path $Dir 'settings.json'
    if (-not (Test-Path $settings)) {
        @"
{
  "schemaVersion": 1,
  "logLevel": "info",
  "feedbackMs": 0,
  "keys": {
    "start": "VK_ADD",
    "confirm": "VK_RETURN",
    "cancel": "VK_ESCAPE",
    "search": "VK_OEM_5",
    "openWorkdir": "VK_CONTROL"
  },
  "instant": { "1": true }
}
"@ | Set-Content -Encoding utf8 $settings
    }
    $slots = Join-Path $Dir 'slots.json'
    if (-not (Test-Path $slots)) {
        @"
{
  "schemaVersion": 1,
  "slots": [
    { "id": "1", "name": "Notepad", "path": "C:\\Windows\\System32\\notepad.exe" }
  ]
}
"@ | Set-Content -Encoding utf8 $slots
    }
}

# 0.15.2+ prefers {exe}/settings/; older builds read exe-adjacent JSON.
Write-MeasureJson -Dir $WorkDir
Write-MeasureJson -Dir (Join-Path $WorkDir 'settings')

function Stop-DialKey {
    Get-Process dialkey -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 400
}

function Measure-ReadyMs {
    $log = Join-Path $exeDir 'logs\dialkey.log'
    if (Test-Path $log) { Remove-Item -Force $log }
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $null = Start-Process -FilePath $ExePath -WorkingDirectory $exeDir -PassThru
    $deadline = (Get-Date).AddSeconds(15)
    $ready = $false
    while ((Get-Date) -lt $deadline) {
        if (Test-Path $log) {
            $text = Get-Content -Raw -ErrorAction SilentlyContinue $log
            if ($text -match 'ready') {
                $ready = $true
                break
            }
        }
        Start-Sleep -Milliseconds 20
    }
    $sw.Stop()
    if (-not $ready) {
        Stop-DialKey
        throw "ready line not found within timeout ($ExePath)"
    }
    Stop-DialKey
    return [int]$sw.ElapsedMilliseconds
}

function Measure-FootprintMb {
    $log = Join-Path $exeDir 'logs\dialkey.log'
    if (Test-Path $log) { Remove-Item -Force $log }
    Start-Process -FilePath $ExePath -WorkingDirectory $exeDir | Out-Null
    $deadline = (Get-Date).AddSeconds(15)
    $ready = $false
    while ((Get-Date) -lt $deadline) {
        if ((Test-Path $log) -and (Select-String -Path $log -Pattern 'ready' -Quiet)) {
            $ready = $true
            break
        }
        Start-Sleep -Milliseconds 20
    }
    if (-not $ready) {
        Stop-DialKey
        throw "ready line not found before footprint sample ($ExePath)"
    }
    Start-Sleep -Seconds 8
    $p = Get-Process dialkey
    $row = [pscustomobject]@{
        Private = [math]::Round($p.PrivateMemorySize64 / 1MB, 2)
        WS      = [math]::Round($p.WorkingSet64 / 1MB, 2)
    }
    Stop-DialKey
    return $row
}

Stop-DialKey
Write-Host "Warmup: $ExePath"
$null = Measure-ReadyMs

$samples = @()
for ($i = 1; $i -le $Runs; $i++) {
    $ms = Measure-ReadyMs
    $samples += $ms
    Write-Host ("  run {0}: {1} ms" -f $i, $ms)
}

$readySt = Get-UnbiasedStats @($samples | ForEach-Object { [double]$_ })
$avg = [math]::Round($readySt.Mean, 1)
$min = $readySt.Min
$max = $readySt.Max
$sd = [math]::Round($readySt.SD, 1)
Write-Host ("READY avg={0} sd={1} (unbiased n-1) min={2} max={3} n={4} samples=[{5}]" -f $avg, $sd, $min, $max, $Runs, ($samples -join ','))
Write-Host (Format-UnbiasedStatsLine 'READY' $readySt)

if ($Footprint) {
    $priv = @()
    $ws = @()
    for ($i = 1; $i -le $Runs; $i++) {
        $row = Measure-FootprintMb
        $priv += $row.Private
        $ws += $row.WS
        Write-Host ("  footprint {0}: Private={1} MB  WS={2} MB" -f $i, $row.Private, $row.WS)
    }
    $privSt = Get-UnbiasedStats @($priv)
    $wsSt = Get-UnbiasedStats @($ws)
    Write-Host ("PRIVATE avg={0} sd={1} (unbiased n-1) min={2} max={3} n={4} samples=[{5}]" -f `
        ([math]::Round($privSt.Mean, 2)), ([math]::Round($privSt.SD, 4)), $privSt.Min, $privSt.Max, $Runs, ($priv -join ','))
    Write-Host ("WS      avg={0} sd={1} (unbiased n-1) min={2} max={3} n={4} samples=[{5}]" -f `
        ([math]::Round($wsSt.Mean, 2)), ([math]::Round($wsSt.SD, 4)), $wsSt.Min, $wsSt.Max, $Runs, ($ws -join ','))
    Write-Host (Format-UnbiasedStatsLine 'PRIVATE' $privSt)
    Write-Host (Format-UnbiasedStatsLine 'WS' $wsSt)
}
