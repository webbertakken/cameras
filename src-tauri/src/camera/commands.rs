use tauri::State;

use crate::camera::backend::CameraBackend;
use crate::camera::types::{
    CameraDevice, ControlDescriptor, ControlId, ControlValue, DeviceId, FormatDescriptor,
};

/// Shared camera state managed by Tauri.
pub struct CameraState {
    pub backend: Box<dyn CameraBackend>,
}

/// Parse a string control ID to a `ControlId` enum, returning a
/// human-readable error on failure.
fn parse_control_id(s: &str) -> Result<ControlId, String> {
    ControlId::from_str_id(s).ok_or_else(|| format!("Unknown control: '{s}'"))
}

/// List all connected cameras.
#[tauri::command]
pub async fn list_cameras(state: State<'_, CameraState>) -> Result<Vec<CameraDevice>, String> {
    state.backend.enumerate_devices().map_err(|e| e.to_string())
}

/// Get all supported controls for a camera.
#[tauri::command]
pub async fn get_camera_controls(
    state: State<'_, CameraState>,
    device_id: String,
) -> Result<Vec<ControlDescriptor>, String> {
    let id = DeviceId::new(device_id);
    state.backend.get_controls(&id).map_err(|e| e.to_string())
}

/// Get supported video formats for a camera.
#[tauri::command]
pub async fn get_camera_formats(
    state: State<'_, CameraState>,
    device_id: String,
) -> Result<Vec<FormatDescriptor>, String> {
    let id = DeviceId::new(device_id);
    state.backend.get_formats(&id).map_err(|e| e.to_string())
}

/// Set a camera control value.
#[tauri::command]
pub async fn set_camera_control(
    state: State<'_, CameraState>,
    device_id: String,
    control_id: String,
    value: i32,
) -> Result<(), String> {
    let id = DeviceId::new(device_id);
    let control = parse_control_id(&control_id)?;

    // Look up the descriptor to know the valid range
    let descriptors = state.backend.get_controls(&id).map_err(|e| e.to_string())?;
    let desc = descriptors
        .iter()
        .find(|d| d.id == control_id)
        .ok_or_else(|| {
            format!(
                "Control '{}' not supported on this device",
                control.display_name()
            )
        })?;

    if desc.flags.is_read_only {
        return Err(format!("Control '{}' is read-only", control.display_name()));
    }

    let clamped = ControlValue::new(value, desc.min, desc.max);
    state
        .backend
        .set_control(&id, &control, clamped)
        .map_err(|e| e.to_string())
}

