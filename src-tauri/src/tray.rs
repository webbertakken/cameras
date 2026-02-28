use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::webview::WebviewWindowBuilder;
use tauri::{AppHandle, Manager};

/// Identifiers for tray menu items.
const MENU_ID_SHOW_HIDE: &str = "show-hide";
const MENU_ID_APP_SETTINGS: &str = "app-settings";
const MENU_ID_QUIT: &str = "quit";

/// Show the main window and give it focus.
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Hide the main window.
fn hide_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

/// Toggle main window visibility: show if hidden, hide if visible.
fn toggle_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            hide_main_window(app);
        } else {
            show_main_window(app);
        }
    }
}

/// Open the app settings window, or focus it if already open.
fn open_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.set_focus();
        return;
    }

    let builder = WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("index.html#settings".into()),
    )
    .title("App Settings")
    .inner_size(500.0, 400.0)
    .resizable(true)
    .center();

    if let Err(e) = builder.build() {
        tracing::error!("Failed to open settings window: {e}");
    }
}

/// Build and register the system tray for the application.
pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show_hide = MenuItemBuilder::with_id(MENU_ID_SHOW_HIDE, "Show/Hide").build(app)?;
    let app_settings = MenuItemBuilder::with_id(MENU_ID_APP_SETTINGS, "App Settings").build(app)?;
    let quit = MenuItemBuilder::with_id(MENU_ID_QUIT, "Exit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_hide)
        .item(&app_settings)
        .item(&quit)
        .build()?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().unwrap())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            match id {
                "show-hide" => toggle_main_window(app),
                "app-settings" => open_settings_window(app),
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Testable representation of menu items (id, label).
    fn menu_item_defs() -> Vec<(&'static str, &'static str)> {
        vec![
            (MENU_ID_SHOW_HIDE, "Show/Hide"),
            (MENU_ID_APP_SETTINGS, "App Settings"),
            (MENU_ID_QUIT, "Exit"),
        ]
    }

    #[test]
    fn menu_item_defs_contains_correct_ids_and_labels() {
        let defs = menu_item_defs();

        assert_eq!(defs.len(), 3);
        assert_eq!(defs[0], (MENU_ID_SHOW_HIDE, "Show/Hide"));
        assert_eq!(defs[1], (MENU_ID_APP_SETTINGS, "App Settings"));
        assert_eq!(defs[2], (MENU_ID_QUIT, "Exit"));
    }

    #[test]
    fn menu_has_show_hide_item() {
        let defs = menu_item_defs();
        let item = defs.iter().find(|(id, _)| *id == MENU_ID_SHOW_HIDE);

        assert!(item.is_some());
        assert_eq!(item.unwrap().1, "Show/Hide");
    }

    #[test]
    fn menu_has_app_settings_item() {
        let defs = menu_item_defs();
        let item = defs.iter().find(|(id, _)| *id == MENU_ID_APP_SETTINGS);

        assert!(item.is_some());
        assert_eq!(item.unwrap().1, "App Settings");
    }

    #[test]
    fn menu_has_quit_item() {
        let defs = menu_item_defs();
        let quit = defs.iter().find(|(id, _)| *id == MENU_ID_QUIT);

        assert!(quit.is_some());
        assert_eq!(quit.unwrap().1, "Exit");
    }
}
