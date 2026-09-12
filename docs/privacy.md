---
layout: default
title: Privacy
permalink: /privacy/
---

# Privacy

DialKey installs a low-level keyboard hook **only while its typing window is
open**. That pattern is why antivirus products sometimes flag it. Here is what
the app does and does not do.

## Policy

- Keystroke contents are never recorded, stored, or transmitted
- The keyboard hook is active only while the DialKey typing window is open
- DialKey itself does not phone home
  (it may open a docs URL or a user-registered `http(s)` slot in your browser)
- No telemetry or analytics of any kind
- All configuration stays local (`settings/` next to the exe)
- The source is public and auditable
- Logs contain launch failures, and may record slow-launch duration (no paths) — never keystroke contents

## Antivirus / VirusTotal

Each published binary is scanned on VirusTotal before the release is announced.
If an engine false-positives, we prefer vendor appeals over packing or
obfuscation.

See also [Troubleshooting](troubleshooting.md).