/// Reset a camera control to its default value.
///
/// Returns the default value that was applied.
#[tauri::command]
pub async fn reset_camera_control(
    state: State<'_, CameraState>,
    device_id: String,
    control_id: String,
) -> Result<i32, String> {
    let id = DeviceId::new(device_id);
    let control = parse_control_id(&control_id)?;

    let descriptors = state.backend.get_controls(&id).map_err(|e| e.to_string())?;
    let desc = descriptors
        .iter()
        .find(|d| d.id == control_id)
        .ok_or_else(|| {
            format!(
                "Control '{}' not supported on this device",
                control.display_name()
            )
        })?;

    let default_val = desc
        .default
        .ok_or_else(|| format!("No default value for '{}'", control.display_name()))?;

    let clamped = ControlValue::new(default_val, desc.min, desc.max);
    state
        .backend
        .set_control(&id, &control, clamped)
        .map_err(|e| e.to_string())?;

    Ok(default_val)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::backend::CameraBackend;
    use crate::camera::error::{CameraError, Result};
    use crate::camera::types::{
        CameraDevice, ControlDescriptor, ControlFlags, ControlId, ControlType, ControlValue,
        DeviceId, FormatDescriptor, HotplugEvent,
    };
    use std::sync::Mutex;

    struct TestBackend {
        devices: Vec<CameraDevice>,
        controls: Vec<ControlDescriptor>,
        formats: Vec<FormatDescriptor>,
        last_set: Mutex<Option<(ControlId, i32)>>,
    }

    impl CameraBackend for TestBackend {
        fn enumerate_devices(&self) -> Result<Vec<CameraDevice>> {
            Ok(self.devices.clone())
        }

        fn watch_hotplug(&self, _callback: Box<dyn Fn(HotplugEvent) + Send>) -> Result<()> {
            Ok(())
        }

        fn get_controls(&self, id: &DeviceId) -> Result<Vec<ControlDescriptor>> {
            if self.devices.iter().any(|d| &d.id == id) {
                Ok(self.controls.clone())
            } else {
                Err(CameraError::DeviceNotFound(id.to_string()))
            }
        }

        fn get_control(&self, _id: &DeviceId, _control: &ControlId) -> Result<ControlValue> {
            Ok(ControlValue::new(128, Some(0), Some(255)))
        }

        fn set_control(
            &self,
            id: &DeviceId,
            control: &ControlId,
            value: ControlValue,
        ) -> Result<()> {
            if !self.devices.iter().any(|d| &d.id == id) {
                return Err(CameraError::DeviceNotFound(id.to_string()));
            }
            *self.last_set.lock().unwrap() = Some((*control, value.value()));
            Ok(())
        }

        fn get_formats(&self, id: &DeviceId) -> Result<Vec<FormatDescriptor>> {
            if self.devices.iter().any(|d| &d.id == id) {
                Ok(self.formats.clone())
            } else {
                Err(CameraError::DeviceNotFound(id.to_string()))
            }
        }
    }

    fn make_test_backend() -> TestBackend {
        TestBackend {
            devices: vec![CameraDevice {
                id: DeviceId::new("test-device"),
                name: "Test Camera".to_string(),
                device_path: "test-path".to_string(),
                is_connected: true,
            }],
            controls: vec![ControlDescriptor {
                id: "brightness".to_string(),
                name: "Brightness".to_string(),
                control_type: ControlType::Slider,
                group: "image".to_string(),
                min: Some(0),
                max: Some(255),
                step: Some(1),
                default: Some(128),
                current: 128,
                flags: ControlFlags {
                    supports_auto: false,
                    is_auto_enabled: false,
                    is_read_only: false,
                },
                supported: true,
            }],
            formats: vec![FormatDescriptor {
                width: 1920,
                height: 1080,
                fps: 30.0,
                pixel_format: "MJPG".to_string(),
            }],
            last_set: Mutex::new(None),
        }
    }

    #[test]
    fn list_cameras_returns_serialisable_json() {
        let backend = make_test_backend();
        let devices = backend.enumerate_devices().unwrap();
        let json = serde_json::to_value(&devices).unwrap();
        assert!(json.is_array());
        assert_eq!(json[0]["name"], "Test Camera");
    }

    #[test]
    fn get_controls_with_valid_device_returns_controls() {
        let backend = make_test_backend();
        let controls = backend.get_controls(&DeviceId::new("test-device")).unwrap();
        assert!(!controls.is_empty());
        assert_eq!(controls[0].id, "brightness");
    }

    #[test]
    fn get_controls_with_invalid_device_returns_error() {
        let backend = make_test_backend();
        let result = backend.get_controls(&DeviceId::new("nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn get_formats_with_valid_device_returns_formats() {
        let backend = make_test_backend();
        let formats = backend.get_formats(&DeviceId::new("test-device")).unwrap();
        assert!(!formats.is_empty());
        assert_eq!(formats[0].width, 1920);
        assert_eq!(formats[0].height, 1080);
    }

    #[test]
    fn get_formats_with_invalid_device_returns_error() {
        let backend = make_test_backend();
        let result = backend.get_formats(&DeviceId::new("nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn camera_state_holds_backend() {
        let state = CameraState {
            backend: Box::new(make_test_backend()),
        };
        let devices = state.backend.enumerate_devices().unwrap();
        assert_eq!(devices.len(), 1);
    }

    // --- parse_control_id tests ---

    #[test]
    fn parse_control_id_accepts_valid_strings() {
        assert_eq!(
            parse_control_id("brightness").unwrap(),
            ControlId::Brightness
        );
        assert_eq!(parse_control_id("exposure").unwrap(), ControlId::Exposure);
        assert_eq!(
            parse_control_id("white_balance").unwrap(),
            ControlId::WhiteBalance
        );
    }

    #[test]
    fn parse_control_id_rejects_unknown_strings() {
        let err = parse_control_id("nonexistent").unwrap_err();
        assert!(err.contains("Unknown control"));
        assert!(err.contains("nonexistent"));
    }

    #[test]
    fn parse_control_id_rejects_empty_string() {
        assert!(parse_control_id("").is_err());
    }

    // --- set_camera_control tests (via backend) ---

    #[test]
    fn set_control_with_valid_device_and_control_succeeds() {
        let backend = make_test_backend();
        let id = DeviceId::new("test-device");
        let control = ControlId::Brightness;
        let value = ControlValue::new(200, Some(0), Some(255));
        let result = backend.set_control(&id, &control, value);
        assert!(result.is_ok());
        let last = backend.last_set.lock().unwrap();
        assert_eq!(*last, Some((ControlId::Brightness, 200)));
    }

    #[test]
    fn set_control_with_invalid_device_returns_error() {
        let backend = make_test_backend();
        let id = DeviceId::new("nonexistent");
        let control = ControlId::Brightness;
        let value = ControlValue::new(100, Some(0), Some(255));
        let result = backend.set_control(&id, &control, value);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not found"),
            "error should mention not found: {err}"
        );
    }

    // --- reset_camera_control tests (logic) ---

    #[test]
    fn reset_reads_default_value_from_descriptor() {
        let backend = make_test_backend();
        let id = DeviceId::new("test-device");
        let descriptors = backend.get_controls(&id).unwrap();
        let desc = descriptors.iter().find(|d| d.id == "brightness").unwrap();
        assert_eq!(desc.default, Some(128));

        // Simulate reset: set to default
        let default_val = desc.default.unwrap();
        let clamped = ControlValue::new(default_val, desc.min, desc.max);
        let result = backend.set_control(&id, &ControlId::Brightness, clamped);
        assert!(result.is_ok());
        let last = backend.last_set.lock().unwrap();
        assert_eq!(*last, Some((ControlId::Brightness, 128)));
    }

    // --- error message tests ---

    #[test]
    fn set_control_error_for_invalid_device_is_human_readable() {
        let backend = make_test_backend();
        let result = backend.set_control(
            &DeviceId::new("nonexistent"),
            &ControlId::Brightness,
            ControlValue::new(100, Some(0), Some(255)),
        );
        let err = result.unwrap_err().to_string();
        // Should not contain raw HRESULT codes, should be descriptive
        assert!(
            err.contains("not found"),
            "error should be human-readable: {err}"
        );
    }

    #[test]
    fn parse_control_id_error_includes_control_name() {
        let err = parse_control_id("fake_control").unwrap_err();
        assert!(
            err.contains("fake_control"),
            "error should include the attempted control name: {err}"
        );
    }
}
