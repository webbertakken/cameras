#[allow(dead_code)]
mod camera;
#[allow(dead_code)]
mod diagnostics;
mod input;
mod integration;
mod pipeline;
mod preset;
#[allow(dead_code)]
mod preview;
mod settings;
mod tray;

use std::sync::Arc;

use tauri::Manager;

use camera::commands::{
    get_camera_controls, get_camera_formats, list_cameras, reset_camera_control,
    set_camera_control, CameraState,
};
use camera::hotplug_bridge::start_hotplug_watcher;
use preview::commands::{
    get_diagnostics, get_frame, get_thumbnail, start_preview, stop_preview, PreviewState,
};
use settings::commands::{get_saved_settings, reset_to_defaults, SettingsState};
use settings::store::SettingsStore;

/// Create the camera backend for the current platform.
///
/// When `DUMMY_CAMERA=1` is set, a simulated camera is used instead.
fn create_camera_state() -> CameraState {
    if camera::dummy::DummyBackend::is_enabled() {
        return CameraState {
            backend: Box::new(camera::dummy::DummyBackend::new()),
        };
    }

    #[cfg(target_os = "windows")]
    {
        use camera::platform::WindowsBackend;
        CameraState {
            backend: Box::new(WindowsBackend::new()),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        CameraState {
            backend: Box::new(NullBackend),
        }
    }
}

/// No-op backend used on platforms without a native camera backend.
#[cfg(not(target_os = "windows"))]
struct NullBackend;

#[cfg(not(target_os = "windows"))]
impl camera::backend::CameraBackend for NullBackend {
    fn enumerate_devices(&self) -> camera::error::Result<Vec<camera::types::CameraDevice>> {
        Ok(vec![])
    }
    fn watch_hotplug(
        &self,
        _callback: Box<dyn Fn(camera::types::HotplugEvent) + Send>,
    ) -> camera::error::Result<()> {
        Ok(())
    }
    fn get_controls(
        &self,
        _id: &camera::types::DeviceId,
    ) -> camera::error::Result<Vec<camera::types::ControlDescriptor>> {
        Ok(vec![])
    }
    fn get_control(
        &self,
        _id: &camera::types::DeviceId,
        _control: &camera::types::ControlId,
    ) -> camera::error::Result<camera::types::ControlValue> {
        Err(camera::error::CameraError::DeviceNotFound(
            "no backend".to_string(),
        ))
    }
    fn set_control(
        &self,
        _id: &camera::types::DeviceId,
        _control: &camera::types::ControlId,
        _value: camera::types::ControlValue,
    ) -> camera::error::Result<()> {
        Err(camera::error::CameraError::DeviceNotFound(
            "no backend".to_string(),
        ))
    }
    fn get_formats(
        &self,
        _id: &camera::types::DeviceId,
    ) -> camera::error::Result<Vec<camera::types::FormatDescriptor>> {
        Ok(vec![])
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .manage(create_camera_state())
        .manage(PreviewState::new())
        .invoke_handler(tauri::generate_handler![
            list_cameras,
            get_camera_controls,
            get_camera_formats,
            set_camera_control,
            reset_camera_control,
            start_preview,
            stop_preview,
            get_frame,
            get_thumbnail,
            get_diagnostics,
            reset_to_defaults,
            get_saved_settings,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::new()
                        .targets([
                            tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                            tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),
                            tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                                file_name: None,
                            }),
                        ])
                        .level(log::LevelFilter::Debug)
                        .build(),
                )?;
            }

            // Initialise settings persistence
            let settings_path = app
                .path()
                .app_data_dir()
                .expect("app data dir should be available")
                .join("cameras.json");
            let store = Arc::new(SettingsStore::new(settings_path));
            store.start_debounce_task();
            app.manage(SettingsState {
                store: Arc::clone(&store),
            });

            // Auto-apply saved settings to connected cameras
            let camera_state = app.state::<CameraState>();
            if let Ok(devices) = camera_state.backend.enumerate_devices() {
                for device in &devices {
                    let applied = settings::commands::apply_saved_settings(
                        camera_state.backend.as_ref(),
                        &store,
                        device.id.as_str(),
                    );
                    if !applied.is_empty() {
                        tracing::info!("Restored {} settings for '{}'", applied.len(), device.name);
                    }
                }
            }

            tray::setup_tray(app.handle())?;

            start_hotplug_watcher(app.handle(), camera_state.backend.as_ref());

            Ok(())
        })
        .on_window_event(|window, event| {
            // Only intercept close on the main window (hide to tray instead of quitting).
            // Other windows (e.g. settings) close and destroy normally.
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
