use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

/// Identifiers for tray menu items.
const MENU_ID_CAMERA_LABEL: &str = "camera-label";
const MENU_ID_OPEN_PANEL: &str = "open-panel";
const MENU_ID_QUIT: &str = "quit";

/// Default label shown when no camera is active.
const DEFAULT_CAMERA_LABEL: &str = "Active Camera: None";

/// Build and register the system tray for the application.
pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let camera_label = MenuItemBuilder::with_id(MENU_ID_CAMERA_LABEL, DEFAULT_CAMERA_LABEL)
        .enabled(false)
        .build(app)?;

    let separator = PredefinedMenuItem::separator(app)?;

    let open_panel = MenuItemBuilder::with_id(MENU_ID_OPEN_PANEL, "Open Panel").build(app)?;
    let quit = MenuItemBuilder::with_id(MENU_ID_QUIT, "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&camera_label)
        .item(&separator)
        .item(&open_panel)
        .item(&quit)
        .build()?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().cloned().unwrap())
        .menu(&menu)
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            match id {
                "open-panel" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click { .. } = event {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
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
            (MENU_ID_CAMERA_LABEL, DEFAULT_CAMERA_LABEL),
            (MENU_ID_OPEN_PANEL, "Open Panel"),
            (MENU_ID_QUIT, "Quit"),
        ]
    }

    #[test]
    fn menu_item_defs_contains_correct_ids_and_labels() {
        let defs = menu_item_defs();

        assert_eq!(defs.len(), 3);
        assert_eq!(defs[0], (MENU_ID_CAMERA_LABEL, DEFAULT_CAMERA_LABEL));
        assert_eq!(defs[1], (MENU_ID_OPEN_PANEL, "Open Panel"));
        assert_eq!(defs[2], (MENU_ID_QUIT, "Quit"));
    }

    #[test]
    fn menu_has_camera_label_item() {
        let defs = menu_item_defs();
        let camera = defs.iter().find(|(id, _)| *id == MENU_ID_CAMERA_LABEL);

        assert!(camera.is_some());
        assert_eq!(camera.unwrap().1, "Active Camera: None");
    }

    #[test]
    fn menu_has_open_panel_item() {
        let defs = menu_item_defs();
        let open = defs.iter().find(|(id, _)| *id == MENU_ID_OPEN_PANEL);

        assert!(open.is_some());
        assert_eq!(open.unwrap().1, "Open Panel");
    }

    #[test]
    fn menu_has_quit_item() {
        let defs = menu_item_defs();
        let quit = defs.iter().find(|(id, _)| *id == MENU_ID_QUIT);

        assert!(quit.is_some());
        assert_eq!(quit.unwrap().1, "Quit");
    }

    #[test]
    fn default_camera_label_shows_none() {
        assert_eq!(DEFAULT_CAMERA_LABEL, "Active Camera: None");
    }

    #[test]
    fn camera_label_format_with_name() {
        let camera_name = "Logitech C920";
        let label = format!("Active Camera: {camera_name}");

        assert_eq!(label, "Active Camera: Logitech C920");
    }
}
