---
layout: default
title: Troubleshooting
permalink: /troubleshooting/
---

# Troubleshooting

## Digits go to the foreground app

The keyboard hook should suppress digits while the typing window is open.

Checklist:

1. Confirm the DialKey window is visible (topmost overlay)
2. Quit other tools that install low-level keyboard hooks
3. Check `logs/` for hook install/uninstall errors
4. Restart DialKey; if Explorer was restarted, also confirm the tray icon returned

## Mouse button still reaches the browser

For mouse triggers, set `"suppress": true`. Default X2 suppresses the browser
“forward” action. If you use X1 or middle, remember you are sacrificing Back /
middle-click behaviour in other apps.

## Window does not appear on trigger

1. Tray → confirm DialKey is running
2. Open Settings — triggers are disabled while Settings is open; close it
3. Reload config (tray → **Reload**)
4. Check that a mouse / hotkey trigger is actually configured in `settings.json`

## Instant digit does nothing

`instant` for that digit may be `false` (default). Alone, non-instant digits are
**silently ignored**. Either enable instant for that digit, or use `+` then the
number then Enter.

## Launch fails

- Relative paths resolve from the **exe folder**, not the shell CWD
- Environment variables must use Windows `%VAR%` form
- URL slots need a real `http://` or `https://` prefix
- Script errors inside a `.bat` / `.ps1` are the script's responsibility —
  DialKey only detects process-create failures

## Tray icon missing after Explorer crash

DialKey listens for `TaskbarCreated` and re-adds the icon. If it still missing,
exit via Task Manager and start again.

## Config “disappeared” / empty slots

If JSON is corrupt, DialKey **refuses to overwrite** the file and starts with an
empty in-memory slot set. Repair `slots.json` by hand, or Settings → General →
**Load** from a Backup stamp folder (or copy files via **Open folder**), then
Reload if you copied by hand.

## Antivirus quarantine

1. Restore the file from quarantine / allowlist `dialkey.exe`
2. Compare SHA256 with the release `SHA256SUMS.txt`
3. Check the VirusTotal link in the release notes
4. If a specific engine false-positives, report it to that vendor (OSS + public
   source helps)

## Hook silently stopped working

Windows can drop a low-level hook whose callback exceeds ~300 ms
(`LowLevelHooksTimeout`). The mouse hook lives on its own thread and launches
do not block the UI loop. DialKey still re-installs the hook if the cursor
moves but callbacks stop. If problems persist, check for other hooks and
reboot.

## Still stuck

1. Set `"logLevel": "debug"` in `settings.json`, Reload, reproduce
2. Inspect `logs/` (no keystroke contents should appear)
3. Open an issue with OS version, DialKey version, and relevant log lines —
   **do not paste logs that somehow contain secrets from your slot paths if you
   consider them sensitive**
