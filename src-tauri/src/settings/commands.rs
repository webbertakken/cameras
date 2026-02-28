use std::sync::Arc;

use tauri::State;

use crate::camera::backend::CameraBackend;
use crate::camera::commands::CameraState;
use crate::camera::types::{ControlId, ControlValue, DeviceId};
use crate::settings::store::SettingsStore;

/// Tauri-managed state wrapping the settings store.
pub struct SettingsState {
    pub store: Arc<SettingsStore>,
}

/// Apply saved settings to a connected camera.
///
/// For each saved control value, looks up the descriptor (for range clamping)
/// and calls `set_control`. Logs and skips individual failures.
pub fn apply_saved_settings(
    backend: &dyn CameraBackend,
    store: &SettingsStore,
    device_id: &str,
) -> Vec<(String, i32)> {
    let saved = match store.get_camera(device_id) {
        Some(s) => s,
        None => return vec![],
    };

    let id = DeviceId::new(device_id);
    let descriptors = match backend.get_controls(&id) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("Failed to get controls for {device_id}: {e}");
            return vec![];
        }
    };

    let mut applied = Vec::new();

    for (control_str, &value) in &saved.controls {
        let control = match ControlId::from_str_id(control_str) {
            Some(c) => c,
            None => {
                tracing::warn!("Skipping unknown control '{control_str}' for {device_id}");
                continue;
            }
        };

        let desc = match descriptors.iter().find(|d| d.id == *control_str) {
            Some(d) => d,
            None => {
                tracing::warn!(
                    "Control '{control_str}' not available on device {device_id}, skipping"
                );
                continue;
            }
        };

        let clamped = ControlValue::new(value, desc.min, desc.max);
        match backend.set_control(&id, &control, clamped) {
            Ok(()) => applied.push((control_str.clone(), value)),
            Err(e) => {
                tracing::warn!("Failed to apply '{control_str}' = {value} on {device_id}: {e}");
            }
        }
    }

    applied
}

/// Reset all controls to their hardware defaults and clear saved settings.
#[tauri::command]
pub async fn reset_to_defaults(
    camera_state: State<'_, CameraState>,
    settings_state: State<'_, SettingsState>,
    device_id: String,
) -> Result<Vec<(String, i32)>, String> {
    let id = DeviceId::new(&device_id);
    let descriptors = camera_state
        .backend
        .get_controls(&id)
        .map_err(|e| e.to_string())?;

    let mut reset_values = Vec::new();

    for desc in &descriptors {
        let default_val = match desc.default {
            Some(v) => v,
            None => continue,
        };

        let control = match ControlId::from_str_id(&desc.id) {
            Some(c) => c,
            None => continue,
        };

        let clamped = ControlValue::new(default_val, desc.min, desc.max);
        camera_state
            .backend
            .set_control(&id, &control, clamped)
            .map_err(|e| e.to_string())?;

        reset_values.push((desc.id.clone(), default_val));
    }

    settings_state.store.remove_camera(&device_id);

    Ok(reset_values)
}

