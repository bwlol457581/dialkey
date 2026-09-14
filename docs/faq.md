---
layout: default
title: FAQ
permalink: /faq/
---

# FAQ

### Slot edits vanish after Save?

The right-hand form must land in the **left list** before it is written to disk.
Press **Update slot** (or Save / Apply — those now commit an uncommitted form
automatically). If the new id is not in the list yet, it will not be saved.

### Is DialKey a Spotlight / Raycast clone?

No. Those tools search by **name**. DialKey launches by a **number you already
know** — like a phone's speed dial. Different category.

### Does it need a fancy macropad or QMK?

No. Any cheap USB numpad works. No firmware flashing. There is no device
pairing step — plug in a numpad and type numbers. Register tools (slots) in
Settings → **Slots**, or edit `slots.json` (see [Slots](slots.md)).

For a pad that is ready on the first key after idle, prefer a **wired USB**
numpad. See [Wireless numpad misses the first key](#wireless-numpad-misses-the-first-key).

### Verified hardware

Devices **actually used** with DialKey. Not a buy list or a ban list — only
what that unit did. Untried models stay off this table.

| | Meaning |
|---|---|
| **○** | No known issues |
| **△** | Usable; details in Notes |
| **×** | Unusable; details in Notes |

| Device | OS | Rating | Notes |
|---|---|---|---|
| Newmen TK-029S (numpad) | Windows 11 | **△** | **Bluetooth:** several keys late after sleep. **2.4G:** better than Bluetooth, but a sleep-wake delay still happens sometimes. One AAA. Not a DialKey bug |

Mouse side buttons (X1/X2) depend on the mouse; DialKey uses standard HID
codes, so most USB mice work without vendor software.

### Will it steal focus from my current app?

The typing window uses `WS_EX_NOACTIVATE` + topmost. Focus stays on the app you
were using. Digits are captured via a temporary keyboard hook, not via focus.

**Search** is different: it takes focus so IME and normal typing work.

### Why does my antivirus complain?

Global keyboard hooks look like keyloggers. DialKey installs the keyboard hook
**only while the typing window is shown**, never logs keystroke contents, and has
no telemetry. See [Privacy](privacy.md).

### What about SmartScreen?

First releases are unsigned. Use **More info → Run anyway**, and verify the
SHA256 checksum from the release page.

### NumLock is off — will digits work?

Yes. DialKey accepts both numpad codes and the NumLock-off navigation codes
(`VK_END` for 1, etc.). It never toggles NumLock for you.

### I pressed `+` but still see 0–9

You are still in Idle if the first line is blank and the legend still says
Trigger. When `+` is accepted the first line shows `+` (or your Start display)
and the legend switches to Open / Back. If it does not, the key never reached
Windows — a sleeping wireless pad, or a pad that does not send `VK_ADD`.
Do not press `+` a second time to “retry”: if the first one arrived, the
second closes the window. On some numpads, waking from sleep takes a
moment — watch the typing window and make sure the key you pressed
actually registered. See
[Wireless numpad misses the first key](#wireless-numpad-misses-the-first-key).
Or Capture **Start** if this pad does not send `VK_ADD`.

### Wireless numpad misses the first key

Many battery pads sleep after idle. Until the radio is **fully awake**, those
taps never reach Windows — sometimes one key, sometimes several in a row.
DialKey cannot see them, and cannot tell asleep from “you have not typed yet.”
The mouse trigger (X2) does not wake the pad.

**Bluetooth vs 2.4G (same pad)**

Bluetooth is usually the worse radio for this. On a dual-mode pad (the
TK-029S above is one), use the **USB receiver (2.4G)** as the first
compromise before buying another device. Keep Bluetooth for when you need
it, and expect more lost keys after idle. 2.4G on a cheap AAA pad can
still drop keys — better than Bluetooth, not the same as wired.

**What to buy if you want the first key to count**

- **Wired USB** — powered from the cable; no radio sleep. Best match.
- **2.4G with a USB receiver**, if the first key after idle is not lost.
- **Bluetooth last** on the same hardware.
- Pads with a setting to delay or disable sleep (uses more battery).

A USB-C cable on a wireless pad is often **charge only** — the pad can
still sleep unless you switch it to wired mode.

**ON key** (Settings → **Advanced** → **Keys**, on the selected key set)

On a pad with that trait, turn the checkbox on and Capture the pad’s ON
key (often Esc on the TK-029S). Leave it off on a wired pad. Shipped
**Numpad**: checkbox on, Esc. Shipped **Keyboard**: checkbox off, Esc
(Cancel is `-` on both sets, so turning ON on does not clash with Esc).

1. Open with X2.
2. Press ON (swallowed: it does not cancel, launch, or type into the app).
3. Then `+` or a digit as usual.

If ON is Esc, Capture Cancel onto another key first (the shipped
keyboard set already uses `-`).

On some numpads, waking from sleep takes a moment. Watch the typing
window and make sure the keys you pressed actually registered.

Do not press `+` a second time to “retry”: if one arrived, the next
closes the window.

Avoid waking with `+`, a digit, or NumLock — those are DialKey keys or,
on the TK-029S, the 2.4G / Bluetooth switch.

The typing window will not turn green/yellow for sleep. That would be a
guess, not a device state.

### A long UNC or folder path does not open

Windows often stops at **260 characters**. DialKey does not rewrite what you
typed in JSON. At launch it uses the extended form (`\\?\` for `C:\…`,
`\\?\UNC\` for `\\server\share\…`) and opens documents/folders through the
shell. Drop onto Path / Working directory uses a buffer as long as the OS
path. Help does not cover this.

### I am on a laptop without a numpad `+` key

Turn on **Advanced**, then switch the **Key set** to **Keyboard**
(Settings → Keys, tray → Keys, or X1 then `k`). The shipped keyboard set
uses the main `+` key (`VK_OEM_PLUS`). Capture is still there if that
key is missing. Switch back with X1 then `t`, or tray → Keys → Numpad.
DialKey is built around an external numpad; laptop use is a fallback.

The keyboard set’s typing window is **green** (`#C6E0B4`). The numpad
set stays **sticky-note yellow**. That is how you tell which set is live.
Search and Settings stay white.

### The typing window does not appear over a fullscreen game

Exclusive fullscreen can cover the overlay. The keyboard hook still runs, so
you can type a number and launch blind. Prefer borderless windowed if you want
to see the overlay.

### Launching a file that is already open

DialKey tries once to find an existing window (exact `.exe` image path, or a
**bounded** file-name token in the title) and bring it forward instead of
starting another copy. Browser window classes are skipped for document matches
so a download title does not steal focus. Tabbed apps such as Excel are
best-effort only — if nothing matches, a normal launch still runs.

To always start a new instance for one slot, set `"focusExisting": false` on
that slot (Settings → Slots), or see [Slots](slots.md).

### How do I back up settings?

Settings → **General** → **Backup**. That copies `settings.json` and `slots*.json`
into a folder you choose (default: Desktop), as `{folder}/yyyyMMddHHmm/`. It does
not copy the app folder. **Load** on the same row copies the chosen files **into
the live settings folder** (`settings/` next to the exe): all slot books or only
the current book, and optionally `settings.json`. DialKey does **not** keep
reading the stamp folder after that. Files are copied by name, so Mode codes
`0`–`9` stay as they are unless you also restore `settings.json` (that holds
`modeSwitch.keys`). `backupFolder` is only where Backup writes; Load asks for a
stamp each time. You can still copy files by hand, then tray → **Reload**.

### `"01"` vs `"1"` — are they the same?

No. Slot ids are **strings**. `"01"` and `"1"` are different slots. Write ids as
JSON strings if you need leading zeros.

### Can I register a website?

Yes. Set `path` to an `http://` or `https://` URL. It opens in your default
browser.

### What is `chain:`?

An advanced Path value that launches several other slots in order, e.g.
`chain:11,20,31`. Only **one level** deep — if a target is itself a `chain:`,
that target is skipped. Cycles cannot be saved. Missing targets warn in Settings
and fail at run time (other targets still run). **Open folder** (`.` on the
shipped numpad set)
opens each member’s working folder in that same order; URL members and nested
chains are skipped. Most users do not need this; register each tool as its own
slot unless you truly want a one-key bundle.

### Why is minute `{min}` instead of `{mm}`?

Date tokens use `{MM}` for month. In many date formats minutes are `{mm}`, but
that is easy to mistype as `{MM}` (or the reverse). The wrong path would be
generated silently with no error. DialKey therefore names minutes `{min}` so a
typo cannot quietly open the wrong folder. See [Slots — Date tokens](slots.md#date-tokens).

### Where are logs?

Next to the exe: `logs/`. If that folder is not writable, DialKey falls back to
`%LOCALAPPDATA%\DialKey\logs\`. Logs record launch **failures** only — never
keystroke contents.

### Can I run two copies?

No. A second instance asks the first to show its window, then exits.

### Is there macOS support?

Not yet. Planned as a future phase; the core is structured to stay portable.
