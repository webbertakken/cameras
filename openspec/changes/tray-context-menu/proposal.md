## Why

The system tray context menu doesn't match the intended UX. Left-click currently triggers both a window toggle and the context menu (Tauri's default). The context menu contains a stale "Active Camera" label and lacks Show/Hide and App Settings items. The user needs left-click to only toggle visibility and right-click to show a clean 3-item menu.

Additionally, 3 unresolved ambiguities from Section 9 (settings persistence) need fixing: TempDir leak in tests, debounce race window, and `enumerate_devices()` called per control change.

## What Changes

- Left-click on tray icon toggles window visibility only (no menu popup)
- Right-click context menu updated to 3 items: Show/Hide, App Settings, Exit
- Remove "Active Camera: None" label and separator from context menu
- "App Settings" opens a separate native Tauri window (not a route in the main window)
- Rename "Quit" to "Exit"
- Fix `Box::leak` TempDir in `settings/commands.rs` tests — return TempDir alongside store
- Fix debounce race window in `settings/store.rs` — add `AtomicBool` dirty flag, register `notified()` before checking flag
- Optimise `set_camera_control` — pass camera name from frontend instead of calling `enumerate_devices()` per change

## Capabilities

### New Capabilities

- `app-settings-window`: A separate native Tauri window for application-level settings, opened from the tray context menu

### Modified Capabilities

- `app-shell`: Tray context menu behaviour — updated menu items, left-click restricted to window toggle, right-click shows simplified menu
- `settings-persistence`: Fix TempDir leak in tests, debounce race window, and enumerate_devices per control change

## Impact

- **`src-tauri/src/tray.rs`**: Rewrite menu items, add `show_menu_on_left_click(false)`, filter `on_tray_icon_event` for `MouseButton::Left`, add handlers for new menu items
- **`src-tauri/src/tray.rs` tests**: Update all existing tests to match new menu structure
- **`src-tauri/tauri.conf.json`**: May need a second window definition for the settings window
- **Frontend**: A minimal settings page to render inside the new window (placeholder or initial implementation)
- **`src-tauri/src/settings/commands.rs`**: Fix `Box::leak` TempDir pattern in tests, remove `enumerate_devices()` call from `set_camera_control` (pass name from frontend)
- **`src-tauri/src/settings/store.rs`**: Add `AtomicBool` dirty flag to close debounce race window
- **`AMBIGUITIES.md`**: Mark resolved items as done
