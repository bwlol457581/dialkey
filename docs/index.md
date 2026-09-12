---
layout: default
title: DialKey
permalink: /
---

# DialKey

**A software macropad.** Turn any cheap numpad into a left-hand command pad.
No firmware. No special hardware. Just a numpad and a number.

## Documentation

| Page | Contents |
|---|---|
| [Install](install.md) | Download, first run, SmartScreen |
| [Configuration](configuration.md) | `settings.json` reference |
| [Slots](slots.md) | Register tools and URL slots |
| [FAQ](faq.md) | Common questions |
| [Troubleshooting](troubleshooting.md) | Hooks, AV false positives, logs |
| [Privacy](privacy.md) | What DialKey does and does not record |

The full specification lives in the repository as
[`docs/spec.md`](https://github.com/bwlol457581/dialkey/blob/main/docs/spec.md).

## Quick start

1. Download the latest zip from
   [GitHub Releases](https://github.com/bwlol457581/dialkey/releases).
2. Extract anywhere (portable — no installer).
3. Run `dialkey.exe`. On first launch it copies `settings/slots.example.json` →
   `settings/slots.json` and opens Settings.
4. Press the mouse **X2** (forward) button, then type a number on your numpad.

| Phone | DialKey |
|---|---|
| Speed dial | Instant slots — a bare digit launches immediately |
| International prefix | `+` then digits then Enter for multi-digit slots |
| Phone book (metaphor) | **Search** — find a slot by name when you forget the number |

## Support

Best effort. Issues are welcome; feature requests are not guaranteed.
MIT licensed — see [LICENSE](https://github.com/bwlol457581/dialkey/blob/main/LICENSE).
