# DialKey Specification v1.0

> **A software macropad.** Turn any cheap numpad into a left-hand command pad.
> No firmware. No special hardware. Just a numpad and a number.

## 0. Status

| Item | Value |
|---|---|
| Specification revision | **v1.0** (aligned with the app major.minor. Patch `1.0.x` is the app version) |
| Latest app version | **1.0.1** (`Cargo.toml`) |
| First public edition | **1.0.0**. This document starts here. Earlier private builds are not listed |
| Public git history | This repository starts at **1.0.1**. 1.0.0 is recorded below as a product edition |
| Author notes | Not in this repository. Pre-1.0.0 history is not published |
| Document role | **Public specification.** Behaviour of the current build. If text and implementation disagree, the implementation is canonical and this document is corrected |

### Public revision history

From **1.0.0** onward. Pre-1.0.0 notes are not in this file.

| App | Date | Change |
|---|---|---|
| **1.0.0** | 2026-09-12 | First public edition |
| **1.0.1** | 2026-09-13 | ON (wake) is **independent per key set**. Numpad ON + Keyboard OFF (or the reverse) is valid. Switching sets keeps each set’s checkbox |

---

## Contents

0. Status
1. Concept
2. Input system
3. Two modes
4. Triggers
5. Numbering
6. Configuration files
7. Settings UI
8. Settled behaviour
9. UI
10. Implementation
11. Release and distribution
12. Privacy / security
13. Language packs

---

## 1. Concept

**A software macropad.**
The right hand stays on the mouse; the left hand uses a cheap generic numpad. **Press a mouse button to invoke DialKey, then type a number** to launch a tool.

The numpad is used for two reasons: **it is easy to operate with the left hand**, and **devices are cheap to obtain**.

### Mental model: the telephone

| Telephone | DialKey |
|---|---|
| Speed dial (1–9) | **Instant-fire slots.** A single digit launches |
| International call (+01-…) | **Multi-digit slots that start with `+`** |
| Phone book (metaphor) | **Search** — find a tool by name when the number is forgotten |

The metaphor is consistent end to end. Help copy can follow the same structure.

### Responsibility split (core of this design)

```
┌──────────────────────────────────┐
│  DialKey                                  │
│  Duty: accept a trigger, launch a process by number   │
│  ─────────────────────────  │
│  Invoked → receive a number → launch         │
│  Release control. The job ends here               │
└──────────────────────────────────┘
                    │ launch
                    ▼
┌──────────────────────────────────┐
│  Each script (existing bat/ps1)                  │
│  Duty: receive input and process it                     │
│  ─────────────────────────  │
│  The bat shows its own drop-wait window          │
│  → receive files → call ps1               │
└──────────────────────────────────┘
```

**Effects of this split**
- DialKey needs no drop UI, argument expansion, or extension dispatch
- Existing scripts can be registered without modification
- Each script still runs on its own without DialKey

### Design principles

| Principle | Content |
|---|---|
| Single responsibility | Stay a launcher. Do not take part in the work itself |
| Light resident | Fully event-driven. No timer loops |
| Blind operation | Being able to type without looking at the screen comes first. UI is secondary |
| Non-destructive to existing assets | Register existing bat / ps1 files without rewriting them |
| Do not add residents | Do not depend on vendor utilities. Standalone |
| Device-independent | Do not assume a specific model or vendor |
| Everything reassignable | Keys and buttons are user-changeable |
| Defaults are enough | The defaults are the canonical path for people who do not change settings. Settings is an escape hatch for people who want to change them. Help is steps only (writing everything makes Settings look mandatory) |
| Do not break files | Do not overwrite or delete configuration files on the user's behalf |

---

## 2. Input system

### Overview

```
Trigger (mouse button, etc.)
   │
   ▼
┌─────────────────────────────┐
│  DialKey window shown                │
│  NOACTIVATE + TOPMOST                │
│  Keyboard hook installed          │
└─────────────────────────────┘
   │
   ├─ Digit alone ────────────▶ Instant fire (when instant: true)
   │
   ├─ "+" ─▶ Multi-digit mode ─▶ digits…… ─▶ Enter ─▶ run
   │            │              │
   │            │              └─ Ctrl (`keys.openWorkdir`) ─▶ open working directory only (do not launch)
   │            └─ "+" pressed again ─▶ cancel
   │
   ├─ "/" ────────────────▶ Search mode
   │
   └─ ESC ────────────────▶ cancel
```

### Content of the typing window (input states)

Look, layout, and colours are **§9 Window display**. This section defines only the content per input state.

| State | Display |
|---|---|
| **Just after the trigger** (Idle) | **Top:** the first **line is empty** (same height as Multi). Then **10 dial lines for 0–9** (name if instant fire, otherwise a blank line to keep height). **Separator** `--------`. **Bottom:** subcommands (default **Trigger → Search → Cancel**. Open / Back hidden). No brand comment line. Option rows always reserve height for up to 5 items |
| Digit pressed (`instant` `false`) | No change (the key is ignored) |
| **After the multi-trigger** (Narrow down) | **Top:** first line is **Start display + current buffer** (`+` if empty; with digits, `+10` and the name). Then the next decade `{prefix}0`–`{prefix}9` as **10 lines** (show the number even when unregistered. Empty prefix is 0–9; e.g. `10` → 100–109. Do not prefix the decade with `+`). **Separator** `--------`. **Bottom:** default **Open → Search → Back → Cancel** (Trigger hidden). Option rows reserve height for up to 5 items |

