# A/B: baseline (pre-tighten) vs tighten focus match (resident path).
# Metrics: focus scan_ms (debug log) + notepad miss+spawn launch ms.
#
# Note: CLI one-shot `--launch` does NOT install the focus handler.
# Measurement must go through a resident DialKey (IPC), matching real use.
param(
    [string]$BaseExe = 'D:\Project\DialKey\_measure\baseline\dialkey.exe',
    [string]$NewExe = 'D:\Project\DialKey\_measure\tighten\dialkey.exe',
    [string]$BaseDir = 'D:\Project\DialKey\_measure\baseline',
    [string]$NewDir = 'D:\Project\DialKey\_measure\tighten',
    [int]$ScanN = 15,
    [int]$LaunchN = 10
)

$ErrorActionPreference = 'Stop'
$env:RUST_LOG = 'debug'

function Ensure-Cfg([string]$Dir) {
    New-Item -ItemType Directory -Force -Path $Dir, (Join-Path $Dir 'logs') | Out-Null
    @"
{
  "schemaVersion": 1,
  "logLevel": "debug",
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
    { "id": "1", "name": "Notepad", "path": "C:\\Windows\\System32\\notepad.exe" },
    { "id": "2", "name": "MissingDoc", "path": "C:\\Windows\\Temp\\dialkey-focus-miss-xyzzy.docx" }
  ]
}
"@ | Set-Content -Encoding utf8 (Join-Path $Dir 'slots.json')
}

function Stop-DialKeyAndNotepad {
    Get-Process dialkey, notepad -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 350
}

function Get-Stats([double[]]$xs) {
    $n = $xs.Count
    $mean = ($xs | Measure-Object -Average).Average
    $ss = 0.0
    foreach ($x in $xs) { $ss += ($x - $mean) * ($x - $mean) }
    $sd = if ($n -gt 1) { [math]::Sqrt($ss / ($n - 1)) } else { 0.0 }
    [pscustomobject]@{
        N    = $n
        Mean = [math]::Round($mean, 3)
        SD   = [math]::Round($sd, 3)
        Min  = [math]::Round(($xs | Measure-Object -Minimum).Minimum, 3)
        Max  = [math]::Round(($xs | Measure-Object -Maximum).Maximum, 3)
    }
}

function Start-Resident([string]$Exe, [string]$Dir) {
    $log = Join-Path $Dir 'logs\dialkey.log'
    if (Test-Path $log) { Remove-Item -Force $log }
    Start-Process -FilePath $Exe -WorkingDirectory $Dir | Out-Null
    $deadline = (Get-Date).AddSeconds(15)
    while ((Get-Date) -lt $deadline) {
        if ((Test-Path $log) -and (Select-String -Path $log -Pattern 'ready' -Quiet)) {
            return
        }
        Start-Sleep -Milliseconds 20
    }
    throw "resident ready timeout: $Exe"
}

function Measure-ScanMs([string]$Exe, [string]$Dir, [string]$SlotId, [int]$N) {
    Stop-DialKeyAndNotepad
    Start-Resident $Exe $Dir
    $log = Join-Path $Dir 'logs\dialkey.log'
    $vals = @()
    for ($i = 0; $i -lt $N; $i++) {
        Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
        Start-Sleep -Milliseconds 120
        $lineCountBefore = if (Test-Path $log) { @(Get-Content $log).Count } else { 0 }
        & $Exe --launch $SlotId | Out-Null
        $deadline = (Get-Date).AddSeconds(5)
        $found = $null
        while ((Get-Date) -lt $deadline) {
            if (Test-Path $log) {
                $all = @(Get-Content $log)
                if ($all.Count -gt $lineCountBefore) {
                    $newPart = $all[$lineCountBefore..($all.Count - 1)]
                    foreach ($line in $newPart) {
                        # tracing formats strings as scan_ms="1.234"
                        if ($line -match 'scan_ms="([0-9.]+)"' -or $line -match 'scan_ms=([0-9.]+)') {
                            $found = [double]$Matches[1]
                        }
                    }
                    if ($null -ne $found) { break }
                }
            }
            Start-Sleep -Milliseconds 25
        }
        if ($null -eq $found) {
            $tail = if (Test-Path $log) { (Get-Content $log -Tail 8) -join ' | ' } else { '<no log>' }
            throw "no scan_ms for $Exe slot $SlotId iter $i; tail=$tail"
        }
        $vals += $found
    }
    Stop-DialKeyAndNotepad
    return ,$vals
}

function Measure-LaunchMs([string]$Exe, [string]$Dir, [int]$N) {
    Stop-DialKeyAndNotepad
    Start-Resident $Exe $Dir
    $vals = @()
    for ($i = 0; $i -lt $N; $i++) {
        Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
        Start-Sleep -Milliseconds 150
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
        $vals += [double]$sw.Elapsed.TotalMilliseconds
        Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
    }
    Stop-DialKeyAndNotepad
    return ,$vals
}

foreach ($p in @($BaseExe, $NewExe)) {
    if (-not (Test-Path $p)) { throw "missing exe: $p" }
}

Ensure-Cfg $BaseDir
Ensure-Cfg $NewDir

Write-Host '=== scan_ms exe miss (slot 1, notepad not running) ==='
$b1 = Measure-ScanMs $BaseExe $BaseDir '1' $ScanN
$n1 = Measure-ScanMs $NewExe $NewDir '1' $ScanN
$sb1 = Get-Stats $b1; $sn1 = Get-Stats $n1
Write-Host ("baseline: " + ($sb1 | ConvertTo-Json -Compress))
Write-Host ("tighten:  " + ($sn1 | ConvertTo-Json -Compress))
Write-Host ("delta mean ms: {0}" -f ([math]::Round($sn1.Mean - $sb1.Mean, 3)))

Write-Host '=== scan_ms document miss (slot 2, missing docx) ==='
$b2 = Measure-ScanMs $BaseExe $BaseDir '2' $ScanN
$n2 = Measure-ScanMs $NewExe $NewDir '2' $ScanN
$sb2 = Get-Stats $b2; $sn2 = Get-Stats $n2
Write-Host ("baseline: " + ($sb2 | ConvertTo-Json -Compress))
Write-Host ("tighten:  " + ($sn2 | ConvertTo-Json -Compress))
Write-Host ("delta mean ms: {0}" -f ([math]::Round($sn2.Mean - $sb2.Mean, 3)))

Write-Host '=== app launch miss+spawn (notepad, resident IPC) ==='
$bL = Measure-LaunchMs $BaseExe $BaseDir $LaunchN
$nL = Measure-LaunchMs $NewExe $NewDir $LaunchN
$sbL = Get-Stats $bL; $snL = Get-Stats $nL
Write-Host ("baseline: " + ($sbL | ConvertTo-Json -Compress))
Write-Host ("tighten:  " + ($snL | ConvertTo-Json -Compress))
Write-Host ("delta mean ms: {0}" -f ([math]::Round($snL.Mean - $sbL.Mean, 3)))

Write-Host 'Done.'
