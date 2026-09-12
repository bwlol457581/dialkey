# DialKey

**A software macropad.** Turn any cheap numpad into a left-hand command pad.
No firmware. No special hardware. Just a numpad and a number.

> Status: Windows **1.0.0** (portable zip). Use at your own risk. Full
> specification: [docs/spec.md](docs/spec.md). User docs:
> https://bwlol457581.github.io/dialkey/

## How it works — like a phone

| Phone | DialKey |
|---|---|
| Speed dial (1–9) | **Instant slots.** A bare digit launches immediately |
| International prefix (+01…) | **`+` then digits then Enter** for unlimited slots |
| Phone book (metaphor) | **Search** — find a slot when you forget the number |

Unlike a hardware macropad, the number of registered tools is unlimited.

## Not a Spotlight-style launcher

Raycast, Alfred, and Flow Launcher have you *type a name to search*.
DialKey has you *type a number you already know*. Different category.

## Install

1. Download `DialKey-<version>-windows-x64.zip` from
   [Releases](https://github.com/bwlol457581/dialkey/releases).
2. Verify SHA256 against `SHA256SUMS.txt`.
3. Extract anywhere and run `dialkey.exe` (portable; JSON in `settings/` next to the exe).

Docs: [Install](https://bwlol457581.github.io/dialkey/install/),
[Configuration](https://bwlol457581.github.io/dialkey/configuration/),
[Slots](https://bwlol457581.github.io/dialkey/slots/).

### Build from source

```powershell
cargo build --release
# target\release\dialkey.exe
```

Portable zip + checksums:

```powershell
.\scripts\release.ps1
```

## Privacy

- Keystroke contents are never recorded, stored, or transmitted
- The keyboard hook is active only while the DialKey typing window is open
- No network access (other than opening the docs URL or user-registered http(s) slots in your browser)
- No telemetry or analytics of any kind
- All configuration stays local
- The source is public and auditable
- Logs contain launch failures, and may record slow-launch duration (no paths) — never keystroke contents

### Antivirus false positives

DialKey installs a low-level keyboard hook while its window is open, which some
antivirus engines flag heuristically. Each release is checked on VirusTotal before
publishing. See the privacy notes above for what the hook does and does not do.

### SmartScreen

Releases are currently unsigned. If Windows SmartScreen warns you, choose
**More info → Run anyway**. Verify the SHA256 checksum published with each release.

## Support

Best effort. Issues are welcome, but feature requests will not necessarily be
implemented. This is a personal tool released in the hope it is useful to others.

## License

MIT. See [LICENSE](LICENSE). Third-party notices ship as `THIRD_PARTY_LICENSES.html`
in each release zip (generated with `cargo-about`).
