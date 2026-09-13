# Host check: false-focus should NOT match backup / embedded titles.
# Scenario: Notepad opens `a.xlsx.bak` while DialKey launches slot for `a.xlsx`.
# Expect: focus_existing miss (no "matched"), then shell open attempt for a.xlsx.
param(
    [string]$SourceExe = 'D:\Project\DialKey\target\release\dialkey.exe',
    [string]$WorkDir = 'D:\Project\DialKey\_measure\false-focus',
    [string]$TempRoot = "$env:TEMP\dialkey-false-focus"
)

$ErrorActionPreference = 'Stop'
$env:RUST_LOG = 'debug'

function Stop-DialKeyAndHelpers {
    Get-Process dialkey, notepad -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 300
}

New-Item -ItemType Directory -Force -Path $WorkDir, (Join-Path $WorkDir 'logs'), $TempRoot | Out-Null
$Exe = Join-Path $WorkDir 'dialkey.exe'
if (-not (Test-Path $SourceExe)) {
    throw "missing release exe: $SourceExe (run cargo build --release first)"
}
Copy-Item -Force $SourceExe $Exe

$bak = Join-Path $TempRoot 'a.xlsx.bak'
$doc = Join-Path $TempRoot 'a.xlsx'
# Decoy backup content (Notepad title becomes "a.xlsx.bak - Notepad").
'backup decoy' | Set-Content -Encoding ascii $bak
# Real target does not need to exist for miss path; shell may fail open — we only care about focus.
if (-not (Test-Path $doc)) {
    'placeholder xlsx-like' | Set-Content -Encoding ascii $doc
}

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
"@ | Set-Content -Encoding utf8 (Join-Path $WorkDir 'settings.json')

$docJson = $doc.Replace('\', '\\')
@"
{
  "schemaVersion": 1,
  "slots": [
    { "id": "1", "name": "DocA", "path": "$docJson" }
  ]
}
"@ | Set-Content -Encoding utf8 (Join-Path $WorkDir 'slots.json')

if (-not (Test-Path $Exe)) {
    throw "missing staged exe: $Exe"
}

Stop-DialKeyAndHelpers
$log = Join-Path $WorkDir 'logs\dialkey.log'
if (Test-Path $log) { Remove-Item -Force $log }

# Open decoy so title contains embedded a.xlsx token as a.xlsx.bak
Start-Process notepad.exe -ArgumentList "`"$bak`""
Start-Sleep -Milliseconds 900
$np = Get-Process notepad -ErrorAction Stop | Sort-Object StartTime -Descending | Select-Object -First 1
$title = $np.MainWindowTitle
Write-Host ("decoy notepad pid={0} mainWindowTitle={1}" -f $np.Id, $title)
if ($title -notmatch 'a\.xlsx\.bak') {
    Write-Host 'WARN: expected title to include a.xlsx.bak; continuing anyway'
}

Start-Process -FilePath $Exe -WorkingDirectory $WorkDir | Out-Null
$deadline = (Get-Date).AddSeconds(15)
while ((Get-Date) -lt $deadline) {
    if ((Test-Path $log) -and (Select-String -Path $log -Pattern 'ready' -Quiet -ErrorAction SilentlyContinue)) { break }
    Start-Sleep -Milliseconds 30
}
if (-not (Test-Path $log)) {
    throw "DialKey did not create log at $log (is another instance running?)"
}
if (-not (Select-String -Path $log -Pattern 'ready' -Quiet)) {
    throw 'DialKey ready timeout'
}

Write-Host ("decoy notepad pid={0} mainWindowTitle={1}" -f $np.Id, (Get-Process -Id $np.Id -ErrorAction SilentlyContinue).MainWindowTitle)

& $Exe --launch 1 | Out-Null
Start-Sleep -Milliseconds 600

$matched = Select-String -Path $log -Pattern 'focus_existing: matched' -Quiet
$noMatch = Select-String -Path $log -Pattern 'focus_existing: no match' -Quiet
$scan = Select-String -Path $log -Pattern 'scan_ms="([0-9.]+)"' | Select-Object -Last 1
$scanMs = if ($scan -and $scan.Line -match 'scan_ms="([0-9.]+)"') { $Matches[1] } else { '?' }

Write-Host '--- result ---'
Write-Host ("focus_matched={0}" -f [bool]$matched)
Write-Host ("focus_no_match={0}" -f [bool]$noMatch)
Write-Host ("scan_ms={0}" -f $scanMs)
Select-String -Path $log -Pattern 'focus_existing|focused existing|process launched|launched via' |
    ForEach-Object { $_.Line.Substring(0, [Math]::Min(220, $_.Line.Length)) }

if ($matched) {
    Stop-DialKeyAndHelpers
    throw 'FAIL: decoy a.xlsx.bak title incorrectly focused (false focus)'
}
if (-not $noMatch) {
    Stop-DialKeyAndHelpers
    throw 'FAIL: expected focus_existing: no match'
}

Write-Host 'PASS: backup title did not steal focus'
Stop-DialKeyAndHelpers
