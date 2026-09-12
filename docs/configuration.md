---
layout: default
title: Configuration
permalink: /configuration/
---

# Configuration

DialKey keeps JSON in a **`settings/` folder next to the executable**:

| File | Contents | Distributed? |
|---|---|---|
| `settings/settings.json` | Triggers, keys, instant digits, search, timeouts | Yes (defaults) |
| `settings/slots.json` | Your registered tools (personal paths) | **No** — local only |
| `settings/slots.example.json` | Sample slots | Yes |

Older zips put those files next to `dialkey.exe`. On first launch DialKey moves
them into `settings/` (it does not overwrite a file that is already there).

Override the config directory:

```text
dialkey.exe --config D:\path\to\config
```

`--config` points at a **folder** that contains both files.

## settings.json

```json
{
  "schemaVersion": 1,
  "triggers": [
    { "type": "mouse",  "button": "x2", "suppress": true },
    { "type": "hotkey", "key": "" },
    { "type": "tray" }
  ],
  "keys": {
    "start":     "VK_ADD",
    "confirm":   "VK_RETURN",
    "cancel":    "VK_ESCAPE",
    "search":    "VK_OEM_5",
    "openWorkdir": "VK_CONTROL",
    "digitBack": "VK_RIGHT",
    "startLabel": "+",
    "confirmLabel": "Ent",
    "cancelLabel": "ESC",
    "searchLabel": "\\",
    "openWorkdirLabel": "Ctr",
    "digitBackLabel": "→"
  },
  "instant": {
    "0": false, "1": true, "2": false, "3": false, "4": false,
    "5": false, "6": false, "7": false, "8": false, "9": false
  },
  "search": {
    "mode": "substring",
    "caseSensitive": false
  },
  "ui": {
    "locale": ""
  },
  "timeoutSec": 30,
  "maxDigits": 10,
  "feedbackMs": 500,
  "logLevel": "info",
  "docsUrl": "https://bwlol457581.github.io/dialkey/",
  "autostart": false,
  "slotsFile": "slots.json",
  "modeSwitch": {
    "mouseButton": "x1",
    "suppress": true,
    "chordTimeoutMs": 2000,
    "keys": { "0": "slots.json" }
  }
}
```

### Fields

