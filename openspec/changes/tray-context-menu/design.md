## Context

The app has an existing tray module (`src-tauri/src/tray.rs`) using Tauri v2's `TrayIconBuilder`. The current implementation:

- Left-click fires `TrayIconEvent::Click` without filtering by mouse button, toggling window visibility but also showing the context menu (Tauri default `show_menu_on_left_click(true)`)
- Right-click context menu has 3 items: disabled "Active Camera: None" label, "Open Panel", "Quit"
- Window close is already intercepted (`on_window_event` in `lib.rs`) to hide instead of quit

The user wants a clean separation: left-click = toggle, right-click = menu with Show/Hide, App Settings, Exit.

Additionally, 3 unresolved ambiguities from Section 9 (settings persistence) need fixing:

1. **TempDir leak** (`settings/commands.rs:255`): `Box::leak` keeps TempDir alive in tests — should return tuple like `store.rs` tests
2. **Debounce race window** (`settings/store.rs`): gap between `save()` and re-registering `notified()` — needs `AtomicBool` dirty flag
3. **enumerate_devices per control change** (`camera/commands.rs:81-86`): `set_camera_control` calls `enumerate_devices()` every time just for the camera name — frontend already has this info

## Goals / Non-Goals

**Goals:**

- Left-click on tray icon only toggles main window visibility (no menu)
- Right-click opens a 3-item context menu: Show/Hide, App Settings, Exit
- "App Settings" opens a separate native Tauri window
- All tray tests updated to match the new menu structure
- Fix TempDir leak, debounce race window, and enumerate_devices per control change (AMBIGUITIES.md)

**Non-Goals:**

- Building out the full app settings UI (placeholder page is sufficient)
- Frontend routing — the settings window loads its own entry point
- Floating widget (separate future feature)
- Camera label in the tray menu (explicitly removed per user decision)

## Decisions

### Decision 1: Disable menu on left-click via `show_menu_on_left_click(false)`

Tauri v2's `TrayIconBuilder` defaults to showing the menu on left-click. We disable this with `.show_menu_on_left_click(false)` and handle left-click explicitly in `on_tray_icon_event` by matching `MouseButton::Left` + `MouseButtonState::Up`.

**Alternative considered**: Removing the menu entirely and manually showing it on right-click. Rejected because Tauri's built-in menu-on-right-click is the native OS behaviour and works correctly by default.

### Decision 2: Separate Tauri window for App Settings

The App Settings page opens in a new `WebviewWindow` created via `WebviewWindowBuilder::new()` with label `"settings"`. This keeps the settings UI isolated from the main camera panel.

- If the settings window already exists, bring it to focus rather than creating a duplicate
- The window loads the same frontend bundle but at a different route/path (e.g., `index.html#settings` or a query param)
- Window size: smaller than main (e.g., 500x400), centred, resizable

**Alternative considered**: Opening settings as a view within the main window (client-side routing). Rejected because the user explicitly requested a separate native window.

### Decision 3: Frontend settings page via hash routing

The settings window loads `index.html#settings`. The React app checks `window.location.hash` on mount and renders either the main camera UI or the settings page. This avoids adding a full router dependency.

**Alternative considered**: A separate HTML entry point (`settings.html`). Rejected because Tauri's Vite setup uses a single entry point, and adding a second one adds build complexity for minimal benefit.

### Decision 4: Reuse existing `show_main_window` / `hide_main_window` helpers

The existing helpers in `tray.rs` are reused for the Show/Hide menu item. The toggle logic (check visibility, then show or hide) is the same as the left-click handler.

### Decision 5: Pass camera name from frontend to eliminate enumerate_devices per control change

Currently `set_camera_control` calls `enumerate_devices()` on every control change just to look up the camera name for persistence. The frontend already has the camera name (`ControlsPanel` receives `cameraName` prop). Add `camera_name: String` as a parameter to the Tauri command and `cameraName: string` to the frontend API function, eliminating the redundant backend call.

**Alternative considered**: Cache device names in the backend on first enumeration. Rejected because the frontend already has the information — just pass it through.

### Decision 6: Fix TempDir leak in settings/commands.rs tests

The `temp_store()` helper in `settings/commands.rs` uses `Box::leak(Box::new(dir))` to keep the TempDir alive. Fix by returning `(SettingsStore, TempDir)` tuple, matching the pattern already used in `settings/store.rs:98`. All test call sites update to `let (store, _dir) = temp_store()`.

### Decision 7: Close debounce race window with AtomicBool dirty flag

The debounce loop has a gap between `save()` completing and `notified().await` re-registering. Fix: add `AtomicBool` dirty flag to `SettingsStore`. Set it `true` in `set_control`/`remove_camera` before `notify_one()`. In the debounce loop, register the `notified()` future before checking the dirty flag, then save if dirty. This fully closes the race.

## Risks / Trade-offs

- **Window label collision**: If `WebviewWindowBuilder::new()` is called with `"settings"` when a settings window already exists, it will error. Mitigation: check `app.get_webview_window("settings")` first; if it exists, focus it instead.
- **Hash routing fragility**: Using `window.location.hash` is simple but doesn't support deep linking or back-button navigation. Acceptable for a settings page that's opened from a tray menu. Can be upgraded to a proper router later if needed.
- **Linux tray limitations**: `show_menu_on_left_click` is unsupported on Linux (Tauri docs note). On Linux, the menu may still show on left-click. Mitigation: none needed now — Windows-first, Linux behaviour is acceptable as-is.
- **Breaking IPC change**: Adding `cameraName` to `set_camera_control` changes the Tauri command signature. Both frontend and backend must be updated together. Low risk since this is an internal API.