> Fixing the dial at the top and subcommands at the bottom keeps number entry primary and helper keys secondary. Legend **visibility and order** can be changed independently in Settings → Keys Idle / Narrow down lists (this does not change the keys' behaviour).

**Narrow down in multi-digit mode covers every slot.** Instant-fire `"1"` is included
(so typing `+1` can still call `"1"` itself). The first line keeps Start + buffer, so after `+10` the next decade is 100–109 while “current is +10” stays visible.

**Confirm and back keys in multi-digit mode**

| Key | Action |
|---|---|
| Confirm (default Enter) | **Launch only** the slot whose ID is the buffer |
| Open-workdir (default Ctrl / legend `Ctr`, `keys.openWorkdir`) | **Open that ID's working directory only** (do not launch Path). Does not consult the slot's `openWorkdir` ✅. Folder is explicit `workdir`, or Path's parent if empty. Ignored in Idle. Ignored when the buffer is empty. URL / `chain:` have no folder (and are not launched) |
| Digit-back (default →, `keys.digitBack`, legend **Back**) | If the buffer has a digit, delete one digit. If empty, **return to Idle**. **In Idle, close the typing window** |

Even an instant-fire slot can open **the folder only** via multi-trigger → digits → Open-workdir (launch is Confirm / instant fire). To open the folder at the same time as launch, use the slot's **Also open working directory** (Confirm / instant-fire path).

Launch-accepted feedback (`feedbackMs`) on success is `Launching {id} — {name}`. **On launch failure**, the same place becomes `Launch failed {id} — {name}`, plus a toast and a log (§8). Do not show an id that is only an ellipsis.

### Digit-count limit

**Stop at 10 digits. Further input is ignored.**
This is a guard against abnormal input. Numbers longer than 10 digits are not expected in practice.

### Instant fire (`instant`)

Per digit. **Launch the moment the key is pressed.** One key = one action, as on a macropad.

```json
"instant": { "0": true, "4": true, "7": true }
```

- Default is all `false` (safe side)
- At most 10 digits can be set
- Because multi-digit mode starts with `+`, **a digit set to instant fire can still be the first digit of a longer number**
  (example: `7` is instant fire, but `+71` Enter still calls slot 71)

**When a digit with `instant: false` is pressed alone**

**Ignore it.** No error. Keep waiting for input.
(`false` is there to prevent accidental fire, so showing an error would be the wrong signal.)

**A digit with `instant: true` fires the moment it is pressed.**

### Start key `+`

**Default is `VK_ADD` (numpad `+`).**

| Question | Decision |
|---|---|
| Laptops have no `VK_ADD` | **Ask the user to reassign once.** Do not accept both keys |
| Why not accept both | `VK_OEM_PLUS` is layout-dependent (US = `=` key, JIS = `;` key) and would blur Help |
| Rationale | Numpad use is the intended path. A laptop is a fallback |

The docs site / FAQ must state that laptop users should reassign the start key (in-app Help does not carry the policy explanation).

### NumLock-independent

**Do not change NumLock state. Accept both key-code series.**

| Digit | NumLock ON | NumLock OFF |
|---|---|---|
| 0 | `VK_NUMPAD0` | `VK_INSERT` |
| 1 | `VK_NUMPAD1` | `VK_END` |
| 2 | `VK_NUMPAD2` | `VK_DOWN` |
| 3 | `VK_NUMPAD3` | `VK_NEXT` |
| 4 | `VK_NUMPAD4` | `VK_LEFT` |
| 5 | `VK_NUMPAD5` | `VK_CLEAR` |
| 6 | `VK_NUMPAD6` | `VK_RIGHT` |
| 7 | `VK_NUMPAD7` | `VK_HOME` |
| 8 | `VK_NUMPAD8` | `VK_UP` |
| 9 | `VK_NUMPAD9` | `VK_PRIOR` |

The hook sees raw key codes, so treating both as the same digit is enough.
Symbol keys (`VK_ADD` and the like) are not affected by NumLock.

### Key assignment list

| Function | Default | Changeable |
|---|---|---|
| Trigger (mouse) | x2 (Forward) | Yes |
| Start key | `+` (`VK_ADD`) | Yes |
| Confirm | Enter | Yes |
| Cancel | ESC (`VK_ESCAPE`) | Yes |
| Search | `\` (`VK_OEM_5`) | Yes |

### Auto-repeat

**Ignore key auto-repeat** so holding a key does not fire repeatedly.

---

## 3. Two modes

| | Typing mode | Search mode |
|---|---|---|
| Use | The number is known (primary path) | The number cannot be recalled at all |
| Open | Mouse button / hotkey | `\` key / tray left-click |
| Window | `WS_EX_NOACTIVATE` + `TOPMOST` | Normal (takes focus) |
| Input | Received via the keyboard hook | Normal input (**IME allowed**) |
| Focus | **Does not take focus** | Takes focus |
| Display | Number + candidate list (display only) | Search field + list |

**Typing mode does not take focus**, so:
- The previous app's focus is never disturbed (no restore step)
- `SetForegroundWindow` restrictions do not apply
- Input can continue after interruption, and digits do not leak into another app

**Search mode takes focus** (for IME). The typing window is `NOACTIVATE`, so DialKey is not the foreground process when Search opens. `SetForegroundWindow` alone can leave Search behind the current app (Excel and the like). The implementation attaches to the foreground thread with `AttachThreadInput`, briefly sets `HWND_TOPMOST`, then returns to `HWND_NOTOPMOST`. Search is not permanently TOPMOST. Tray left-click also takes a foreground lock on the host before posting.

### Search mode specification

**Search targets**: **slot number (id)** / name / program name / description

**Match modes** (chosen in settings)

| Mode | Behaviour |
|---|---|
| Exact | The string matches in full |
| Prefix | Matches from the start |
| **Substring (default)** | Contained anywhere |
| Fuzzy | Characters may be non-contiguous (`ptrl` → `patrol`) |

**Other**

- **Space-separated AND search** (Google-style). A hit must contain every term
- **Case-insensitive by default.** Changeable in settings

**Running from the list**

| Action | Behaviour |
|---|---|
| Arrow keys then Enter | Run |
| Click | Run (**once only.** Ignore nested repeats, double-clicks, and Enter) |
| Typing | Filter the list (**id / name / path / description**). Run is selection + Enter, or click |
| **Single press of `keys.openWorkdir`** (default Ctrl. Press and release with no other key in between) | **Open the folder only** (do not launch). Target is the selected row, or an exact-match id in the search field. Folder is registered `workdir`, or Path's parent if empty. URL / `chain:` have no folder (failure toast). Hint under the window: **`{key}: open folder`** |

**Close the Search window after a run.** Close it the same way after opening a folder. On launch or folder-open failure, toast + log (the typing window is already gone).

**Ctrl tap-alone**: fire only when the key is released with no other key in between. If another key (`A` / `C` and the like) arrives while it is held, treat it as a modifier and do not fire on release. **Standard Search-field edit shortcuts such as Ctrl+A (select all) and Ctrl+C/V (copy/paste) keep working** (OS key-state tracking is independent of the app intercept). The same rule applies when the role key is Capture-changed away from Ctrl.

---

## 4. Triggers

### Four families (all configurable; multiple registrations allowed)

| # | Method | Default | Intended use |
|---|---|---|---|
| A | Mouse button (launch) | **x2 (Forward)** | Primary desktop path |
| B | Hotkey (launch) | Unset | Optional |
| C | Tray menu | Always on | Laptops; environments with no mouse setup |
| D | Mouse button (mode) | **x1 (Back)** | Slot-book switch (§6). A **different button** from the launch trigger |

### Mouse-button method

**DialKey includes generic HID mouse-button customisation.**
Vendor utilities such as Logi Options+ are not required. Buttons 4/5 (X1/X2) and middle-click are
vendor-independent standard HID input, so **the same implementation works on any mouse**.

| Button | Function lost if suppressed |
|---|---|
| middle | Open in a new tab, close a tab, autoscroll |
| x1 | Browser Back |
| **x2 (default)** | **Browser Forward (rarely used in practice)** |

**Implementation note**: `WH_MOUSE_LL` is called on every mouse move.
Return immediately for anything that is not the target button.

### Mode switch (manual)

A separate family from the launch trigger (default x2). **Replace the entire slot JSON book** (§6).

| Action | Behaviour |
|---|---|
| **Mode mouse (default x1)** | Wait for a code (`modeSwitch.chordTimeoutMs`) |
| `0`–`9` during the wait | Switch to that book → update `slotsFile` → reload slots → tray notification |
| `a`–`z` during the wait (if assigned) | **Key-set** switch (default `t` = numpad, `k` = main keyboard). Not shared with book codes |
| **Timeout** / unrelated key during the wait | **Cancel** (do not treat as a launch trigger) |
| **Tray → Mode** | Pick directly from the enumerated books (show the display name when present) |
| Settings → General → Slots file (**Advanced** pin) | Manual pick |
| Settings → Triggers (**Advanced**) | Leader / suppress / timeout / key-set letters. Launch mouse / hotkey on the same tab |
| Settings → Mode (**Advanced**) | Code assignment (Assign/Unassign) / switch the current book |

**If the launch button and the mode button are the same, disable the mode side and log.**  
Digit codes work **only after the leader** (no clash with instant-fire / typing 0–9).  
While Settings is shown, both the launch trigger and the mode switch are disabled.

Foreground-linked automatic switching is not adopted.

### Trigger pressed again while a window is open

**Close it.** Same behaviour as ESC — an exit path for a mistaken press.

### Conflict with other apps

**DialKey takes the input (default).** Set `suppress: false` to let the input also reach other apps.

---

## 5. Numbering

### Category (leading digit)

The leading digit is a category. **Assignment is user-defined**.

| Leading digit | Use |
|---|---|
| 0 | Unassigned |
| 1 | App launch |
| 2 | Macros |
| 3 | File processing |
| 4–9 | Unassigned |

### Digit length

`+` enters multi-digit mode, so one-digit and multi-digit are distinct. There is no upper bound on length.

### Slot IDs are strings (important)

**`id` is a string, not an integer, so leading zeros are preserved.**

| Input | Slot |
|---|---|
| `1` (instant fire) or `+1` Enter | `"1"` |
| `+01` Enter | **`"01"` (a different slot)** |
| `+001` Enter | **`"001"` (another)** |

Normalising as an integer would collapse `01` into `1`.
**JSON `id` must be written as a string (`"01"`).**

Category (leading digit) is the first character of the string.

#### Allowed characters

**Digits 0–9 only.**
Values such as `"abc"` or `"1-2"` are **skipped with a warning in the log** (startup continues).

> The string type would otherwise accept any value, so the restriction is explicit.

#### Lenient read

A user editing by hand may write `"id": 11` as an **integer**.
**If it is written as an integer, convert it to a string and accept it.** Do not reject it as an error.

> A strict reject would block hand-editors at high rate.
> `01` cannot be written as an integer, so zero-padded IDs are necessarily strings.

#### Confusion warning

`1` and `01` look almost the same and are different objects, so they are easy to mix up after time away.

**Settings shows a warning: "`1` and `01` are both registered".**
They are not forbidden. Being able to notice is enough.

#### Sort order

**A string sort puts `10` before `2`.**
Search list display uses **natural sort** (compare as numbers).

> If unspecified, implementers will use a plain string sort.

#### Leading zeros

IDs are strings so `"1"` and `"01"` remain distinct. Collapsing leading zeros later is easy (normalise on read); the reverse is not.

### Memory hints (operations)

- Set a few most-used slots to instant fire (shortest path is two keystrokes)
- Group by purpose (10s = Git, 20s = PDF, and so on)
- Use mnemonics where they help

---

## 6. Configuration files

### Two-file layout (separated for public distribution)

| File | Content | Distribution |
|---|---|---|
| `settings.json` | Behaviour (triggers, key bindings, timeouts, and so on) | Defaults shipped |
| `slots.json` | Tool registrations (personal paths) | **Not shipped.** `.gitignore` |
| `slots.example.json` | Sample | Shipped |

**Location**: `{exe}/settings/` (portable. `lang/` `logs/` `samples/` sit next to the exe). If a legacy `settings.json` exists next to the exe, move it into `settings/` on first run (do not overwrite a file that is already at the destination). `--config` can name the folder explicitly.

### Slot books (mode)

Switch **purpose-specific slot books** such as work / hobby / games.  
The number → launch mental model does not change. Only “which book is current” changes.

**A mode is a wholesale swap of the slot JSON.** Per-key layers are not adopted.

```
settings.json slotsFile  → the book to read now (basename)
slots.json                  ← default personal book
slots.work.json             ← work
slots.hobby.json            ← hobby
slots.example.json          ← shipped sample (read-only when slotsFile is empty)
```

**Naming**: `slots.<mode>.json` (`<mode>` is preferably ASCII lowercase).  
**Auto-enumerate** `slots*.json` in the settings folder (except `slots.example.json`). `settings` does not store a mode list.

**Book display name**: optional field `meta.displayName` on each JSON (below). Unset or empty = basename.  
**The Settings UI does not edit the display name** (edit the JSON directly. The General settings-folder button opens the folder).

**Code assignment** (one stroke after the leader. Operations in §4):

| Code | Assignment |
|---|---|
| `0` | Shipped personal book `slots.json` (copied from example). Other books do not fill this slot if the file is missing. User-overridable |
| `1` … `9` | User-specified (`modeSwitch.keys` / Set), otherwise filename order |
| Tray only | Books that have no remaining code |

- **Auto-assign:** code `0` is reserved for shipped `slots.json`. If it is missing, `0` stays empty (do not fill it with another book). The rest take `1`…`9` in filename order. Overrides are `modeSwitch.keys` (Settings → Mode (**Advanced**), or JSON)
- Checks on load (ignore a bad override. Do not rewrite the settings file):
  - A code is exactly one character `"0"`–`"9"` (legacy `a`–`z` is ignored with a warning)
  - Two files on the same code, or two codes on the same file, are not allowed
  - A value that is not a candidate / `slots.example.json` / empty is not allowed
  - If the launch mouse and the mode mouse are the same, disable the mode side

**Out of scope**: per-mode hotkeys in the slot JSON / foreground auto-switch / a Settings UI to edit display names

### settings.json

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

> **`keys.search` and top-level `search` are different.**  
> The former is the key that opens Search mode; the latter is the match-mode setting.  
> Legacy `keys.phonebook` is accepted as `keys.search` on read (writes use `search` only).

| Key | Default | Meaning |
|---|---|---|
| `schemaVersion` | `1` | Settings schema version (separate from the app version). No migration on the current schema. **Missing / non-numeric: warn and treat as `1`**. Too new: stop without overwriting |
| `dialkeyVersion` | `Cargo.toml` version at save time (optional) | App version that last wrote the file. For logs. **Not used to refuse a read** (refusal is `schemaVersion` only) |
| `triggers` | mouse x2 + empty hotkey + tray | How the typing window opens. Same defaults when fields are missing |
| `keys.start` / `confirm` / `cancel` / `search` / `openWorkdir` / `digitBack` | `VK_ADD` / `RETURN` / `ESCAPE` / `OEM_5` / `CONTROL` / `RIGHT` | Role keys. Legacy `keys.phonebook` is accepted as `search` on read. **Search default is `\` (`VK_OEM_5`)**. When a legacy setting is `VK_DIVIDE` / `VK_OEM_2`, accept both slashes. **`openWorkdir` (string) opens the working directory only in multi-digit mode** (does not launch. Default Ctrl, short name **Ctr**. The LL hook also accepts left and right Ctrl. Distinct from the slot bool `openWorkdir`). **The Search window uses the same role key** (**1.0.0**. A normal Edit control, so **tap-alone** rather than the typing window's immediate intercept. A Capture applies to both windows). **`digitBack`** is digit-back / Idle if empty / **close the typing window in Idle**. `digitBack` allows NumLock-off arrow VKs (other roles reserve digits) |
| `keys.wakeEnabled` | `false` (optional) | **When on**, swallow the `keys.wake` VK while the typing window is shown. Does not change the sequence. Wireless-pad **ON**. **Per key set** (numpad and keyboard are independent. Numpad ON + Keyboard OFF is valid). Wired pads do not need it — leave off. Settings → Keys (**Advanced**). Write to JSON only when `true`. When on, that VK must not duplicate another role (no swap) |
| `keys.wake` | `VK_SUBTRACT` (optional) | ON VK for the **active** set. Empty uses `VK_SUBTRACT` on numpad, `VK_OEM_MINUS` on keyboard. Unused when that set’s ON is off |
| `keys.active` | `"numpad"` (optional) | Current key-set id. `"numpad"` / `"keyboard"`. Numpad if omitted |
| `keys.sets` | Two shipped sets (optional) | Each item is id / chord (a–z; defaults `t` / `k`) plus a full role-key set including `wake` / `wakeEnabled`. Sets do not share ON. If empty, load builds numpad + main-keyboard defaults from live keys. Do not bump `schemaVersion` |
| `keys.startLabel` / `confirmLabel` / `cancelLabel` / `searchLabel` / `openWorkdirLabel` / `digitBackLabel` | `+` / `Ent` / `ESC` / `\` / `Ctr` / `→` | Typing-window legend display names (max 3 characters each). Empty = VK short name |
| `instant` | Code default: all `false` / shipped example: `"1": true` | Digits that launch immediately on a lone press |
| `search.mode` | `substring` | `exact` / `prefix` / `substring` / `fuzzy` |
| `search.caseSensitive` | `false` | Search case sensitivity |
| `ui.locale` | `""` (empty) | Empty = follow the OS. `en` = built-in English. Anything else is valid only when `lang/<code>.toml` loads |
| `ui.showAdvanced` | `false` (optional) | Show Mode / Keys / Triggers / Search and General advanced items in Settings. Omitted / `false` = default simple view. Write to JSON only when `true`. Do not bump `schemaVersion` |
| `timeoutSec` | `30` | Idle timeout in typing mode (seconds). Does not apply to Search mode |
| `maxDigits` | `10` | Maximum digits in multi-digit mode |
| `feedbackMs` | `500` | Launch-accepted feedback duration (ms). How long to delay closing the typing window. `0` disables (§8 success notification) |
| `logLevel` | Code default: `warn` / shipped example: `info` | `error` / `warn` / `info` / `debug` |
| `docsUrl` | (shipped URL) | Documentation site opened from tray Help |
| `autostart` | `false` | Start at sign-in via the HKCU Run key |
| `slotsFile` | `"slots.json"` | Slot JSON filename in the settings folder (basename only). **Empty string** = read `slots.example.json` read-only (do not overwrite it). Personal books are `slots.json` / `slots.work.json` and the like. **Switch the current book with tray Mode / X1 → digit**. Settings → Mode (**Advanced**. Combo / row double-click) also works. The General combo is the `slotsFile` pin (Advanced. Distinct from the Mode book combo). UI shows `meta.displayName` when present, otherwise basename |
| `backupFolder` | `""` (optional) | Destination for JSON backups. **Empty = Desktop** (resolved at run time. Portable builds do not store an absolute default). Settings → General. Paths inside the app / settings folder are rejected |
| `modeSwitch.mouseButton` | `"x1"` | Mode-switch leader. **Must not be the same button as the launch mouse** (warn on load and disable the mode side) |
| `modeSwitch.suppress` | `true` | Do not pass the leader press to other apps (default. Suppresses browser Back) |
| `modeSwitch.chordTimeoutMs` | `2000` | After the leader, wait this many ms for a digit / letter. `0` disables the mode-switch mouse |
| `modeSwitch.keys` | `{}` (`0` is shipped `slots.json`) | Optional overrides. Keys are `"0"`–`"9"` (one character). Even when empty, `0` is reserved for `slots.json`. Other books enumerate from `1` |
| `legend` | See below | Typing-window subcommands: `order` (5 items) and `hidden` for **Idle / Narrow down (multi)** each. If omitted, Idle = Trigger→Search→Cancel (Open/Back hidden), multi = Open→Search→Back→Cancel (Trigger hidden). ids are `start` / `openWorkdir` / `search` / `digitBack` / `cancel` |

### slots.json

```json
{
  "schemaVersion": 1,
  "meta": {
    "displayName": "Home"
  },
  "slots": [
    {
      "id": "11",
      "name": "Cursor",
      "description": "Code editor",
      "path": "C:/Users/xxx/AppData/Local/Programs/cursor/Cursor.exe"
    },
    {
      "id": "31",
      "name": "Patrol log split",
      "description": "Split monthly patrol logs by sheet",
      "path": "C:/tools/split_patrol.bat"
    },
    {
      "id": "91",
      "name": "Docs",
      "description": "Project documentation",
      "path": "https://example.com/docs"
    }
  ]
}
```

| Field | Required | Use |
|---|---|---|
| `meta.displayName` | — | **Book display name** (tray Mode, Slots file combo, and so on). **Not edited in the Settings UI** (edit the JSON directly). Unset or empty = show the filename (basename) as-is |
| `id` | Yes | Slot number. **String**. Leading zeros are preserved (`"01"` ≠ `"1"`). **A Search target** |
| `name` | Yes | Slot display name. A Search target |
| `path` | Yes | Executable / script / **document** / **folder**, an **`http://` / `https://` URL**, or a **special command that starts with `chain:`**. Path strings and URLs are also Search targets |
| `description` | — | Description. Search target and list display |
| `workdir` | — | Explicit working directory. **If empty, use Path's parent folder (after resolution)**. **Ignored on URL / chain slots** |
| `openWorkdir` | — | When `true`, open the working directory in Explorer at the same time as launch (Path's parent if empty). Default `false`. **Disabled on URL / chain slots** |
> **When `path` is a URL**: schemes `http` / `https` only. Scheme-less hosts such as `www.example.com`, and `ftp://` and the like, are out of scope (treated as a normal path and launch fails).  
> **Documents and folders**: open via the shell association (§6 launch options). A file on the Desktop is written as `%USERPROFILE%\Desktop\aaa.xlsx`.

### slots.example.json (shipped sample)

First-run guide. Shows launch patterns (app / URL / `.bat` / folder / **chain** / **date tokens**).

```json
{
  "schemaVersion": 1,
  "meta": {
    "displayName": "Example"
  },
  "slots": [
    {
      "id": "1",
      "name": "Calculator",
      "description": "Launch an app — Windows Calculator",
      "path": "calc.exe"
    },
    {
      "id": "2",
      "name": "Example.com",
      "description": "Open a website — swap later for DialKey docs URL",
      "path": "https://example.com"
    },
    {
      "id": "3",
      "name": "Hello BAT",
      "description": "Run a .bat — samples/hello.bat",
      "path": "samples/hello.bat"
    },
    {
      "id": "4",
      "name": "Samples folder",
      "description": "Open a folder — samples/ next to the exe",
      "path": "samples"
    },
    {
      "id": "5",
      "name": "Demo chain",
      "description": "Advanced — launch slots 1 and 3 together",
      "path": "chain:1,3"
    },
    {
      "id": "6",
      "name": "Date-token report",
      "description": "Date tokens — {yyyy}/{yyyyMM} folders; missing path opens deepest existing ancestor",
      "path": "D:\\work\\{yyyy}\\{yyyyMM}\\report_{yyyyMM}.xlsx"
    }
  ]
}
```

Setting `1` as an instant-fire example makes the feature obvious.
The URL stays `example.com` for now. It may be swapped for the official GitHub Pages
documentation (the same URL as `docsUrl`) once that is published.
The folder example **writes a folder path directly in `path`** (shell association). Fixed arguments belong in a `.bat` / `.ps1` (DialKey only launches).
`5` is an advanced `chain:` example (`+` → `5` → Enter launches 1 and 3 together).
`6` is a date-token example (expanded to the launch-day date. If the path is missing, walk up to the deepest existing ancestor. Folders are not created automatically).

### schemaVersion (schema version)

**Separate from the app version. This is the version of the JSON structure itself.**

After a public release, each user keeps a `slots.json` on disk. A later structure change that
cannot read the old file would **wipe that user's registrations**. This field prevents that.

**Rules**

| Situation | Behaviour |
|---|---|
| **Add** fields only | Do not bump the version |
| **Change** the structure | Bump the version and add a migration |
| Loaded version is **older** | **The current schema (schemaVersion = 1) has no migration** |
| Loaded version is **newer** | **Do not overwrite. Show an error and stop** (prevents destruction on downgrade). Same for `settings.json` and **the currently selected slots file** |
| **Unknown fields** | Ignore them and keep reading (forward compatible) |
| `schemaVersion` is **missing / not a number** | **Warn and treat as `1`** (do not overwrite. A broken file is left untouched, as before) |

**`dialkeyVersion`:** written as the app version (`Cargo.toml`) at save time. May be absent on older files. JSON written by another app version is still read when `schemaVersion` is in range (log only).

**Current**: `"schemaVersion": 1` only.

### Abnormal cases

| Situation | Behaviour |
|---|---|
| JSON is broken | **Do not overwrite.** **Log it** and continue with an empty slot set. A tray notification is allowed but not required |
| Duplicate slot numbers | **First wins.** Warn in the log |
| `path` does not exist | Warn at registration. On launch failure, toast + log. **Do not auto-delete**. **URLs (`http://` / `https://`) are not existence-checked** |

> Never overwrite a broken file. Keep the file in a state the user can repair by hand.

### Write method (atomic write)

When saving from the Settings UI, **prevent a crash mid-write from corrupting the file**.

1. Write a temporary file (`settings.json.tmp`)
2. After the write completes, flush to disk with the equivalent of `fsync`
3. **Replace by rename** (rename is atomic)

Apply the “do not break files” principle to file writes as well.

### Character encoding

`slots.json` may contain non-ASCII tool names (for example Japanese), so the encoding is defined explicitly.

| Direction | Handling |
|---|---|
| **Read** | **UTF-8. Accept with or without BOM** |
| **Write** | **UTF-8 without BOM** |

> Windows Notepad may save with a BOM.
> An implementation that ignores BOM will fail to read.

### First run

When the settings folder has **no personal `slots*.json` at all**:

1. Copy `slots.example.json` to create `slots.json`
2. Open Settings

If another book already exists (for example `slots.NDV.json`), do not create `slots.json`.

> Do not dump the user into an empty state.

Missing fields such as key display names use the same defaults as the §6 shipped example. Do not overwrite a broken file. Manual JSON backup is General → Backup (destination is configurable; empty = Desktop. Only JSON directly under the settings folder, into `yyyyMMddHHmm/`).

### Reload

Reload settings from the tray menu **Reload**, or from the CLI **`--reload`**.
(Settings are read once at startup, so there must be a way to apply hand edits. With no resident instance, `--reload` exits with code 1.)

**If a window is open when Reload runs, close the window first, then reload.**
(Prevents settings from swapping while an in-progress input state is held.)

**Settings Apply** does not close the Settings window. If a typing session is open, discard it the same way as Reload before rereading settings (prevents Idle and multi-digit displays from disagreeing). The Search window may stay open.

### Launch options

| Kind | How it launches |
|---|---|
| `.bat` / `.cmd` | `cmd /c "<path>"` |
| `.ps1` | `powershell -ExecutionPolicy Bypass -File "<path>"` |
| `.exe` | Direct launch (a bare filename on PATH is allowed) |
| Folder | **Open via the shell association** (Explorer and the like) |
| `.ahk` / documents (`.xlsx` / `.pdf` / `.txt` and the like) | **Open via the shell association** |
| `http://` / `https://` | **Open in the default browser**. Path resolution and working directory do not apply |
| `chain:` | **Special command (advanced).** IDs after `chain:`, comma-separated (spaces allowed). Launch sequentially in written order. Nesting is one level only. Continue the rest after a failure. Do not consult the target's instant. No path resolution, date tokens, `workdir`, or existence check (they remain Search targets). Cycles cannot be saved. Missing IDs warn. A reference that disappeared at run time is a failure notification |

Relative paths are resolved from the folder that contains the executable (§6 path-resolution rules).

| Writing | Meaning |
|---|---|
| `%USERPROFILE%\Desktop\aaa.xlsx` | A file on the user's Desktop (recommended) |
| `files\aaa.xlsx` | `files\aaa.xlsx` next to the exe (portable) |
| `desktop\aaa.xlsx` | **The `desktop` folder next to the exe** (not the Windows Desktop) |

**Working directory (`workdir`)**: if empty, use Path's parent after resolution. **Does not apply to URL / chain slots.** If there is nowhere for Explorer to open, skip the open and log only (do not show a Windows error dialog).

Fixed arguments belong in a **`.bat` / `.ps1`**. Slots have no `args` field (matches the duty to launch and release control).

### Focus-existing (best effort)

**Once**, before launch, look for an already-open window. If found, bring it to the front instead of starting a new process.

| Item | Decision |
|---|---|
| Timing | Immediately before launch only (no timer loop or process wait) |
| `.exe` | A visible top-level window whose process image path matches |
| Documents, folders, and the like | Window title contains the filename (or stem length ≥ 3 if there is no filename) with a **token boundary**. Browser-class windows are excluded from document matching (table below) |
| Tabbed apps (Excel and the like) | Depends on title match. **Not guaranteed** (if not found, launch as usual) |
| URL / `chain:` | Out of scope (no focus-existing) |
| Per-slot opt-out | **`focusExisting`** (each slot in `slots.json`. Default `true` when omitted). When `false`, skip the scan and launch as usual. Settings → Slots checkbox. **A global off is not adopted** |
| On failure | Fall back silently to a normal launch (success is info on the launch side. Scan time is `scan_ms` at `logLevel=debug`) |
| Weight measurement | §10 (n=10 before a version-bump PR). **A PR that changes focus-path behaviour records an A/B measurement before merge** |
| Out of scope | COM/ROT or Excel-specific code, process tracking to suppress multiple instances, always-on polling |

#### Title boundary (reduce false focus)

`needle` (filename, or a stem of sufficient length) matches only when it has a **token boundary** in the title.

- Boundary: start or end of the string, or a character **other than** `A–Z` / `0–9` / `_` / `.`
- Example: `a.xlsx - Excel` → match. `a.xlsx.bak` / `b_a.xlsx` → **no match** (avoid embedded false positives)
- A filename token in a space-separated title can still match → exclude browsers with the **class deny** list

#### Window classes denied on document match

Canonical implementation: `is_document_focus_denied_class` in `src/core/focus_match.rs`.

| Class name | Typical host |
|---|---|
| `Chrome_WidgetWin_1` / `Chrome_WidgetWin_0` | Chrome / Chromium family |
| `MozillaWindowClass` | Firefox |
| `OperaWindowClass` | Opera |
| `ApplicationFrameWindow` | Edge / Store family (broad. Avoids false focus on UWP Edge tabs) |

Office (for example `XLMAIN`) and Explorer (`CabinetWClass`) are **not** denied. Add a class if false focus appears in real use.

#### Slot `focusExisting`

```json
{ "id": "12", "name": "Excel book", "path": "%USERPROFILE%\\Desktop\\a.xlsx", "focusExisting": false }
```

| Value | Meaning |
|---|---|
| Omitted / `true` | Try focus-existing as today (skip spawn on a hit) |
| `false` | No EnumWindows scan. Always a normal launch (for example when a tabbed app should start another instance) |

Do not put this on the `instant` map in `settings.json`. On JSON save, `true` may be omitted.

This is not about drawing DialKey's pale-yellow UI over exclusive fullscreen. It only **brings an already-open target to the front**.

### Path-resolution rules (required for portable use)

**If `path` starts with `http://` or `https://`, do not resolve it.** Open the string as-is in the browser.

For every other `path` and for `workdir`, resolve in this order.

1. **Expand date tokens** — braces only (`{yyyy}` / `{yy}` / `{MM}` / `{dd}` / `{HH}` / `{min}` / `{yyyyMM}` / `{yyyyMMdd}`). Minutes are not `{mm}`. `path` / `workdir` only. Not applied to URL / `chain:`. Missing levels walk up to the deepest existing ancestor. Folders are not created automatically
2. **Expand environment variables** — `%USERPROFILE%`, `%APPDATA%`, `%ProgramFiles%`, and the like
3. **If absolute, use as-is**
4. **If relative, resolve against the folder that contains the executable (DialKey.exe)**

On launch, existence check, and folder open, Windows prefixes absolute paths / UNC that exceed **MAX_PATH (260)** with `\\?\` / `\\?\UNC\` (do not write that in JSON. Save the input as-is). The manifest is `longPathAware`. Documents / folders use a shell execute, not `cmd start`. Not in Help.

> **Without this, every slot breaks when the app is carried on a USB stick.**
> Portable distribution requires relative-path support.

```json
{ "id": "31", "name": "Patrol Split", "path": "tools/split_patrol.bat" }
{ "id": "91", "name": "Docs", "path": "https://example.com/docs" }
```

---

## 7. Settings UI

### Tab layout

```text
Default:  General | Slots
Advanced: General | Slots | Mode | Keys | Triggers | Search
```

The initial tab on open is **Slots** (not General on the left). The tab name **Keys** stays. **Do not add tabs** (defaults are enough. Even under Advanced, items go on existing tabs. Spare space is for the Advanced checkbox). **The current tab stays pressed** (radio. Re-clicking the same tab does not deselect it). **Advanced** checkbox on the right (`ui.showAdvanced`). Off by default.

Vertical spacing inside a tab is shared: label → control below it is tight; **row-to-row gap is the same** (Action keys and Displayed options checkboxes use the same row gap).

| Tab | Content | Visible by default |
|---|---|---|
| **Slots** | Slot list and editor. At the top, **the book being edited** (display name. `1 · Example` if it is on 0–9). **Instant fire** checkbox (enabled only when the ID is exactly `"0"`–`"9"`. Data is `settings.instant`, not the slot JSON) | Yes |
| **Keys** | At the top, **key set**▼ (numpad / main keyboard. **Edit target** = which set's Action keys Capture applies to). Action keys Capture. Each row is **Caption \| Trigger (VK name) \| Display (max 3; empty = default short name) \| Capture** (one row. Leave space between the caption column and Trigger, and between rows, so rows do not overlap). **ON** checkbox + **ON Capture** on **both** sets (off by default. When on, swallow the chosen VK while the window is shown. No combo). Each set stores its own ON; switching the combo must load that set’s checkbox (do not copy the HWND state). Then **Reset this key set to defaults** (this set's Action keys / ON / that set's letter only. Do not touch the launch mouse). **Capture guidance text sits under the Displayed options group**. No gap between Action keys and Displayed options. Displayed options: Idle / Narrow down checklists + Move up / Move down (visibility and order. Does not change key behaviour. JSON is `legend`) | Advanced |
| **Triggers** | At the top, **Switch triggers** (leader mouse default x1 / suppress / `chordTimeoutMs` / per-set letter▼ a–z. 0–9 = books, letters = key sets). Below that, **Launch triggers** (launch mouse / suppress / hotkey / **Reset triggers to defaults** = launch + switch mouse / timeout / letters `t`/`k`. Do not touch book `modeSwitch.keys` assignments) | Advanced |
| **Search** | Match mode / case sensitivity | Advanced |
| **Mode** | **Current-book combo** (assigned 0–9 only, `code · display name`) / **single list: codes 0–9, filename, display name (tab-aligned. Height shows 10 rows. Do not stretch with empty space. Scroll if overflow) + book combo + Assign / Unassign + status line**. No ✔ in the list (the current book is the Slots header line and tray Mode). The blue row is for Assign/Unassign. Row double-click = switch the current book (empty row does nothing). **Do not edit the book display name (`meta.displayName`) here** | Advanced |
| **General** | **Default:** UI language / **autostart** / **Open settings folder** / **Backup folder** (Browse + Backup + Load). Backup status line is **under** the group (same as Keys Capture guidance. Do not leave a 40px frame when empty). **Added under Advanced:** **Slots file** (`slotsFile`) pin, timeout / max digits / `feedbackMs` / log level | Tab always. Items as above |

**Do not persist window size.** Each open uses a content preferred size, then clamps to the monitor work area (`fit_settings_window_to_content`). **Height is the max of the tabs currently visible** (with Advanced off, only Slots and General default items). Do not save a user-shrunk size into settings.  
**Tabs are scrollable** (keep a compact outer frame; Mode and similar must not clip when items grow).

When Advanced is turned off and the current tab is Mode / Keys / Triggers / Search, **return to Slots**. Values stay in JSON (hidden only). Book switching stays on tray Mode. Triggers “Reset to defaults” is on the Advanced side (Keys has its own key-set reset). The checkbox stays visible, so there is always an exit path.

### Key reassignment (gamepad style)

**Remember the key that was pressed.** The user does not need to know virtual-key codes.

**Required validation**

| Check | Content |
|---|---|
| Mutual exclusion | Start / Confirm / Cancel / Search / **open working directory** / **digit-back** must not share a VK. **Capturing the same VK onto another role swaps them** (gamepad style). Do not swap Display text; **overwrite both with the VK short name** (max 3 characters, `Enter`→`Ent`). **When ON is on**, do not give that VK to another role (reject; no swap. If Esc is set as ON, move Cancel first) |
| Display clash | Reject identical display strings (blank is treated as the VK short name). Short names that collide across sets, such as numpad `+` and main-keyboard `+`, are rare |
| Reserved keys | Digits 0–9 cannot be assigned |
| Esc | **Assignable as Cancel.** Not used to abort Capture |
| Abort Capture | That row's **Cancel** (label is Capture→Cancel only while waiting) / Capture on another role / close Settings. No timeout. Apply/Set does not abort |
| Display focus | Keys in that interval are not passed to Capture (neither assign nor abort) |
| Mouse buttons | **Left-click and right-click are excluded** (Settings would become unusable) |

**Disable triggers while Settings is shown (required)**

Because of key capture, **pressing the trigger button during setup opens the typing window and conflicts.**
Disable triggers while Settings is open (especially during key capture). The mode switch is also disabled (§4).

**Recovery**

- “Reset this key set to defaults” (Keys tab. Current set)
- “Reset triggers to defaults” (Triggers tab. Launch + switch. Book assignments stay)
- Edit the settings file directly → tray **Reload**
- **The tray menu is always alive** — an exit path to Settings even if a trigger is broken

### Slot editor field order

Field order on the Settings Slots tab is as follows.

**ID → Name → Path → Working directory → Also open working directory → Instant fire → Description → error area**

- The UI label is **“Working directory”** (the Japanese pack may say the local equivalent). Do not use the abbreviation Working
- **Labels sit above each field** (a side-by-side fixed width would clip). ID and Name share one row; Path and after are full form width
- Empty edit fields show a placeholder (cue banner) for purpose. Placeholders are not saved as values
- An empty **Working directory** = automatically use Path's parent (same as `workdir` above). The placeholder (cue) also states that default. Do not save it as a value
- **Drop onto Path / Working directory.** Fill only the field that received the drop. Do not expand or insert date tokens. Path is the full path of a file / folder / exe (`.lnk` after resolve, `.url` as `http(s)`). **An unresolvable `.lnk` / `.url` does not rewrite the field** (do not insert the shortcut's own path). workdir is the folder itself, or the parent for a file / app. Dropping a URL onto workdir does not rewrite it. Multiple items: first only. Not in Help
- **Also open working directory** is `openWorkdir` (disabled on chain / URL). It can be set even when the field is empty (then Path's parent is opened)
- **Instant fire** is editable only when the ID is exactly `"0"`–`"9"` (disabled for `"01"` / multi-digit / empty / read-only). Synced with `settings.instant`. Not a slot field
- Description is multi-line (easier to write a Search-target description)
- **Footer bottom-aligned (Slots)**: the 7 buttons **Add / Delete / Swap IDs / Update slot / Save / Apply / Cancel** share one baseline at the window bottom. Leave space between the left 4 and the right 3. **The gap between Update and Save has the same minimum as the other buttons** (if tight, pack all 7 left. Minimum width follows the 7-button row). The left list's bottom aligns with the bottom of Description + error area (do not stretch the list to fill the panel). Overflow slots scroll the list
- Only when there is an ID clash (`1` and `01` both registered), show a short warning in the error area under Description
- Leave top inset inside a group box so the heading does not overlap the first control (example: Keys Start)

### General: slots file and settings folder

- The **Slots file** combo selects `slotsFile` (empty = sample, read-only)
- To the right of the combo, **Open settings folder** (where `settings.json` / `slots*.json` live). Not the parent of the selected file. The path to book display-name edits and direct JSON edits
- **Backup folder** row: read-only path + **Browse** + **Backup** + **Load**. Empty `backupFolder` is the Desktop. Copy only JSON directly under the settings folder (`settings.json` + `slots*.json`). Into `yyyyMMddHHmm/` at the chosen location (same minute replaces). Paths inside the app / settings folder are rejected. Result is the status line under the row (Backup has no dialog). **Load** picks a stamp folder, then picks the scope on the spot (all books / current book only, optionally `settings.json`. Default is all books, no settings) and **copies into the current settings folder**. It does not keep using the stamp as the live source (reads stay `{exe}/settings/`. `backupFolder` is Backup's write destination only. Load's read source is chosen each time). Same-name overwrite, so **Mode list code order does not change** (`0` = `slots.json`, others follow `modeSwitch.keys` or filename order. To restore assignments, include `settings.json` with Restore). Do not write `slots.example.json`. If the current book is the sample, “current book only” cannot be chosen. After success, reopen Settings on General (reread from disk)

### Other settings items

- Slot management (add / edit / delete). `path` can register executables / scripts / documents / folders plus **`http://` / `https://` URLs**
- **Slots file** (`slotsFile`. General) and the settings-folder button (above)
- Instant fire on/off per digit (Instant fire on the Slots tab. Data is `settings.instant`)
- Search match mode and case sensitivity (Search tab)
- Mode-switch trigger (Triggers tab **Switch triggers**. §4 · §6). Book assignment is the Mode tab
- Timeout seconds (General · Advanced)
- **Launch-accepted feedback duration** (`feedbackMs`. Default 500 ms, `0` disables. General · Advanced)
- Log level (General · Advanced)
- **UI language** (`ui.locale`. General default)
- Autostart on/off (General default. Immediately after language)

---

## 8. Settled behaviour

| Item | Decision |
|---|---|
| Success notification | **Show only that the launch request was accepted, briefly on the typing window. Do not follow success or failure after launch** (a slow-starting app does not make “you will see it come up” true, so it looks idle and the user launches again). This is **acceptance, not success**, and control is released after launch (§1 responsibility split). No process tracking or polling. The implementation **only delays closing the typing window**; it does not create a new window. Delay is `feedbackMs` (default **500**, `0` disables). Draw the acceptance line (`Launching {id} — {name}`) **before** spawn / `ShellExecute`. Do not leave `+10` on a slow association. Do not close the window before launch work. The actual spawn / `ShellExecute` / existing-window scan **does not wait on the UI thread** (dedicated worker). Search accepts a launch once, closes the window, then hands off to the worker |
| Failure notification | **Toast + log** (the log remains if Focus Assist hides the toast. No dedicated handling). **While the typing window is open**, change the acceptance line to the failure wording (do not treat it as success). **Search** closes the window and relies on the toast |
| Unregistered number | Show an error → release the trigger |
| Idle input timeout | **Default 30 seconds**, changeable. **Typing mode only. Does not apply to Search mode** (disappearing mid-search is a problem) |
| Window position | **Fixed offset per window (§9 Window display)**. Centring on the cursor is not adopted |
| Multi-monitor | **Typing and Search = primary** work area. **Settings = work area of the monitor that has the cursor** |
| High DPI | **Enable DPI awareness and scale the UI by the DPI factor** |
| Autostart | **Registry Run key** (with self-repair) |
| Settings location | **`{exe}/settings/`** (`lang/` `logs/` sit next to the exe) |
| Single instance | **Single instance** (Mutex). **A second instance shows the first instance's window, then exits** (matches user intent) |
| Candidates while typing | **Show them** (from the first digit, immediately. Display only; no selection) |
| When candidates narrow to one | **Do not auto-run.** Always confirm with Enter |
| `+` pressed again in multi-digit mode | **Cancel** |
| Key auto-repeat | **Ignore** |
| Instant-fire default (code Default) | **All `false`** (safe side) |
| Shipped `settings.json` example | **`instant["1"] = true`**, `logLevel`: `info` (first-run experience. The user may change it) |

### High DPI

**Implemented from the first edition.**
Without it, windows blur or become extremely small on high-DPI displays.
**Adding it later touches coordinate math everywhere, so it is done from the start.**

### Launch-failure detection scope

**Only a failed process launch can be detected** (missing path, missing permission, and the like).

Because the design does not wait after launch, **errors and abnormal exits inside a script cannot be detected**.
**The script itself is responsible for showing its errors.**

### Log specification

| Item | Decision |
|---|---|
| Destination | `logs/` next to the executable |
| Rotation | 1 MB per file, 3 generations |
| Level | Default `warn` and above. Settings can raise to `info` / `debug` |
| Never recorded | **Never write keystroke contents.** The slow-launch `warn` below does not include a path |
| Slow launch | If worker-side spawn / shell / existing-window scan takes **250 ms** or more, `warn` (duration and success/failure. No path). Ordinary duration is `debug`. The hook-neighbourhood budget (`LowLevelHooksTimeout` ≈ 300 ms) is defended first by **not blocking the UI thread** |
| Fallback | If the destination is **not writable**, `%LOCALAPPDATA%\\DialKey\\logs\\`. If that is also unusable, give up on logging and continue to start |
| Implementation | `tracing` + `tracing-appender` |

### Display over fullscreen apps

| Situation | Behaviour |
|---|---|
| Borderless fullscreen | Displays without a problem |
| **Exclusive fullscreen** (games and the like) | **May be unable to draw on top** |
| Focus Assist | **Toast notifications may be suppressed** |

> Even in exclusive fullscreen, **the keyboard hook still works, so number entry and launch still succeed when the window is invisible**.
> The design assumes blind operation, so practical harm is small (do not put this in in-app Help; use the docs site / FAQ if needed).
> The log still records events when toasts are suppressed.

---

## 9. UI

### What is built

| UI | Content |
|---|---|
| Typing window | Primary key row + number / candidate list. `NOACTIVATE` + `TOPMOST`, compact. Look is **Window display** below |
| Search window | Search field + list. A normal focused window. Hint under the list: **`{key}: open folder`** (**1.0.0**. `keys.openWorkdirLabel`, default `Ctr`) |
| Tray menu | Run DialKey / Search / Settings / **Mode** / **Reload** / Help / Quit |
| Settings | Key reassignment, slot management, mode switch, other settings (§7) |

### Window display

Canonical position and look. Content per input state is §2.

#### Position

Offsets are a **small gap** from the work-area edge (about 16–24 px @96 DPI, scaled by DPI). Not a settings item.

| Window | Monitor | Anchor | Window corner used |
|---|---|---|---|
| **Typing** | **Primary** work area | From the top-right, a little down and left | **Window top-right** |
| **Search** | **Primary** work area | From the bottom-right, a little up and left | **Window bottom-right** |
| **Settings** | Work area of the **monitor that has the cursor** | From the top-left, a little down and right | **Window top-left** |

Typing and Search may overlap. Clamp so they stay inside the work area.

#### Typing window

| Item | Decision |
|---|---|
| Background | Pale sticky-note yellow **`#FFF9C4`** (distinct from other windows) |
| Branded hint | **Do not show** (`DialKey — instant \| …` and the like are withdrawn) |
| Alignment | **Digits, keys, and titles are left-aligned** (measure the key-column width at draw time and align the title column) |
| Primary key row (bottom, this order) | Under the `--------` separator. Follows the **Settings → Keys Idle / Narrow down lists** (default Idle: Trigger → Search → Cancel. Default Narrow down: Open → Search → Back → Cancel). Hidden items still **reserve 5 rows of height**. The key column is Settings → Keys Display (empty = VK short name). Cancel short name is **ESC**; Search default short name is **`\`**. Legend labels are initial-capped (Trigger / Open / Back / Search / Cancel) |
| Body | **Top = dial / candidates, bottom = subcommands**. The dial is **1 header line (empty in Idle. Multi is Start + buffer: `+` / `+1` / `+10`) + 10 decade lines**. Idle decade is **0–9**. Multi-digit is `{prefix}0`–`{prefix}9` (unregistered still shows the number). Window height is these 11 lines |
| After the multi-trigger | Start + buffer on the first line (`+` if empty; `+10` with digits). Decade below. No brand comment line |

Text colour is a dark readable colour on the pale yellow (black to dark grey).

#### Search and Settings

- Search and Settings keep the previous palette (system / white). Only the typing window is pale yellow
- Settings keeps roughly the current compact outer frame; in-tab scroll prevents clipping (§7)

**Tray icon actions**

| Action | Behaviour |
|---|---|
| Left-click | Open Search |
| **Double-click** | **Same as single-click** (no separate action. Avoids mistakes) |
| Right-click | Show the menu |

**Icon restore after Explorer restart (required)**

If explorer.exe crashes or restarts, **a resident app's tray icon stays gone**.
The app itself must restore it.

1. At startup, obtain the message ID with `RegisterWindowMessage("TaskbarCreated")`
2. When the window procedure receives that ID, **re-add the icon** with `Shell_NotifyIcon(NIM_ADD)`
3. Keep registration data in memory so re-add is a few lines

**Also implement**

- **Retry `Shell_NotifyIcon` on failure** — registration can fail just after the shell starts. Retry several times at a few-hundred-ms interval
- **Log the fact that a re-add occurred**

> ⚠ If a crate such as `tray-icon` / `trayicon` is used,
> **confirm that this message is handled inside it.**
> Assuming “it should be handled” is the most dangerous path. If it is not, implement it locally.

### Icon

**Telephone dial-pad motif.** Adjacent tiles do not share the same colour.

| Use | Design |
|---|---|
| Tray (16/32px) | **Simplified 2×2 or 3×3 grid**. One cell in an accent colour. One or two colours |
| App (256px) | **Full 3×4 dial pad**. Multi-colour |

> A 3×4 grid is unreadable at 16×16px, so there are two densities.
> The motif stays the same; only the amount of detail changes.

---

## 10. Implementation

### Language

**Rust.**

| Aspect | Reason |
|---|---|
| Weight | No runtime, single binary, small memory footprint |
| Resident fit | No irregular GC pauses |
| FFI | Direct Windows API (`windows-rs`) |
| Portability | Core stays OS-independent; platform traits keep the architecture portable in shape |

### Layout: daemon + on-demand UI

| State | Memory |
|---|---|
| Resident (no UI) | **Goal: Private Bytes in the low single-digit MB.** Hook and event loop only |
| Shown | Create the window. Destroy it on close |

> Holding a GUI-framework context resident adds tens of MB,
> so **windows are created and destroyed on demand to keep the resident small.**

**Version-bump PR gate (n=10):** before opening a PR, measure startup (ready) and resident footprint (Private Bytes canonical, Working Set also recorded) at **n=10** (one warmup is not counted) and append to the tables in this section. Keep **mean, unbiased standard deviation \(\hat\sigma=\sqrt{\mathrm{SS}/(n-1)}\), min–max, and the raw sample list**. Keep old rows. The goal is that Private stays in the low single-digit MB. When comparing builds: Shapiro–Wilk (normality), F-test (equal variance), and mean difference (Student t if variance is not rejected, otherwise Welch). Significance level 0.05. Focus-path changes also keep the existing A/B. Scripts: `scripts/measure-startup.ps1 -Runs 10 -Footprint` (prints σ̂). Comparison: `scripts/measure-stats.ps1`.

#### Startup time measurements (append per version)

**Definition:** wall-clock milliseconds from process start until the log line `ready … tray / mouse / hotkey triggers active`.  
Release build, tray resident only (do not open Settings / the typing UI). After one warmup, **n=10** for mean and **unbiased σ̂**.

| App version | Date | Mean | σ̂ (unbiased) | Min–max | Conditions |
|---|---|---|---|---|---|
| **1.0.0** | 2026-09-12 | **59.1 ms** | **15.8 ms** | 45–92 ms | n=10. Same machine, release, isolated `_measure/v100`. samples=[67,64,76,47,59,47,48,45,46,92]. Search folder-open (tap-alone) is a Search-shown-only code path and is not on the resident path |
| **1.0.1** | 2026-09-13 | **115.1 ms** | **33.0 ms** | 76–174 ms | n=10. Same machine, release, isolated `_measure/v101`. samples=[171,174,121,109,90,95,94,116,105,76]. Confirmed `DialKey 1.0.1` (`--version` / log). Numpad-only ON is Settings-only and is not on the resident path |

**Procedure**

1. Exit any existing `dialkey.exe`
2. Copy that version's release binary into a writable **isolated** folder (do not hit `dist/stage` directly. `{exe}/settings/` has `settings.json` / `slots.json`. `logLevel` is `info`. Missing slots opens Settings and inflates the numbers)
3. Warm up once and exit (not counted)
4. Repeat **10** times: delete the log → start → record ms until the ready line in `logs\dialkey.log` → exit (`scripts/measure-startup.ps1 -Runs 10`)
5. Append the 10-run arithmetic mean, **unbiased σ̂**, and raw samples to the table above (keep old rows. Do this before a version-bump PR)

#### Resident footprint measurements (append per version)

**The canonical metric is Private Bytes** (commit owned by this process).  
Task Manager's “Memory” column leans toward Working Set (shared DLLs included), so a mid-teens MB reading is not by itself abnormal.

| App version | Date | Private Bytes | Working Set | Conditions |
|---|---|---|---|---|
| **1.0.0** | 2026-09-12 | **3.94 MB** (σ̂ **0.0095**) | **11.72 MB** (σ̂ **0.0114**) | n=10. Isolated `_measure/v100`. Private samples=[3.95,3.93,3.93,3.95,3.93,3.93,3.93,3.95,3.94,3.93]. WS samples=[11.72,11.71,11.72,11.72,11.71,11.70,11.71,11.73,11.72,11.74]. Search folder-open is not on the resident path |
| **1.0.1** | 2026-09-13 | **3.92 MB** (σ̂ **0.0053**) | **11.77 MB** (σ̂ **0.0074**) | n=10. Isolated `_measure/v101`. Private samples=[3.93,3.92,3.93,3.92,3.93,3.93,3.92,3.93,3.92,3.92]. WS samples=[11.76,11.76,11.77,11.76,11.78,11.77,11.77,11.78,11.77,11.77]. Confirmed `DialKey 1.0.1`. Numpad-only ON is not on the resident path |

**Procedure (same conditions on every version bump)**

1. Exit any existing `dialkey.exe`
2. Start the isolated-folder release (tray resident only. Do not open Settings / the typing UI)
3. Wait 5–10 seconds after start and take a stable counter. A version bump does this **10** times (exit the process each time and start again)
4. Recommended script: `.\scripts\measure-startup.ps1 -ExePath <isolated\dialkey.exe> -WorkDir <same> -Runs 10 -Footprint`
5. Append one row to the table above with **app version, date, Private, WS (mean and unbiased σ̂), and raw samples** (keep old rows). A version bump is **n=10**. Keep past single-sample rows as they are

PowerShell for a one-off check:

```powershell
Get-Process dialkey |
  Select-Object Id,
    @{N='Private_MB';E={[math]::Round($_.PrivateMemorySize64/1MB,2)}},
    @{N='WS_MB';E={[math]::Round($_.WorkingSet64/1MB,2)}}
```

**Rough judgment**

| Private Bytes (resident, no UI) | Judgment |
|---|---|
| **Low single-digit MB (under about 10 MB)** | **Small (goal met).** Matches the current design |
| Mid-teens to tens of MB | Investigate. Always-on UI / extra resident state / possible leak |
| Tens of MB as the normal state | **Large.** The daemon may be holding a GUI context. Suspect the on-demand destroy rule |

> Size context: Electron-style trays often sit in the tens to 100+ MB. An AHK resident can sit around 10 MB depending on the environment.  
> **DialKey's about 4 MB Private is on the light side**, and about 11 MB Working Set is reasonable including shared pages.

### Target platforms

**Windows only.**

| Environment | Role |
|---|---|
| Windows | **Implementation target** |
| Ubuntu (WSL) | **Development only.** WSL cannot capture host input, so DialKey cannot run there |
| Ubuntu (server) | Out of scope. A resident launcher is not needed |
| macOS | **Kept portable in shape.** Windows is the only implementation |

**Platform traits are in place so the architecture stays portable in shape.**

```
┌─────────────────────────────┐
│  Core (OS-independent)                    │
│  - Slot management / JSON I/O        │
│  - Number-sequence state machine             │
│  - Candidate filter / Search          │
│  - Process launch                      │
├─────────────────────────────┤
│  trait TriggerSource                 │
│  trait InputCapture                  │
│  trait TrayIcon                      │
│  trait Notifier                      │
│  trait AutoStart                     │
├─────────────────────────────┤
│  Windows impl  │  macOS (portable shape)    │
└─────────────────────────────┘
```

| Feature | Windows | macOS (portable shape) |
|---|---|---|
| Mouse-button capture | `WH_MOUSE_LL` | `CGEventTap` |
| Keyboard capture | `WH_KEYBOARD_LL` | `CGEventTap` |
| Global hotkey | `RegisterHotKey` | `RegisterEventHotKey` |
| Tray resident | `Shell_NotifyIcon` | `NSStatusItem` |
| Autostart | Registry Run key | LaunchAgent |
| Notification | Toast | `UNUserNotificationCenter` |

> A macOS implementation would need Accessibility permission, and signing plus notarization to distribute.

### Versioning

**`Cargo.toml` `version` is the single source of truth.**

- Embedded at compile time with `env!("CARGO_PKG_VERSION")`
- `--version`, Help, and About all read this value
- Windows executable information is filled automatically via `winres`
- **Managed separately from `schemaVersion`**

### Autostart (Run key) and self-repair

The registry Run key accepts **absolute paths only**, so a portable install breaks when the drive letter changes.

**Mitigation: on every start, compare the Run-key value with the actual path and rewrite it if they differ.**

One manual launch restores later autostart. The implementation is a few lines.

> Volume-GUID paths (`\\?\Volume{...}`) are not adopted; Run-key behaviour is environment-dependent and unstable.

### Command-line options

| Option | Behaviour |
|---|---|
| `--version` | Print the version |
| `--help` | Print the minimum in-app help |
| `--config <dir>` | Name the settings **folder** explicitly (there are two files, `settings.json` and `slots.json`, so the unit is a folder, not a file) |
| `--uninstall` | **Delete the HKCU Run key only.** Do not delete settings files. No confirmation prompt |
| `--launch <id>` | Launch a slot (for automation). **If a resident is running**, ask it via `WM_COPYDATA`; **if not**, read settings, launch once, and exit. No HTTP listener. `--config` applies only to the one-shot path (the resident path uses the settings the resident already holds) |
| `--list-slots` | Print registered slots as `id<TAB>name` to stdout (**on-disk settings**. Natural order). For automation that needs an ID list |
| `--reload` | Ask a resident DialKey to reload settings / slots (same as tray Reload). Exit code 1 if no resident |

**`--launch` exit codes**

| Code | Meaning |
|---|---|
| 0 | Launch request accepted (resident path enqueues. Process success is not followed. Same as §1 / §8) |
| 1 | Usage error / invalid ID (non-digits and the like) |
| 2 | **Unregistered slot ID** (stderr hint points to `--list-slots`) |
| 3 | Launch failed (one-shot spawn failure, or enqueue to the resident failed) |

Slot IDs are strings. `"01"` and `"1"` are different. Edit settings in the Settings UI or edit JSON directly with PowerShell and the like; while resident, use tray **Reload**.

### Privilege policy

**Run unelevated.**

- Triggers do not work when an elevated app (Task Manager, regedit, admin PowerShell, and the like) is in the foreground
- Everyday work apps are unelevated, so practical harm is small
- The UAC prompt (secure desktop) cannot receive input in any case

### Lightweight implementation notes

- Do not create UI while resident
- `WH_MOUSE_LL` returns immediately for anything that is not the target button
- **Install the keyboard hook only while the window is shown.** Always uninstall on close
  (manage it with RAII so it is released on exceptions as well)
- **Hook-timeout mitigation** (below)
- The keyboard hook takes only digits, the start key, confirm, cancel, and the Search key. Everything else is passed through
- Keep the hook body light. Post a message and do heavy work elsewhere
- JSON settings are read once at startup and held in memory
- No timer loop; fully event-driven
- **Do not wait after launching a process**

> Growing the settings surface does not change resident cost. Tens of fields are still a few KB.

### Low-level hook timeout (known pitfall)

If a low-level hook callback exceeds **300 ms by default**, Windows
**uninstalls that hook with no warning** (registry `LowLevelHooksTimeout`).

Once it is gone, input is no longer received, and the cause is hard to see.

**Mitigation**

- Do no heavy work inside the hook (post a message and handle it elsewhere)
- Install the keyboard hook only while shown, and always uninstall on close (RAII)
- **The mouse LL hook has its own thread** (its own message loop). It must not drop because the UI thread is blocked in `ShellExecute`
- **Mouse LL hook health**: record callback arrivals; if the cursor is moving but no callback arrives for a while, reinstall the hook. Log the fact. Interval is **a few seconds** (about 5 seconds). Not a busy loop. Second defence when another process's slow hook caused the drop
- The keyboard hook is newly installed at the start of each typing session, so reinstall outside a session is mainly the resident mouse hook

> Timeout uninstall is a real problem. Keeping the hook body light is the first defence. Reinstall is the second.

---

## 11. Release and distribution

### Licence

**MIT.** Not sold. Includes the no-warranty clause.

### Signing and distribution

No signature. SmartScreen is handled by the README steps (More info → Run anyway). SHA256 is published.
Distribution is a portable zip.

### Help

**Roles (Help = end user; this specification = current behaviour).**
The model is **specification ⊃ Help** (Help / the docs site are projections of the specification).

| Place | Content |
|---|---|
| **In-app Help** (tray) | Version, **shortest how-to**, the door to slot registration, and a pointer to the documentation site. No policy, implementation notes, or edge-case essays |
| **Site** (GitHub Pages) | Install, SmartScreen, configuration reference, slot examples, FAQ, troubleshooting (explanations that a public release needs or recommends) |
| **This specification** | Design, invariants, implementation detail (for developers) |

- Tray **Help**: About dialog. Version (when a language pack is active, `Language pack: …` may be shown as well), three how-to points + Settings → Slots, shortest mode switch (X1→0–9, tray Mode), and that an accepted `+` is visible on the first line. Wireless pad: **one paragraph on why to teach ON** + check whether the key registered in the typing window (which pad to buy is FAQ. Copy matches the English original: teach ON in Settings → Keys). One line on `chain:`. One-line warning that **Load copies into the settings folder and overwrites** (does not keep using the stamp). Open the site with Yes/No only when `docsUrl` is set (OK only if empty)
- `--help`: minimum CLI help (English. Not a language-pack target)
- **Not in Help**: `slotsFile` field name, date-token table, excluded-app list, **focus-existing / `focusExisting` detail**, Backup / Load **steps** (scope picking and the like), `schemaVersion` / `dialkeyVersion`, Settings → Mode (Advanced), explanation of the Advanced checkbox, design rationale, **which wireless numpad to buy** (BT vs 2.4G, what to purchase. → FAQ). Keep the typing-window check when wake-from-sleep is slow, and add the operational reason to teach ON (Capture steps are FAQ)
- **Where Load writes (user-facing)**: **copy** into the settings folder. Do not keep using the stamp. Help is the overwrite warning only. Detail is FAQ “How do I back up settings?” and [Configuration](configuration.md)
- **Focus-existing, user-facing**: FAQ “Launching a file that is already open” and `focusExisting` on [Slots](slots.md) (not in Help)
- **Wireless-numpad sleep (user-facing)**: Help is why to teach ON + the typing-window check. Detail is FAQ “Wireless numpad misses the first key”. Wake-from-sleep is not detectable from the hook. For the same unit, **prefer 2.4G (USB receiver) over Bluetooth** (a compromise; not as good as wired). Instant-fire suppression is not adopted. The FAQ model table is **measured units only**. Ratings are **○** / **△** / **×** (detail in notes). TK-029S is **△**. FAQ is press after Capturing ON (the throw-away-key procedure is withdrawn)

---

## 12. Privacy / security

### VirusTotal

The release zip is checked on VirusTotal before publication. Results are also posted on the GitHub Release.

| Version | Target | Detections | Notes |
|---|---|---|---|
| **1.0.0** | `DialKey-1.0.0-windows-x64.zip` SHA256 `d64b5fc53416105046d0186a1dc6de93381dd5315964b735c34f2ea2e2c9838d` | **1/67** | Bkav Pro only (`W32.Malware.B950DC92`). Major engines clean. [Report](https://www.virustotal.com/gui/file/d64b5fc53416105046d0186a1dc6de93381dd5315964b735c34f2ea2e2c9838d) |

### Privacy policy

Canonical text for README / the docs site.

- Keystroke contents are neither recorded, stored, nor transmitted
- The keyboard hook is active only while the window is shown
- DialKey itself does not open network connections (it may open a Help URL, or a user-registered http(s) slot, in the default browser)
- No telemetry or analytics
- All settings are stored locally
- Source is public and can be inspected by anyone
- Logs never contain keystroke contents. Besides launch failures, a **duration only** (no path) may be recorded when launch work is slow

---


## 13. Language packs

English is built in. Other languages can be added by placing `lang/<code>.toml` next to the exe (`ui.locale` in `settings.json`).
**Japanese (reference)** `ja.toml` and **Vietnamese (reference)** `vi.toml` are published as samples. They are not finished official translations. Missing keys fall back to English.

---

*v1.0 / app **1.0.1** — public specification. Normative text is §1–13. Author-only notes are not in this repository.*