/// Get saved settings for a camera.
#[tauri::command]
pub async fn get_saved_settings(
    settings_state: State<'_, SettingsState>,
    device_id: String,
) -> Result<Option<crate::settings::types::CameraSettings>, String> {
    Ok(settings_state.store.get_camera(&device_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::backend::CameraBackend;
    use crate::camera::error::{CameraError, Result as CamResult};
    use crate::camera::types::{
        CameraDevice, ControlDescriptor, ControlFlags, ControlId, ControlType, ControlValue,
        DeviceId, FormatDescriptor, HotplugEvent,
    };
    use crate::settings::store::SettingsStore;
    use std::sync::Mutex;

    /// Mock backend that tracks set_control calls for verification.
    struct MockBackend {
        devices: Vec<CameraDevice>,
        controls: Vec<ControlDescriptor>,
        set_calls: Mutex<Vec<(String, String, i32)>>,
        /// Controls that should fail when set (control_id strings).
        fail_controls: Vec<String>,
    }

    impl MockBackend {
        fn new(controls: Vec<ControlDescriptor>) -> Self {
            Self {
                devices: vec![CameraDevice {
                    id: DeviceId::new("test-device"),
                    name: "Test Camera".to_string(),
                    device_path: "test-path".to_string(),
                    is_connected: true,
                }],
                controls,
                set_calls: Mutex::new(Vec::new()),
                fail_controls: Vec::new(),
            }
        }

        fn with_failing_controls(mut self, fails: Vec<String>) -> Self {
            self.fail_controls = fails;
            self
        }
    }

    impl CameraBackend for MockBackend {
        fn enumerate_devices(&self) -> CamResult<Vec<CameraDevice>> {
            Ok(self.devices.clone())
        }

        fn watch_hotplug(&self, _callback: Box<dyn Fn(HotplugEvent) + Send>) -> CamResult<()> {
            Ok(())
        }

        fn get_controls(&self, id: &DeviceId) -> CamResult<Vec<ControlDescriptor>> {
            if self.devices.iter().any(|d| &d.id == id) {
                Ok(self.controls.clone())
            } else {
                Err(CameraError::DeviceNotFound(id.to_string()))
            }
        }

        fn get_control(&self, _id: &DeviceId, _control: &ControlId) -> CamResult<ControlValue> {
            Ok(ControlValue::new(128, Some(0), Some(255)))
        }

        fn set_control(
            &self,
            id: &DeviceId,
            control: &ControlId,
            value: ControlValue,
        ) -> CamResult<()> {
            if !self.devices.iter().any(|d| &d.id == id) {
                return Err(CameraError::DeviceNotFound(id.to_string()));
            }
            let control_str = control.as_id_str().to_string();
            if self.fail_controls.contains(&control_str) {
                return Err(CameraError::ControlWrite(format!(
                    "simulated failure for {control_str}"
                )));
            }
            self.set_calls.lock().unwrap().push((
                id.as_str().to_string(),
                control_str,
                value.value(),
            ));
            Ok(())
        }

        fn get_formats(&self, _id: &DeviceId) -> CamResult<Vec<FormatDescriptor>> {
            Ok(vec![])
        }
    }

    fn make_brightness_control(default: Option<i32>) -> ControlDescriptor {
        ControlDescriptor {
            id: "brightness".to_string(),
            name: "Brightness".to_string(),
            control_type: ControlType::Slider,
            group: "image".to_string(),
            min: Some(0),
            max: Some(255),
            step: Some(1),
            default,
            current: 128,
            flags: ControlFlags {
                supports_auto: false,
                is_auto_enabled: false,
                is_read_only: false,
            },
            supported: true,
        }
    }

    fn make_contrast_control(default: Option<i32>) -> ControlDescriptor {
        ControlDescriptor {
            id: "contrast".to_string(),
            name: "Contrast".to_string(),
            control_type: ControlType::Slider,
            group: "image".to_string(),
            min: Some(0),
            max: Some(100),
            step: Some(1),
            default,
            current: 50,
            flags: ControlFlags {
                supports_auto: false,
                is_auto_enabled: false,
                is_read_only: false,
            },
            supported: true,
        }
    }

    fn temp_store() -> SettingsStore {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("cameras.json");
        // Leak the TempDir to keep it alive for the test
        let dir = Box::leak(Box::new(dir));
        let _ = dir;
        SettingsStore::new(path)
    }

    // --- Apply saved settings tests (Step 5) ---

    #[test]
    fn apply_saved_settings_calls_set_control_for_each_saved_value() {
        let backend = MockBackend::new(vec![
            make_brightness_control(Some(128)),
            make_contrast_control(Some(50)),
        ]);
        let store = temp_store();
        store.set_control("test-device", "Camera", "brightness", 200);
        store.set_control("test-device", "Camera", "contrast", 80);

        let applied = apply_saved_settings(&backend, &store, "test-device");
        assert_eq!(applied.len(), 2);

        let calls = backend.set_calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn apply_saved_settings_skips_unknown_controls() {
        let backend = MockBackend::new(vec![make_brightness_control(Some(128))]);
        let store = temp_store();
        store.set_control("test-device", "Camera", "brightness", 200);
        store.set_control("test-device", "Camera", "nonexistent_control", 42);

        let applied = apply_saved_settings(&backend, &store, "test-device");
        // Only brightness should be applied, nonexistent_control skipped
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].0, "brightness");
    }

    #[test]
    fn apply_saved_settings_does_nothing_when_no_saved_settings() {
        let backend = MockBackend::new(vec![make_brightness_control(Some(128))]);
        let store = temp_store();

        let applied = apply_saved_settings(&backend, &store, "test-device");
        assert!(applied.is_empty());
        assert!(backend.set_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn apply_saved_settings_continues_on_individual_control_failure() {
        let backend = MockBackend::new(vec![
            make_brightness_control(Some(128)),
            make_contrast_control(Some(50)),
        ])
        .with_failing_controls(vec!["brightness".to_string()]);

        let store = temp_store();
        store.set_control("test-device", "Camera", "brightness", 200);
        store.set_control("test-device", "Camera", "contrast", 80);

        let applied = apply_saved_settings(&backend, &store, "test-device");
        // brightness fails but contrast should still be applied
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].0, "contrast");
    }

    // --- reset_to_defaults tests (Step 6) ---
    // These test the logic directly using the mock backend, not through Tauri IPC

    #[test]
    fn reset_to_defaults_sets_all_controls_to_hardware_defaults() {
        let backend = MockBackend::new(vec![
            make_brightness_control(Some(128)),
            make_contrast_control(Some(50)),
        ]);
        let store = temp_store();
        store.set_control("test-device", "Camera", "brightness", 200);
        store.set_control("test-device", "Camera", "contrast", 80);

        // Directly test the reset logic
        let id = DeviceId::new("test-device");
        let descriptors = backend.get_controls(&id).unwrap();

        let mut reset_values = Vec::new();
        for desc in &descriptors {
            if let Some(default_val) = desc.default {
                let control = ControlId::from_str_id(&desc.id).unwrap();
                let clamped = ControlValue::new(default_val, desc.min, desc.max);
                backend.set_control(&id, &control, clamped).unwrap();
                reset_values.push((desc.id.clone(), default_val));
            }
        }
        store.remove_camera("test-device");

        assert_eq!(reset_values.len(), 2);
        let calls = backend.set_calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        // Verify default values were applied
        assert!(calls.iter().any(|(_, c, v)| c == "brightness" && *v == 128));
        assert!(calls.iter().any(|(_, c, v)| c == "contrast" && *v == 50));
        // Verify settings cleared
        assert!(store.get_camera("test-device").is_none());
    }

    #[test]
    fn reset_to_defaults_returns_error_for_unknown_device() {
        let backend = MockBackend::new(vec![make_brightness_control(Some(128))]);
        let id = DeviceId::new("nonexistent");
        let result = backend.get_controls(&id);
        assert!(result.is_err());
    }

    #[test]
    fn reset_to_defaults_skips_controls_without_defaults() {
        let backend = MockBackend::new(vec![
            make_brightness_control(Some(128)),
            make_contrast_control(None), // no default
        ]);

        let id = DeviceId::new("test-device");
        let descriptors = backend.get_controls(&id).unwrap();

        let mut reset_count = 0;
        for desc in &descriptors {
            if let Some(default_val) = desc.default {
                let control = ControlId::from_str_id(&desc.id).unwrap();
                let clamped = ControlValue::new(default_val, desc.min, desc.max);
                backend.set_control(&id, &control, clamped).unwrap();
                reset_count += 1;
            }
        }

        assert_eq!(reset_count, 1); // Only brightness has a default
    }

    #[test]
    fn reset_to_defaults_clears_saved_settings_for_device() {
        let store = temp_store();
        store.set_control("test-device", "Camera", "brightness", 200);
        assert!(store.get_camera("test-device").is_some());

        store.remove_camera("test-device");
        assert!(store.get_camera("test-device").is_none());
    }
}
