---
layout: default
title: Install
permalink: /install/
---

# Install

## Requirements

- Windows 10 or later (x64)
- A keyboard with a numpad (or remapped keys — see Configuration). A wired
  USB numpad is the most reliable. On a dual-mode pad, **2.4G (USB receiver)
  before Bluetooth**. Cheap wireless pads may miss keys after idle until the
  radio is awake (see the [FAQ](faq.md#wireless-numpad-misses-the-first-key))
- Optional: a mouse with an X2 (forward) button — the default trigger

DialKey is **Windows-only** for now. WSL cannot see host input devices; build and
run on Windows, not inside WSL.

## Download

1. Open [GitHub Releases](https://github.com/bwlol457581/dialkey/releases).
2. Download `DialKey-<version>-windows-x64.zip`.
3. Verify the SHA256 checksum published next to the zip (PowerShell):

```powershell
Get-FileHash .\DialKey-*-windows-x64.zip -Algorithm SHA256
```

The hash must match the value in `SHA256SUMS.txt` (or the release notes).

## Extract and run

Extract the zip to any folder you can write to (USB drive is fine). Contents:

| File | Purpose |
|---|---|
| `dialkey.exe` | The app |
| `settings/settings.json` | Behaviour defaults (edit or use Settings UI) |
| `settings/slots.example.json` | Sample slots — copied to `slots.json` on first run |
| `lang/ja.toml`, `lang/vi.toml` | Reference UI packs (Japanese / Vietnamese). English is built into the exe. Add more with `lang/<code>.toml` |
| `LICENSE` | MIT |
| `THIRD_PARTY_LICENSES.html` | Dependency notices |

Run `dialkey.exe`. No installer, no admin rights required for normal use.

### First launch

If no personal `slots*.json` is present:

1. DialKey copies `slots.example.json` → `slots.json` inside `settings/`
2. The Settings window opens so you can register your own tools

JSON lives in **`settings/` next to the executable** (portable layout).
`lang/` and `logs/` stay beside the exe. Settings → General → **Backup** copies
`settings.json` and `slots*.json` into a folder you choose (default: Desktop),
as `{folder}/yyyyMMddHHmm/`. It does not copy the whole app folder. Restore with
**Load** on the same row (copies JSON **into** `settings/`; it does not keep
reading the stamp), or copy those JSON files back into `settings/` and tray →
**Reload**.

## SmartScreen

Releases are currently **unsigned**. Windows may show SmartScreen:

1. Click **More info**
2. Click **Run anyway**

Always verify the SHA256 checksum before trusting a download.

## Antivirus

Low-level keyboard hooks look similar to keyloggers. Some engines flag DialKey
heuristically. Each release is scanned on VirusTotal before publishing. See
[Privacy](privacy.md) and [Troubleshooting](troubleshooting.md).

## Autostart

Enable **Autostart** in Settings (or set `"autostart": true` in `settings.json`).
DialKey writes an HKCU Run key and re-heals it on each launch.

To remove the Run key without deleting config:

```text
dialkey.exe --uninstall
```

## Uninstall

1. Quit DialKey from the tray menu
2. Optionally run `dialkey.exe --uninstall` to clear autostart
3. Delete the folder

No registry leftovers beyond the optional Run key.

## Build from source

```powershell
git clone https://github.com/bwlol457581/dialkey.git
cd dialkey
cargo build --release
# binary: target\release\dialkey.exe
```

Use the release script for a distributable zip (see repository `scripts/release.ps1`).
