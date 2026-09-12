# Compare DialKey 0.13.1 vs focus-existing (n=10).
# Metrics: ready ms, Private/WS MB, app-launch ms (notepad cold = focus miss path).
# Stats: unbiased SD, Shapiro-Wilk, F (equal var), Student or Welch t (alpha 0.05).
param(
    [string]$BaseExe = 'D:\Project\DialKey\_measure\v0131\dialkey.exe',
    [string]$NewExe = 'D:\Project\DialKey\_measure\focus\dialkey.exe',
    [string]$BaseDir = 'D:\Project\DialKey\_measure\v0131',
    [string]$NewDir = 'D:\Project\DialKey\_measure\focus',
    [int]$N = 10
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'measure-stats.ps1')

function Stop-DialKeyAndNotepad {
    Get-Process dialkey, notepad -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 350
}

function Ensure-MeasureConfig([string]$Dir) {
    New-Item -ItemType Directory -Force -Path $Dir, (Join-Path $Dir 'logs') | Out-Null
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
  "instant": {}
}
"@ | Set-Content -Encoding utf8 (Join-Path $Dir 'settings.json')
    @"
{
  "schemaVersion": 1,
  "slots": [
    { "id": "1", "name": "Notepad", "path": "C:\\Windows\\System32\\notepad.exe" }
  ]
}
"@ | Set-Content -Encoding utf8 (Join-Path $Dir 'slots.json')
}

function Measure-ReadyMs([string]$Exe, [string]$Dir) {
    $log = Join-Path $Dir 'logs\dialkey.log'
    if (Test-Path $log) { Remove-Item -Force $log }
    $sw = [Diagnostics.Stopwatch]::StartNew()
    Start-Process -FilePath $Exe -WorkingDirectory $Dir | Out-Null
    $deadline = (Get-Date).AddSeconds(15)
    while ((Get-Date) -lt $deadline) {
        if ((Test-Path $log) -and (Select-String -Path $log -Pattern 'ready' -Quiet)) { break }
        Start-Sleep -Milliseconds 10
    }
    $sw.Stop()
    if (-not (Select-String -Path $log -Pattern 'ready' -Quiet -ErrorAction SilentlyContinue)) {
        throw "ready not found for $Exe"
    }
    return [double]$sw.Elapsed.TotalMilliseconds
}

function Measure-FootprintMb {
    Start-Sleep -Seconds 5
    $p = Get-Process dialkey
    return [pscustomobject]@{
        Private = [math]::Round($p.PrivateMemorySize64 / 1MB, 4)
        WS      = [math]::Round($p.WorkingSet64 / 1MB, 4)
    }
}

function Start-Resident([string]$Exe, [string]$Dir) {
    $log = Join-Path $Dir 'logs\dialkey.log'
    if (Test-Path $log) { Remove-Item -Force $log }
    Start-Process -FilePath $Exe -WorkingDirectory $Dir | Out-Null
    $deadline = (Get-Date).AddSeconds(15)
    while ((Get-Date) -lt $deadline) {
        if ((Test-Path $log) -and (Select-String -Path $log -Pattern 'ready' -Quiet)) { return }
        Start-Sleep -Milliseconds 20
    }
    throw "resident ready timeout: $Exe"
}

function Measure-AppLaunchMs([string]$Exe) {
    Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 200
    $sw = [Diagnostics.Stopwatch]::StartNew()
    & $Exe --launch 1 | Out-Null
    $deadline = (Get-Date).AddSeconds(8)
    while ((Get-Date) -lt $deadline) {
        if (Get-Process notepad -ErrorAction SilentlyContinue) { break }
        Start-Sleep -Milliseconds 5
    }
    $sw.Stop()
    if (-not (Get-Process notepad -ErrorAction SilentlyContinue)) {
        throw 'notepad did not start'
    }
    Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 150
    return [double]$sw.Elapsed.TotalMilliseconds
}

function Format-StatsRow([string]$label, $st) {
    '{0}: n={1} mean={2:N3} sd={3:N3} (unbiased) min={4:N3} max={5:N3} SS={6:N3}' -f `
        $label, $st.N, $st.Mean, $st.SD, $st.Min, $st.Max, $st.SS
}

function Format-Welch([string]$label, $w) {
    $verdict = if ($w.P -lt 0.05) { 'significant (p<0.05)' } else { 'not significant (p>=0.05)' }
    @(
        "${label}:"
        ('  0.13.1  mean={0:N3}  sd={1:N3}' -f $w.MeanA, $w.SdA)
        ('  focus   mean={0:N3}  sd={1:N3}' -f $w.MeanB, $w.SdB)
        ('  delta(focus-0.13.1)={0:N3}  ({1:N2}%)' -f $w.Delta, $w.Pct)
        ('  Welch t={0:N3}  df~{1:N2}  two-sided p~{2:N4}' -f $w.T, $w.Df, $w.P)
        ("  -> $verdict")
    ) -join "`n"
}

