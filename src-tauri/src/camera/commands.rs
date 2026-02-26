use tauri::State;

use crate::camera::backend::CameraBackend;
use crate::camera::types::{CameraDevice, ControlDescriptor, DeviceId, FormatDescriptor};

/// Shared camera state managed by Tauri.
pub struct CameraState {
    pub backend: Box<dyn CameraBackend>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::backend::CameraBackend;
    use crate::camera::error::{CameraError, Result};
    use crate::camera::types::{
        CameraDevice, ControlDescriptor, ControlFlags, ControlId, ControlType, ControlValue,
        DeviceId, FormatDescriptor, HotplugEvent,
    };

    struct TestBackend {
        devices: Vec<CameraDevice>,
        controls: Vec<ControlDescriptor>,
        formats: Vec<FormatDescriptor>,
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
            _id: &DeviceId,
            _control: &ControlId,
            _value: ControlValue,
        ) -> Result<()> {
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
}