| Field | Default | Meaning |
|---|---|---|
| `schemaVersion` | `1` | Config schema version (not the app version). Missing / non-number → treat as `1` (logged). Newer than this build → refuse, do not overwrite |
| `dialkeyVersion` | written on save | App version that last wrote the file. Informational; never used to reject a load |
| `triggers` | mouse x2 + tray | How to open the typing window |
| `keys.start` | `VK_ADD` | Enter multi-digit mode (`+` on the numpad) |
| `keys.confirm` | `VK_RETURN` | Confirm a multi-digit number (launch only) |
| `keys.cancel` | `VK_ESCAPE` | Close the window |
| `keys.search` | `VK_OEM_5` | Open Search (`\`; legacy JSON key `phonebook` still accepted). Older `VK_DIVIDE` / `VK_OEM_2` bindings still accept both slash keys |
| `keys.openWorkdir` | `VK_CONTROL` | In multi-digit mode with a non-empty buffer: **open the working folder only** (does not launch Path; ignored in Idle). Legend short name **Ctr**. Left/right Ctrl both accepted. **Search window (1.0.0):** the same key also opens the folder there — tap it alone (press, then release with no other key in between) on the highlighted row or an exact-id query. Not the same as the per-slot `openWorkdir` bool (that one still opens the folder **with** Confirm / instant launch) |
| `keys.digitBack` | `VK_RIGHT` | Multi-digit: delete last digit; empty buffer returns to Idle; Idle closes the typing window |
| `keys.wakeEnabled` | `false` (omitted) | When true, swallow `keys.wake` while the typing window is open (the pad’s ON key). Settings → Keys (Advanced). Capture ON; leave off for a wired pad |
| `keys.wake` | `VK_SUBTRACT` (omitted) | ON-key VK. Empty = set default (numpad `VK_SUBTRACT`, keyboard `VK_OEM_MINUS`). Unused while off |
| `keys.active` | `numpad` (omitted) | Current key set (`numpad` / `keyboard`) |
| `keys.sets` | two shipped sets (omitted) | All key sets. Each has `id`, `chord` (`t` / `k`), and the same role keys as the live `keys.*` fields |
| `keys.startLabel` / `confirmLabel` / `cancelLabel` / `searchLabel` / `openWorkdirLabel` / `digitBackLabel` | *(empty)* | Optional typing-window legend labels (max 3 characters each). Empty = VK short form |
| `instant` | code Default: all `false`; shipped example: `"1": true` | Digits that launch immediately when pressed alone |
| `search.mode` | `substring` | Search match: `exact` / `prefix` / `substring` / `fuzzy` |
| `search.caseSensitive` | `false` | Search case sensitivity |
| `ui.locale` | `""` | UI language: empty = OS default, `en` = embedded English, otherwise `lang/<code>.toml` |
| `ui.showAdvanced` | `false` (omitted) | Settings: show Mode / Keys / Triggers / Search and extra General fields. Default view is Slots + language / autostart / folder / Backup |
| `timeoutSec` | `30` | Idle timeout in **typing mode only** (not Search) |
| `maxDigits` | `10` | Max digits in multi-digit mode |
| `feedbackMs` | `500` | How long the typing window stays open after a launch request to show acceptance feedback. `0` disables |
| `logLevel` | code Default: `warn`; shipped example: `info` | `error` / `warn` / `info` / `debug` |
| `docsUrl` | (shipped URL) | Opened by tray **Help** |
| `autostart` | `false` | HKCU Run key on launch |
| `slotsFile` | `slots.json` | Basename of the slots JSON in the config folder. Empty = load `slots.example.json` **read-only**. Daily switch: tray Mode / X1. Settings → General combo (**Advanced**) lists `slots*.json` (except the sample) |
| `backupFolder` | `""` (omitted) | Where Backup writes JSON. Empty = Desktop at backup time. Must be outside the app and settings folders |
| `modeSwitch.mouseButton` | `x1` | Mode-switch leader (must differ from the launch mouse button) |
| `modeSwitch.suppress` | `true` | Swallow the leader click (avoids browser Back on X1) |
| `modeSwitch.chordTimeoutMs` | `2000` | Wait for a digit/letter after the leader; `0` disables mouse mode switch |
| `modeSwitch.keys` | `{}` | Optional chord overrides (`"0"`–`"9"` → slots basename). Chord `0` is reserved for shipped `slots.json`; other books start at `1` in filename order |

### Triggers

| Type | Notes |
|---|---|
| `mouse` | `button`: `x1` / `x2` / `middle`. `suppress: true` eats the click |
| `hotkey` | Global hotkey string; empty disables |
| `tray` | Always available via the tray icon |

Pressing the trigger again while the typing window is open **closes** it.

Per-slot **`focusExisting`** (default on): when `false` in `slots.json`, DialKey
skips “already open → focus” for that id and always launches normally. See
[Slots](slots.md).

While the Settings window is open (especially during key capture), triggers are
disabled so they cannot fight the settings UI.

### Instant digits

When `instant["7"]` is `true`, pressing `7` alone launches slot `"7"` immediately.

- Digits with `instant: false` are **ignored** when pressed alone (no error)
- Instant digits can still be used as the first digit of a multi-digit sequence
  after `+` (e.g. `+71` Enter)

### Laptop note

The default start key is numpad `+` (`VK_ADD`). Notebooks without a numpad should
rebind **Start** in Settings once. DialKey does not also accept the main-keyboard
`=` / `+` key by default (layout-dependent).

### Encoding

- **Read:** UTF-8 with or without BOM (Notepad-safe)
- **Write:** UTF-8 without BOM
- Broken JSON is **never overwritten** — DialKey reports the error and continues
  with an empty slot set so you can repair the file by hand

### Reload

Tray menu → **Reload**, or run `dialkey.exe --reload`, re-reads both JSON files.
If a typing or Search window is open, DialKey closes it first, then reloads.
`--reload` exits with code 1 when no resident DialKey is running.

### Mode switch

Put extra slot books in the **settings folder** as `slots.<name>.json`. DialKey lists them
automatically (tray → **Mode**, or Settings → **Advanced** → **Mode** /
General → Slots file).

- Press the mode mouse button (default **X1**), then a digit `0`–`9` (or a
  letter for a key set). Timeout cancels the chord. Leader mouse, suppress,
  timeout, and the Numpad/Keyboard letters are on Settings → **Triggers** →
  **Switch triggers**.
- The tray **Mode** submenu and the Mode list **✔** both mark the book in
  `slotsFile` (Reload keeps them in sync)
- Chord **`0`** is reserved for the shipped book `slots.json`. Other books fill
  `1`–`9` in filename order (they never take `0` unless you Set an override).
  Optional overrides: Settings → **Advanced** → **Mode**, or `modeSwitch.keys`
- Optional book label: add `"meta": { "displayName": "Home" }` at the top of a
  slots JSON. Unset → filename. Settings does not edit this field — use
  **Open folder** on General and edit the JSON
- **Backup** on General copies `settings.json` and `slots*.json` into
  `{backup folder}/yyyyMMddHHmm/` (default folder: Desktop; same minute replaces
  that stamp folder). It does not copy the app folder. Success and errors show
  under the Backup row (no dialog). **Load** picks a stamp folder, then you
  choose all slot books or only the current book, and optionally `settings.json`
  (off by default). Chosen files are **copied into** `settings/` (never
  `slots.example.json`). DialKey does not keep using the stamp as the live
  folder; `backupFolder` is only Backup’s write destination. Mode `0`–`9` order
  does not change unless you restore `settings.json` (`modeSwitch.keys`).
  Settings reopens so the running process rereads disk.

Language packs live in `lang/` next to the exe. English is built in; add a
language by dropping `lang/<code>.toml`. The zip ships **Japanese (reference)**
and **Vietnamese (reference)** as samples. Missing keys fall back to English.
Change the language under Settings → General → **UI language**.

Slot `path` / `workdir` may use [date tokens](slots.md#date-tokens) such as
`{yyyyMM}` (expanded at launch, before environment variables). Long UNC /
local paths are kept as typed in JSON; at launch Windows uses an extended
prefix so they can exceed 260 characters. See the [FAQ](faq.md#a-long-unc-or-folder-path-does-not-open).