Ensure-MeasureConfig $BaseDir
Ensure-MeasureConfig $NewDir

Write-Host "=== 1) DialKey startup → ready (n=$N) ==="
Stop-DialKeyAndNotepad
# warmup each
$null = Measure-ReadyMs $BaseExe $BaseDir; Stop-DialKeyAndNotepad
$null = Measure-ReadyMs $NewExe $NewDir; Stop-DialKeyAndNotepad

$readyBase = [double[]]::new($N)
$readyNew = [double[]]::new($N)
for ($i = 0; $i -lt $N; $i++) {
    $readyBase[$i] = Measure-ReadyMs $BaseExe $BaseDir
    Stop-DialKeyAndNotepad
    $readyNew[$i] = Measure-ReadyMs $NewExe $NewDir
    Stop-DialKeyAndNotepad
    Write-Host ("  ready[{0}] 0.13.1={1:N1} focus={2:N1}" -f ($i + 1), $readyBase[$i], $readyNew[$i])
}

Write-Host "=== 2) Resident Private / WS after ready (n=$N, settle 5s) ==="
$privBase = [double[]]::new($N); $wsBase = [double[]]::new($N)
$privNew = [double[]]::new($N); $wsNew = [double[]]::new($N)
for ($i = 0; $i -lt $N; $i++) {
    Stop-DialKeyAndNotepad
    $null = Measure-ReadyMs $BaseExe $BaseDir
    $fp = Measure-FootprintMb
    $privBase[$i] = $fp.Private; $wsBase[$i] = $fp.WS
    Stop-DialKeyAndNotepad
    $null = Measure-ReadyMs $NewExe $NewDir
    $fp = Measure-FootprintMb
    $privNew[$i] = $fp.Private; $wsNew[$i] = $fp.WS
    Stop-DialKeyAndNotepad
    Write-Host ("  mem[{0}] 0.13.1 P={1:N3}/WS={2:N3}  focus P={3:N3}/WS={4:N3}" -f `
        ($i + 1), $privBase[$i], $wsBase[$i], $privNew[$i], $wsNew[$i])
}

Write-Host "=== 3) App launch (notepad cold = focus MISS + spawn) (n=$N) ==="
Stop-DialKeyAndNotepad
Start-Resident $BaseExe $BaseDir
# warmup launch
$null = Measure-AppLaunchMs $BaseExe
$launchBase = [double[]]::new($N)
for ($i = 0; $i -lt $N; $i++) {
    $launchBase[$i] = Measure-AppLaunchMs $BaseExe
    Write-Host ("  launch 0.13.1[{0}]={1:N2} ms" -f ($i + 1), $launchBase[$i])
}
Stop-DialKeyAndNotepad
Start-Resident $NewExe $NewDir
$null = Measure-AppLaunchMs $NewExe
$launchNew = [double[]]::new($N)
for ($i = 0; $i -lt $N; $i++) {
    $launchNew[$i] = Measure-AppLaunchMs $NewExe
    Write-Host ("  launch focus-miss[{0}]={1:N2} ms" -f ($i + 1), $launchNew[$i])
}
Stop-DialKeyAndNotepad

Write-Host ''
Write-Host '========== SUMMARY (mean +/- sd unbiased; Shapiro-Wilk / F / t) =========='
Write-MeasureCompare 'READY' $readyBase $readyNew '0.13.1' 'focus'
Write-Host ''
Write-MeasureCompare 'PRIVATE_MB' $privBase $privNew '0.13.1' 'focus'
Write-Host ''
Write-MeasureCompare 'WS_MB' $wsBase $wsNew '0.13.1' 'focus'
Write-Host ''
Write-MeasureCompare 'APP_LAUNCH_MS (focus=miss+spawn)' $launchBase $launchNew '0.13.1' 'focus-miss'

# Also dump raw for the doc
$out = Join-Path 'D:\Project\DialKey\_measure' 'compare-n10.json'
[pscustomobject]@{
    readyBase = $readyBase; readyNew = $readyNew
    privBase = $privBase; privNew = $privNew
    wsBase = $wsBase; wsNew = $wsNew
    launchBase = $launchBase; launchNew = $launchNew
} | ConvertTo-Json | Set-Content -Encoding utf8 $out
Write-Host "raw -> $out"
