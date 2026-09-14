use super::keys;

/// Embedded English defaults — source locale. No `en.toml` on disk.
pub fn english_defaults() -> &'static [(&'static str, &'static str)] {
    &[
        (keys::TRAY_MENU_RUN, "Run DialKey"),
        (keys::TRAY_MENU_SEARCH, "Search"),
        (keys::TRAY_MENU_SETTINGS, "Settings"),
        (keys::TRAY_MENU_RELOAD, "Reload"),
        (keys::TRAY_MENU_HELP, "Help"),
        (keys::TRAY_MENU_QUIT, "Quit"),
        (keys::TRAY_MENU_MODE, "Mode"),
        (keys::TRAY_MENU_KEYS, "Keys"),
        (keys::TRAY_TOOLTIP, "DialKey v{version}"),
        (keys::SETTINGS_TAB_SLOTS, "Slots"),
        (keys::SETTINGS_TAB_KEYS, "Keys"),
        (keys::SETTINGS_TAB_TRIGGERS, "Triggers"),
        (keys::SETTINGS_TAB_SEARCH, "Search"),
        (keys::SETTINGS_TAB_MODE, "Mode"),
        (keys::SETTINGS_TAB_GENERAL, "General"),
        (keys::SETTINGS_ADVANCED, "Advanced"),
        (keys::SETTINGS_ADD, "Add"),
        (keys::SETTINGS_DELETE, "Delete"),
        (keys::SETTINGS_ID, "ID"),
        (keys::SETTINGS_NAME, "Name"),
        (keys::SETTINGS_PATH, "Path"),
        (keys::SETTINGS_WORKING, "Working directory"),
        (keys::SETTINGS_DESCRIPTION, "Description"),
        (keys::SETTINGS_UPDATE_SLOT, "Update slot"),
        (keys::SETTINGS_SWAP_IDS, "Swap IDs"),
        (
            keys::SETTINGS_OPEN_WORKDIR,
            "Also open working directory when launching",
        ),
        (
            keys::SETTINGS_FOCUS_EXISTING,
            "Focus existing window if already open",
        ),
        (keys::SETTINGS_CUE_NAME, "display name"),
        (
            keys::SETTINGS_CUE_PATH,
            "exe, script, URL, chain:11,20, or file — date tokens {yyyyMM}; e.g. %USERPROFILE%\\Desktop\\a.xlsx",
        ),
        (
            keys::SETTINGS_CUE_WORKDIR,
            "optional; empty = parent folder of Path",
        ),
        (
            keys::SETTINGS_WARN_LOOKALIKE,
            "Warning: look-alike slot IDs are registered:\n{pairs}They are different slots. This is allowed.",
        ),
        (keys::SETTINGS_WARN_LOOKALIKE_PAIR, "  \"{a}\" and \"{b}\"\n"),
        (
            keys::SETTINGS_WARN_CHAIN_MISSING,
            "Warning: chain targets not found:\n{items}",
        ),
        (
            keys::SETTINGS_WARN_CHAIN_MISSING_ITEM,
            "  slot {slot}: missing {ids}\n",
        ),
        (
            keys::SETTINGS_CHAIN_INVALID,
            "Invalid chain on slot {slot}:\n{message}",
        ),
        (
            keys::SETTINGS_CHAIN_CYCLE,
            "Cannot save: chain cycle detected:\n{path}",
        ),
        (
            keys::SETTINGS_SWAP_NEED_OTHER,
            "To swap, select a slot in the list, type the other slot's ID in the ID field, then click Swap IDs.",
        ),
        (
            keys::SETTINGS_SWAP_TARGET_MISSING,
            "No slot with ID {id} to swap with.",
        ),
        (
            keys::SETTINGS_SWAP_SELECT_FIRST,
            "Select a slot in the list first, then Swap IDs.",
        ),
        (
            keys::SETTINGS_GROUP_ACTION_KEYS,
            "Action keys (press Capture, then a key)",
        ),
        (keys::SETTINGS_START, "Multi-trigger"),
        (keys::SETTINGS_START_LABEL, "Display"),
        (keys::SETTINGS_CONFIRM, "Confirm"),
        (keys::SETTINGS_CANCEL, "Cancel"),
        (keys::SETTINGS_SEARCH, "Search"),
        (
            keys::SETTINGS_OPEN_WORKDIR_KEY,
            "Open folder",
        ),
        (keys::SETTINGS_DIGIT_BACK, "Digit back"),
        (keys::SETTINGS_SETTINGS_KEY, "Settings"),
        (keys::SETTINGS_WINDOW_BG, "Window background"),
        (keys::SETTINGS_WINDOW_FG, "Text"),
        (
            keys::SETTINGS_WAKE_KEY,
            "ON key (wireless pad)",
        ),
        (keys::SETTINGS_KEY_SET, "Key set"),
        (keys::SETTINGS_KEY_CHORD, "X1 letter"),
        (keys::SETTINGS_GROUP_MODE_SWITCH, "Switch triggers"),
        (keys::SETTINGS_WAKE_VK, "ON"),
        (keys::KEY_SET_NUMPAD, "Numpad"),
        (keys::KEY_SET_KEYBOARD, "Keyboard"),
        (keys::SETTINGS_CAPTURE, "Capture"),
        (keys::SETTINGS_GROUP_LEGEND, "Displayed options"),
        (keys::SETTINGS_LEGEND_IDLE, "Idle"),
        (keys::SETTINGS_LEGEND_MULTI, "Narrow down"),
        (keys::SETTINGS_LEGEND_UP, "Up"),
        (keys::SETTINGS_LEGEND_DOWN, "Down"),
        (keys::SETTINGS_GROUP_TRIGGERS, "Launch triggers"),
        (keys::SETTINGS_MOUSE_BUTTON, "Mouse button"),
        (keys::SETTINGS_SUPPRESS, "Suppress button from other apps"),
        (keys::SETTINGS_HOTKEY, "Hotkey"),
        (
            keys::SETTINGS_HOTKEY_HINT,
            "(e.g. F9 or Ctrl+Alt+D; empty = off)",
        ),
        (keys::SETTINGS_RESET_DEFAULTS, "Reset this key set to defaults"),
        (keys::SETTINGS_RESET_TRIGGERS, "Reset triggers to defaults"),
        (
            keys::SETTINGS_CAPTURING_KEY,
            "Capturing key…\nPress a key. Cancel on that row aborts.\nDigits 0–9 are reserved.",
        ),
        (
            keys::SETTINGS_CAPTURING_MOUSE,
            "Capturing mouse…\nPress X1, X2, or middle.\nLeft/right are ignored.",
        ),
        (keys::SETTINGS_CAPTURE_FAILED, "Capture failed — see log."),
        (
            keys::SETTINGS_INVALID_KEY,
            "Invalid key — try another.\nCancel on that row aborts.",
        ),
        (
            keys::SETTINGS_RESET_STATUS,
            "Key set reset to defaults (not saved yet).",
        ),
        (
            keys::SETTINGS_RESET_TRIGGERS_STATUS,
            "Triggers reset to defaults (not saved yet).",
        ),
        (keys::SETTINGS_INSTANT_FIRE, "Instant fire"),
        (keys::SETTINGS_GROUP_SEARCH, "Search"),
        (keys::SETTINGS_MATCH_MODE, "Match mode"),
        (keys::SETTINGS_CASE_SENSITIVE, "Case sensitive"),
        (keys::SETTINGS_GROUP_GENERAL, "General"),
        (keys::SETTINGS_TIMEOUT, "Timeout (sec)"),
        (keys::SETTINGS_MAX_DIGITS, "Max digits"),
        (
            keys::SETTINGS_FEEDBACK_MS,
            "Launch feedback (ms, 0 = off)",
        ),
        (keys::SETTINGS_LOG_LEVEL, "Log level"),
        (
            keys::SETTINGS_AUTOSTART,
            "Start DialKey when I sign in to Windows",
        ),
        (keys::SETTINGS_UI_LANGUAGE, "UI language"),
        (keys::SETTINGS_LANG_SYSTEM, "System default"),
        (keys::SETTINGS_LANG_ENGLISH, "English"),
        (keys::SETTINGS_SLOTS_FILE, "Slots file"),
        (
            keys::SETTINGS_SLOTS_FILE_EXAMPLE,
            "Sample (read-only)",
        ),
        (
            keys::SETTINGS_SLOTS_READONLY,
            "Sample / read-only slots file — choose a personal file (e.g. slots.json) to edit and save.",
        ),
        (keys::SETTINGS_OPEN_CONFIG_FOLDER, "Open folder"),
        (keys::SETTINGS_BACKUP_CONFIG, "Backup"),
        (keys::SETTINGS_BACKUP_FOLDER, "Backup folder"),
        (keys::SETTINGS_BACKUP_BROWSE, "Browse"),
        (
            keys::SETTINGS_BACKUP_OK,
            "Copied JSON to:\n{path}",
        ),
        (
            keys::SETTINGS_BACKUP_FAIL,
            "Backup failed:\n{error}",
        ),
        (
            keys::SETTINGS_BACKUP_INTO_APP,
            "Choose a folder outside DialKey (not the app or settings folder).",
        ),
        (
            keys::SETTINGS_BACKUP_NO_FOLDER,
            "Could not find the Desktop folder.",
        ),
        (keys::SETTINGS_RESTORE_CONFIG, "Load"),
        (keys::SETTINGS_RESTORE_TITLE, "Restore"),
        (keys::SETTINGS_RESTORE_PICK, "Restore from"),
        (
            keys::SETTINGS_RESTORE_ALL_BOOKS,
            "All slots files (slots*.json)",
        ),
        (
            keys::SETTINGS_RESTORE_CURRENT_BOOK,
            "Current slots file only",
        ),
        (
            keys::SETTINGS_RESTORE_INCLUDE_SETTINGS,
            "Also restore settings.json",
        ),
        (
            keys::SETTINGS_RESTORE_HINT,
            "This overwrites files in the settings folder.",
        ),
        (
            keys::SETTINGS_RESTORE_OK,
            "Restored:\n{files}",
        ),
        (
            keys::SETTINGS_RESTORE_FAIL,
            "Restore failed:\n{error}",
        ),
        (
            keys::SETTINGS_RESTORE_NOTHING,
            "No matching JSON in that folder.",
        ),
        (
            keys::SETTINGS_RESTORE_READONLY,
            "The current slots file is the sample (read-only). Choose a personal file first.",
        ),
        (
            keys::SETTINGS_RESTORE_MISSING,
            "That backup folder does not contain the current slots file.",
        ),
        (
            keys::SETTINGS_RESTORE_FROM_LIVE,
            "Choose a backup stamp folder, not the live settings folder.",
        ),
        (keys::SETTINGS_GROUP_MODE, "Mode"),
        (keys::SETTINGS_MODE_MOUSE, "Mode-switch mouse button"),
        (
            keys::SETTINGS_MODE_SUPPRESS,
            "Suppress button from other apps",
        ),
        (keys::SETTINGS_MODE_TIMEOUT, "Chord timeout (ms)"),
        (keys::SETTINGS_MODE_CHORDS, "code / slots file / display name"),
        (keys::SETTINGS_MODE_CHORD_CODE, "Code"),
        (keys::SETTINGS_MODE_CHORD_FILE, "Slots file"),
        (keys::SETTINGS_MODE_CURRENT, "Current book"),
        (
            keys::SETTINGS_SLOTS_CURRENT,
            "Editing {book}",
        ),
        (
            keys::SETTINGS_MODE_SWITCH_DIRTY,
            "Slots have unsaved changes.\nSave them before switching books?",
        ),
        (keys::SETTINGS_MODE_SET_OVERRIDE, "Assign"),
        (keys::SETTINGS_MODE_CLEAR_OVERRIDE, "Unassign"),
        (
            keys::SETTINGS_MODE_HINT,
            "Mode button, then 0–9 switches books; a letter switches the key set. Double-click a row or use Current book to switch here. Assign / Unassign pins a file to a code (does not switch).",
        ),
        (
            keys::SETTINGS_MODE_SET_NEED_BOTH,
            "Select a code row and a slots file, then Assign.",
        ),
        (keys::SETTINGS_MODE_SET_OK, "Pinned {code} → {file}."),
        (
            keys::SETTINGS_MODE_SET_SWAPPED,
            "Swapped {code} ↔ {other} ({file}).",
        ),
        (
            keys::SETTINGS_MODE_CLEAR_OK,
            "Cleared {code} (auto-assign).",
        ),
        (
            keys::SETTINGS_WINDOW_TITLE,
            "DialKey — Settings  v{version}",
        ),
        (keys::SETTINGS_SAVE, "Save"),
        (keys::SETTINGS_APPLY, "Apply"),
        (keys::SETTINGS_BTN_CANCEL, "Cancel"),
        (keys::SETTINGS_DIALOG_TITLE, "DialKey"),
        (
            keys::SETTINGS_SLOT_ID_DIGITS,
            "Slot ID must be digits only (e.g. 1, 01, 11).",
        ),
        (keys::SETTINGS_NAME_PATH_REQUIRED, "Name and Path are required."),
        (
            keys::SETTINGS_PATH_MISSING,
            "Warning: path does not exist:\n{path}\n\nThe slot will still be saved.",
        ),
        (keys::SETTINGS_TIMEOUT_NUMBER, "Timeout must be a number."),
        (
            keys::SETTINGS_TIMEOUT_MIN,
            "Timeout must be at least 1 second.",
        ),
        (
            keys::SETTINGS_MAX_DIGITS_NUMBER,
            "Max digits must be a number.",
        ),
        (
            keys::SETTINGS_MAX_DIGITS_RANGE,
            "Max digits must be between 1 and 32.",
        ),
        (
            keys::SETTINGS_FEEDBACK_MS_NUMBER,
            "Launch feedback must be a number (milliseconds; 0 disables).",
        ),
        (
            keys::SETTINGS_KEY_RESERVED_DIGIT,
            "Key \"{role}\" cannot be a digit (0–9). Digits are reserved for dialing.",
        ),
        (
            keys::SETTINGS_KEY_DUPLICATE,
            "Keys \"{role_a}\" and \"{role_b}\" both use {name}. They must be unique.",
        ),
        (
            keys::SETTINGS_KEY_DUPLICATE_CHORD,
            "X1 letters for \"{set_a}\" and \"{set_b}\" are both {letter}. They must be unique.",
        ),
        (
            keys::SETTINGS_KEY_DISPLAY_DUPLICATE,
            "Display labels for \"{role_a}\" and \"{role_b}\" are both {label}. They must be unique.",
        ),
        (
            keys::SETTINGS_KEY_UNKNOWN,
            "Key \"{role}\" has an unknown binding: {name}",
        ),
        (
            keys::SETTINGS_UNWRITABLE,
            "settings.json could not be read at startup.\nDialKey will not overwrite it. Fix or rename the file, then Reload.",
        ),
        (
            keys::SETTINGS_SLOTS_UNWRITABLE,
            "The slots file could not be read at startup.\nDialKey will not overwrite it. Fix or rename the file, then Reload.",
        ),
        (
            keys::SETTINGS_SAVE_SETTINGS_FAILED,
            "Failed to save settings.json:\n{error}",
        ),
        (
            keys::SETTINGS_SAVE_SLOTS_FAILED,
            "Failed to save slots file:\n{error}",
        ),
        (keys::SETTINGS_SAVED, "Settings saved."),
        (keys::SEARCH_WINDOW_TITLE, "DialKey — Search  v{version}"),
        (keys::SEARCH_HINT_OPEN_WORKDIR, "{key}: open folder"),
        (
            keys::APP_TYPING_HINT_IDLE,
            "DialKey — instant | + multi-digit | Tab search | -",
        ),
        (
            keys::APP_TYPING_HINT_MULTI_EMPTY,
            "DialKey — multi-digit: type number, Enter to launch",
        ),
        (keys::APP_TYPING_HINT_MULTI_BUFFER, "+{buffer}"),
        (keys::APP_TYPING_FEEDBACK, "Launching {id} — {name}"),
        (keys::APP_TYPING_FEEDBACK_FAILED, "Launch failed {id} — {name}"),
        (keys::APP_TYPING_FEEDBACK_OPEN, "Opening {id} — {name}"),
        (
            keys::APP_TYPING_FEEDBACK_OPEN_FAILED,
            "Open folder failed {id} — {name}",
        ),
        (keys::APP_TYPING_LEGEND_SEARCH, "Search"),
        (keys::APP_TYPING_LEGEND_MULTI, "Trigger"),
        (keys::APP_TYPING_LEGEND_ESC, "Cancel"),
        (keys::APP_TYPING_LEGEND_OPEN_WORKDIR, "Open"),
        (keys::APP_TYPING_LEGEND_DIGIT_BACK, "Back"),
        (keys::APP_TYPING_LEGEND_SETTINGS, "Settings"),
        (
            keys::APP_TYPING_NO_INSTANT,
            "No instant-fire slots configured.",
        ),
        (keys::TRAY_NOTIFY_TITLE, "DialKey"),
        (
            keys::TRAY_NOTIFY_STARTED,
            "v{version} — running in the notification area.",
        ),
        (
            keys::TRAY_NOTIFY_OPEN_TYPING_FAILED,
            "Failed to open typing window.",
        ),
        (keys::TRAY_NOTIFY_RELOADED, "Configuration reloaded."),
        (keys::TRAY_NOTIFY_MODE_SWITCHED, "Mode: {file}"),
        (keys::TRAY_NOTIFY_KEY_SET, "Keys: {name}"),
        (keys::TRAY_NOTIFY_RELOAD_FAILED, "Reload failed — see log."),
        (
            keys::TRAY_NOTIFY_OPEN_SEARCH_FAILED,
            "Failed to open Search.",
        ),
        (
            keys::TRAY_NOTIFY_OPEN_SETTINGS_FAILED,
            "Failed to open settings.",
        ),
        (
            keys::TRAY_NOTIFY_APPLY_RELOAD_FAILED,
            "Saved, but reload failed — see log.",
        ),
        (
            keys::TRAY_NOTIFY_LAUNCH_FAILED,
            "Launch failed — see log.",
        ),
        (keys::HELP_DIALOG_TITLE, "DialKey"),
        (
            keys::HELP_ABOUT_NO_DOCS,
            "DialKey v{version}\n\nA software macropad.\n\n1. Open with the mouse forward button (X2).\n2. An instant digit launches at once. For other numbers press + (the first line keeps +: then +1, +10), type the digits, then Enter. After that, `.` opens the working folder only (does not launch); Backspace deletes the last digit (or returns to the first screen).\n3. Press Tab or left-click the tray icon for Search. Cancel closes. The Settings key (/ by default) opens Settings.\n\nSome wireless pads sleep after idle. In Settings → Keys you can remember the pad’s ON key; DialKey then ignores it while the typing window is open, so waking the pad does not cancel or launch. Watch the window to confirm keys registered.\n\nRegister tools in Settings → Slots. Switch books with the mode button (X1) then a digit 0–9, or tray → Mode.\n\nCaution: Settings → General → Load copies into the settings folder and overwrites those files. DialKey does not keep using the backup.\n\nTip: Path may be chain:11,20 to launch several slots at once.\n\nFull guide: documentation site (docsUrl is not set).",
        ),
        (
            keys::HELP_ABOUT_WITH_DOCS,
            "DialKey v{version}\n\nA software macropad.\n\n1. Open with the mouse forward button (X2).\n2. An instant digit launches at once. For other numbers press + (the first line keeps +: then +1, +10), type the digits, then Enter. After that, `.` opens the working folder only (does not launch); Backspace deletes the last digit (or returns to the first screen).\n3. Press Tab or left-click the tray icon for Search. Cancel closes. The Settings key (/ by default) opens Settings.\n\nSome wireless pads sleep after idle. In Settings → Keys you can remember the pad’s ON key; DialKey then ignores it while the typing window is open, so waking the pad does not cancel or launch. Watch the window to confirm keys registered.\n\nRegister tools in Settings → Slots. Switch books with the mode button (X1) then a digit 0–9, or tray → Mode.\n\nCaution: Settings → General → Load copies into the settings folder and overwrites those files. DialKey does not keep using the backup.\n\nTip: Path may be chain:11,20 to launch several slots at once.\n\nFull guide (install, slots, FAQ):\n{url}\n\nOpen the docs site now?",
        ),
        (keys::ROLE_START, "multi-trigger"),
        (keys::ROLE_CONFIRM, "confirm"),
        (keys::ROLE_CANCEL, "cancel"),
        (keys::ROLE_SEARCH, "search"),
        (keys::ROLE_OPEN_WORKDIR, "open folder"),
        (keys::ROLE_DIGIT_BACK, "digit back"),
        (keys::ROLE_SETTINGS, "settings"),
        (keys::ROLE_WAKE, "ON"),
    ]
}

pub fn lookup_english(key: &str) -> Option<&'static str> {
    english_defaults()
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
}
