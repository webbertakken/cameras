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
mod virtual_camera;

use std::sync::Arc;

use tauri::{Emitter, Manager};

use camera::commands::{
    get_camera_controls, get_camera_formats, list_cameras, reset_camera_control,
    set_camera_control, CameraState,
};
use camera::hotplug_bridge::start_hotplug_watcher;
use preview::commands::{
    get_active_gpu, get_active_previews, get_diagnostics, get_encoding_stats, get_frame,
    get_thumbnail, list_gpu_adapters, set_gpu_adapter, start_all_previews, start_preview,
    stop_preview, PreviewState,
};
use preview::gpu::GpuState;
use settings::commands::{get_saved_settings, reset_to_defaults, SettingsState};
use settings::store::SettingsStore;
use virtual_camera::commands::{
    get_virtual_camera_status, start_virtual_camera, stop_virtual_camera,
};
use virtual_camera::VirtualCameraState;

/// Holds an optional Canon SDK reference for creating live view sessions.
///
/// Stored as Tauri managed state so the preview commands can access the
/// SDK when creating Canon capture sessions.
pub struct CanonSdkState {
    #[cfg(all(feature = "canon", target_os = "windows"))]
    sdk: Option<Arc<camera::canon::sdk::EdsSdk>>,
    #[cfg(all(feature = "canon", target_os = "windows"))]
    handle_map: camera::canon::backend::HandleMap,
    #[cfg(not(all(feature = "canon", target_os = "windows")))]
    _phantom: (),
}

impl CanonSdkState {
    /// Get the Canon SDK reference, if available.
    #[cfg(all(feature = "canon", target_os = "windows"))]
    pub fn sdk(&self) -> Option<&Arc<camera::canon::sdk::EdsSdk>> {
        self.sdk.as_ref()
    }

    /// Get the Canon SDK reference (always None on non-Canon builds).
    #[cfg(not(all(feature = "canon", target_os = "windows")))]
    pub fn sdk(&self) -> Option<&Arc<()>> {
        None
    }

    /// Look up the CameraHandle for a device_path (e.g. `edsdk://Canon EOS R5`).
    #[cfg(all(feature = "canon", target_os = "windows"))]
    pub fn find_handle(&self, device_path: &str) -> Option<camera::canon::api::CameraHandle> {
        self.handle_map.lock().unwrap().get(device_path).copied()
    }

    /// Look up the CameraHandle (always None on non-Canon builds).
    #[cfg(not(all(feature = "canon", target_os = "windows")))]
    pub fn find_handle(&self, _device_path: &str) -> Option<()> {
        None
    }
}

