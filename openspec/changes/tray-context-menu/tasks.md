## 1. Update tray menu structure

- [ ] 1.1 Remove `MENU_ID_CAMERA_LABEL`, `DEFAULT_CAMERA_LABEL` constants and the disabled camera label menu item
- [ ] 1.2 Remove the separator (`PredefinedMenuItem::separator`)
- [ ] 1.3 Add `MENU_ID_SHOW_HIDE` constant and "Show/Hide" menu item
- [ ] 1.4 Add `MENU_ID_APP_SETTINGS` constant and "App Settings" menu item
- [ ] 1.5 Rename `MENU_ID_QUIT` label from "Quit" to "Exit"
- [ ] 1.6 Update `MenuBuilder` to use the 3 new items: Show/Hide, App Settings, Exit

## 2. Fix left-click behaviour

- [ ] 2.1 Add `.show_menu_on_left_click(false)` to `TrayIconBuilder` chain
- [ ] 2.2 Update `on_tray_icon_event` to match only `MouseButton::Left` + `MouseButtonState::Up` (instead of catching all `Click` events)

## 3. Update menu event handlers

- [ ] 3.1 Add `show-hide` handler to `on_menu_event` that toggles main window visibility
- [ ] 3.2 Add `app-settings` handler to `on_menu_event` that opens the settings window
- [ ] 3.3 Update `quit` handler to use new `MENU_ID_QUIT` id (or keep as-is if id unchanged)

## 4. App settings window

- [ ] 4.1 Create `open_settings_window` helper in `tray.rs` — uses `WebviewWindowBuilder::new()` with label `"settings"`, loads `index.html#settings`, 500x400, centred
- [ ] 4.2 Handle existing window case — if `app.get_webview_window("settings")` returns `Some`, focus it instead of creating a new one
- [ ] 4.3 Settings window close should destroy the window (default Tauri behaviour, no `prevent_close` override needed)

## 5. Frontend settings page

- [ ] 5.1 Add hash-based routing in `App.tsx` or `main.tsx` — check `window.location.hash` and render settings page when `#settings`
- [ ] 5.2 Create a minimal `SettingsPage` component (placeholder with "App Settings" heading, wrapped in `ThemeProvider`)

## 6. Update tests

- [ ] 6.1 Update `menu_item_defs()` test helper to reflect new 3-item menu (Show/Hide, App Settings, Exit)
- [ ] 6.2 Update `menu_item_defs_contains_correct_ids_and_labels` test
- [ ] 6.3 Replace `menu_has_camera_label_item` test with `menu_has_show_hide_item` test
- [ ] 6.4 Replace `menu_has_open_panel_item` test with `menu_has_app_settings_item` test
- [ ] 6.5 Update `menu_has_quit_item` test to check for "Exit" label
- [ ] 6.6 Remove `default_camera_label_shows_none` and `camera_label_format_with_name` tests
- [ ] 6.7 Add frontend test for hash routing (renders `SettingsPage` when hash is `#settings`, renders main app otherwise)

## 7. Fix ambiguity: pass camera name from frontend (AMBIGUITIES.md #3/#6)

- [ ] 7.1 Add `camera_name: String` parameter to `set_camera_control` Tauri command in `camera/commands.rs`
- [ ] 7.2 Remove `enumerate_devices()` call for camera name lookup in `set_camera_control`
- [ ] 7.3 Update frontend `setCameraControl` API function to accept and pass `cameraName`
- [ ] 7.4 Update `ControlsPanel` to pass `cameraName` when calling `setCameraControl`
- [ ] 7.5 Update all frontend tests that mock/call `setCameraControl` to include `cameraName`

## 8. Fix ambiguity: TempDir leak in tests (AMBIGUITIES.md #7)

- [ ] 8.1 Change `temp_store()` in `settings/commands.rs` to return `(SettingsStore, TempDir)` instead of using `Box::leak`
- [ ] 8.2 Update all test call sites to `let (store, _dir) = temp_store()`

## 9. Fix ambiguity: debounce race window (AMBIGUITIES.md #5)

- [ ] 9.1 Add `AtomicBool` dirty flag to `SettingsStore` struct
- [ ] 9.2 Set dirty flag `true` in `set_control` and `remove_camera` before `notify_one()`
- [ ] 9.3 Update `start_debounce_task` to register `notified()` future before checking dirty flag, save if dirty, then loop
- [ ] 9.4 Add test verifying that a change during the save-to-notify gap is not lost

## 10. Clean up AMBIGUITIES.md

- [ ] 10.1 Mark resolved ambiguities as done or remove the file if all are resolved
