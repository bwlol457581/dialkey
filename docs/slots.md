---
layout: default
title: Slots
permalink: /slots/
---

# Slots

Slots map a number (string id) to a program, script, or URL.

The **numpad itself is not registered** — any USB numpad works. A **wired**
pad is ready on the first key after idle; many wireless pads spend one or
more keys waking the radio. On a dual-mode pad, try **2.4G before
Bluetooth** (see the [FAQ](faq.md#wireless-numpad-misses-the-first-key)).
You only register
the tools you want to launch. Long UNC paths are fine to paste as-is
(see the [FAQ](faq.md#a-long-unc-or-folder-path-does-not-open)).

## How to register tools

1. Tray icon → **Settings** (first launch also opens Settings after copying
   `slots.example.json` → `slots.json` in the settings folder)
2. Open the **Slots** tab
3. Fill **Id** / **Name** / **Path** (and optional description, workdir).
   You can drop a file, folder, app, or `.url` onto **Path** or **Working
   directory**: Path gets the item, Working directory gets a folder (the item
   itself if it is a folder, otherwise its parent). `.lnk` is resolved to its
   target; if that fails (or a `.url` has no `http(s)` address), the field is
   left unchanged. Date tokens are not filled in for you.
4. Click **Update slot**, then **Save**

You can also edit `slots.json` in the `settings/` folder by hand, then tray →
**Reload**.

To switch books in daily use, press the mode button (**X1**) then `0`–`9`, or
use tray → **Mode**. With **Advanced** on, Settings → **General** → **Slots file**
pins which `slots*.json` is current. Leave it on **Sample (read-only)** to use
`slots.example.json` without writing it. Personal files (`slots.json`,
`slots.work.json`, …) are editable and saved atomically.

Personal paths stay in your chosen personal file only — that file is never
shipped in the release zip.

## slots.json

```json
{
  "schemaVersion": 1,
  "slots": [
    {
      "id": "1",
      "name": "Notepad",
      "description": "Text editor",
      "path": "notepad.exe"
    },
    {
      "id": "11",
      "name": "Cursor",
      "description": "Code editor",
      "path": "%LOCALAPPDATA%/Programs/cursor/Cursor.exe"
    },
    {
      "id": "31",
      "name": "Patrol Split",
      "description": "Split monthly patrol sheets",
      "path": "tools/split_patrol.bat"
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

### Fields

| Field | Required | Meaning |
|---|---|---|
| `id` | yes | Slot number as a **string**. Digits only. `"01"` ≠ `"1"` |
| `name` | yes | Display name (Search) |
| `path` | yes | Executable / script / document / folder path, `http(s)` URL, or `chain:11,20` (advanced). Documents (e.g. `.xlsx`) open via file association. Relative paths resolve against the DialKey exe folder; use `%USERPROFILE%\Desktop\file.xlsx` for the Windows Desktop. May include [date tokens](#date-tokens) such as `{yyyyMM}` |
| `description` | no | Extra search text |
| `workdir` | no | Working directory (ignored for URL / `chain:` slots). Empty = parent folder of Path. May include [date tokens](#date-tokens) |
| `openWorkdir` | no | When `true`, also open the working directory (or Path’s parent) in Explorer when launching. Ignored for URL / `chain:` slots. Default `false` |
| `focusExisting` | no | When `false`, skip “already open → focus” and always launch normally for that slot. Omitted / `true` keeps best-effort focus. Ignored for URL / `chain:` (never focused). Editable in Settings → Slots |

Integer `"id": 11` in the file is accepted and stored as `"11"`. Prefer strings
so leading zeros survive (`"01"`).

## How to dial

| Action | Result |
|---|---|
| Trigger → digit with `instant: true` | Launch that slot immediately |
| Trigger → `+` → digits → Enter | Launch the matching multi-digit slot |
| Trigger → `+` → digits → `.` (or your Open-workdir key) | Open the working folder **only** (parent of Path, or `workdir` if set). Does not launch Path. Works for instant digits too if you use `+` first |
| Trigger → Tab (or tray → Search) | Search by **id** / name / path / description; select a row and Enter (or click) to launch |

Max **10 digits** in multi-digit mode; further digits are ignored.

## Path resolution

For non-URL / non-chain paths:

1. Expand **date tokens** (`{yyyy}`, `{yyyyMM}`, … — see below)
2. Expand environment variables (`%USERPROFILE%`, `%APPDATA%`, …)
3. Absolute paths are used as-is
4. Relative paths resolve against the **folder that contains `dialkey.exe`**, not
   the process current directory

This keeps USB / portable layouts working.

### Date tokens

Brace-wrapped tokens in `path` and `workdir` expand to **today’s local date/time**
at launch (one fixed instant per launch). Bare text like `yyyy` is **not**
replaced (avoids mangling folder names such as `D:\yyyy_backup\`).

| Token | Meaning | Example (2026-08-10 14:05) |
|---|---|---|
| `{yyyy}` | 4-digit year | `2026` |
| `{yy}` | 2-digit year | `26` |
| `{MM}` | month | `08` |
| `{dd}` | day | `10` |
| `{HH}` | hour (24h) | `14` |
| `{min}` | **minute** | `05` |
| `{yyyyMM}` | year+month | `202608` |
| `{yyyyMMdd}` | year+month+day | `20260810` |

Unknown `{…}` groups are left unchanged. URLs and `chain:` paths are not expanded.

Example:

```text
D:\work\{yyyy}\{yyyyMM}\report_{yyyyMM}.xlsx
→ D:\work\2026\202608\report_202608.xlsx
```

If the expanded path does not exist yet (e.g. early in a new month), DialKey opens
the **deepest existing ancestor folder**. It never creates missing folders
(mistyped tokens must not litter the disk). Walking back is not treated as an
error.

Minutes use `{min}`, not `{mm}` — see [FAQ](faq.md).

### URL slots

If `path` starts with `http://` or `https://`:

- Opens in the default browser
- No path resolution, `workdir`, or `openWorkdir`
- Not warned as “path missing”

Bare hosts (`www.example.com`) and other schemes (`ftp://`) are **not** treated as
URLs.

### Chain slots (advanced)

If `path` starts with `chain:`:

```text
chain:11,20,31
```

- Launches the listed slot ids in order (one level only — nested `chain:` targets are skipped)
- **Open folder** opens each member’s working folder in that same order (URL / nested `chain:` / missing ids skipped). The chain row’s own `workdir` is not used
- Continues if a target fails or is missing
- Launch does not use the chain row’s `workdir` or `openWorkdir`
- Cycles are rejected when you Update / Save; missing targets show a warning

`chain:` is an advanced special command — most users never need it. Prefer a short
`description` that explains what the bundle launches.

Need fixed command-line arguments? Put them in a `.bat` / `.ps1` and register that
script as the slot `path`. DialKey does not carry an `args` field.

## Launch rules

| Path kind | How it starts |
|---|---|
| `.exe` | Direct |
| `.bat` / `.cmd` | `cmd /c "…"` |
| `.ps1` | `powershell -ExecutionPolicy Bypass -File "…"` |
| `.ahk` | File association |
| folder / document | Shell association |
| `http(s)://` | Default browser |
| `chain:…` | Launch each referenced slot (one level). Open folder walks member folders |

Default working directory (non-URL / non-chain): the folder containing the
script/exe, unless `workdir` is set. Check **Also open working directory** in
Settings (or set `"openWorkdir": true`) to open that folder in Explorer at the
same time.

## Naming tips

- Put a few everyday tools on **instant** digits (shortest path)
- Group by leading digit (10s = editors, 20s = macros, …)
- Use mnemonics when they stick (`15` ≈ strawberry / イチゴ, etc.)

Search lists use **natural sort** (`2` before `10`).

## Example file

Shipped as `slots.example.json`:

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
      "description": "Open a folder — samples/ next to the exe (shell association)",
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

The release zip includes `samples/hello.bat` next to `dialkey.exe`.
Slot `2` can later point at the GitHub Pages docs (`settings.json` → `docsUrl`).
Slot `5` shows the advanced `chain:` command (`+` → `5` → Enter launches 1 and 3).
Slot `6` shows date tokens (`{yyyy}` / `{yyyyMM}`); if the path is missing, DialKey opens the deepest existing ancestor and never creates folders.