/// Create the camera backend for the current platform.
///
/// Builds a `CompositeBackend` that merges device lists from all
/// available backends:
/// - `WindowsBackend` (DirectShow) on Windows
/// - `CanonBackend` when the `canon` feature is enabled
/// - `DummyBackend` when `DUMMY_CAMERA=1` is set
///
/// Also returns a `CanonSdkState` for live view session creation.
fn create_camera_state() -> (CameraState, CanonSdkState) {
    use camera::composite::CompositeBackend;

    let mut backends: Vec<Box<dyn camera::backend::CameraBackend>> = Vec::new();

    #[cfg(target_os = "windows")]
    {
        use camera::platform::WindowsBackend;
        backends.push(Box::new(WindowsBackend::new()));
    }

    #[cfg(not(target_os = "windows"))]
    {
        backends.push(Box::new(NullBackend));
    }

    #[cfg(all(feature = "canon", target_os = "windows"))]
    let handle_map: camera::canon::backend::HandleMap =
        Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));

    #[allow(unused_mut)]
    let mut canon_sdk_state = CanonSdkState {
        #[cfg(all(feature = "canon", target_os = "windows"))]
        sdk: None,
        #[cfg(all(feature = "canon", target_os = "windows"))]
        handle_map: Arc::clone(&handle_map),
        #[cfg(not(all(feature = "canon", target_os = "windows")))]
        _phantom: (),
    };

    #[cfg(all(feature = "canon", target_os = "windows"))]
    {
        use camera::canon::backend::CanonBackend;
        use camera::canon::sdk::EdsSdk;

        match EdsSdk::new() {
            Ok(sdk) => {
                let sdk = Arc::new(sdk);
                backends.push(Box::new(CanonBackend::new(
                    Arc::clone(&sdk),
                    Arc::clone(&handle_map),
                )));
                canon_sdk_state.sdk = Some(sdk);
                tracing::info!("Canon EDSDK backend initialised");
            }
            Err(e) => {
                tracing::warn!("Canon EDSDK initialisation failed: {e}");
            }
        }
    }

    if camera::dummy::DummyBackend::is_enabled() {
        backends.push(Box::new(camera::dummy::DummyBackend::new()));
    }

    (
        CameraState {
            backend: Box::new(CompositeBackend::new(backends)),
        },
        canon_sdk_state,
    )
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
    let (camera_state, canon_sdk_state) = create_camera_state();

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .manage(camera_state)
        .manage(canon_sdk_state)
        .manage(PreviewState::new())
        .manage(GpuState::new())
        .manage(VirtualCameraState::new())
        .invoke_handler(tauri::generate_handler![
            list_cameras,
            get_camera_controls,
            get_camera_formats,
            set_camera_control,
            reset_camera_control,
            start_preview,
            start_all_previews,
            stop_preview,
            get_frame,
            get_thumbnail,
            get_diagnostics,
            get_encoding_stats,
            reset_to_defaults,
            get_saved_settings,
            list_gpu_adapters,
            get_active_gpu,
            set_gpu_adapter,
            get_active_previews,
            start_virtual_camera,
            stop_virtual_camera,
            get_virtual_camera_status,
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

            // Enumerate cameras once for both settings restore and preview auto-start.
            // Calling enumerate_devices() multiple times causes unnecessary EDSDK
            // session close/re-open cycles which can fail on some cameras.
            let camera_state = app.state::<CameraState>();
            let devices = camera_state.backend.enumerate_devices().unwrap_or_default();

            // Auto-apply saved settings to connected cameras
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

            // Auto-start preview sessions for all connected cameras
            {
                let preview_state = app.state::<PreviewState>();
                #[allow(unused_variables)]
                let canon_sdk_state = app.state::<CanonSdkState>();
                let gpu_state = app.state::<GpuState>();
                let gpu = gpu_state.context();
                let mut sessions = preview_state.sessions.lock();
                for device in &devices {
                    let device_id = device.id.as_str().to_string();
                    if sessions.contains_key(&device_id) {
                        continue;
                    }

                    // Canon live view: device_path starts with "edsdk://"
                    if device.device_path.starts_with("edsdk://") {
                        #[cfg(all(feature = "canon", target_os = "windows"))]
                        {
                            if let (Some(sdk), Some(handle)) = (
                                canon_sdk_state.sdk(),
                                canon_sdk_state.find_handle(&device.device_path),
                            ) {
                                match preview::capture::CanonCaptureSession::new(
                                    device_id.clone(),
                                    Arc::clone(sdk),
                                    handle,
                                ) {
                                    Ok(session) => {
                                        sessions.insert(
                                            device_id,
                                            preview::capture::PreviewSession::Canon(session),
                                        );
                                        tracing::info!(
                                            "Auto-started Canon preview for '{}' at startup",
                                            device.name
                                        );
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to start Canon preview for '{}': {e}",
                                            device.name
                                        );
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    let on_error = {
                        let app_handle = app.handle().clone();
                        std::sync::Arc::new(move |dev_id: &str, error: &str| {
                            let _ = app_handle.emit(
                                "preview-error",
                                preview::capture::PreviewErrorPayload {
                                    device_id: dev_id.to_string(),
                                    error: camera::error::humanise_error(error),
                                },
                            );
                        }) as preview::capture::ErrorCallback
                    };

                    let session = preview::capture::CaptureSession::new(
                        device.device_path.clone(),
                        device.name.clone(),
                        640,
                        480,
                        30.0,
                        Some(on_error),
                        gpu.clone(),
                        75,
                    );
                    sessions.insert(
                        device_id,
                        preview::capture::PreviewSession::DirectShow(session),
                    );
                    tracing::info!(
                        "Auto-started preview session for '{}' at startup",
                        device.name
                    );
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
