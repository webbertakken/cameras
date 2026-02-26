use crate::camera::error::Result;
use crate::camera::types::{
    CameraDevice, ControlDescriptor, ControlId, ControlValue, DeviceId, FormatDescriptor,
    HotplugEvent,
};

/// Platform-agnostic camera backend trait.
///
/// Implemented per-platform (DirectShow on Windows, AVFoundation on macOS,
/// V4L2 on Linux). Provides device enumeration, control access, format
/// queries, and hot-plug detection.
pub trait CameraBackend: Send + Sync {
    /// Enumerate all currently connected camera devices.
    fn enumerate_devices(&self) -> Result<Vec<CameraDevice>>;

    /// Register for hot-plug notifications.
    ///
    /// The callback fires on the backend's internal thread when a device is
    /// connected or disconnected.
    fn watch_hotplug(&self, callback: Box<dyn Fn(HotplugEvent) + Send>) -> Result<()>;

    /// Get all supported controls for a device.
    fn get_controls(&self, id: &DeviceId) -> Result<Vec<ControlDescriptor>>;

    /// Read a single control value.
    fn get_control(&self, id: &DeviceId, control: &ControlId) -> Result<ControlValue>;

    /// Write a single control value.
    fn set_control(&self, id: &DeviceId, control: &ControlId, value: ControlValue) -> Result<()>;

    /// Get supported video formats for a device.
    fn get_formats(&self, id: &DeviceId) -> Result<Vec<FormatDescriptor>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::error::CameraError;

    /// Mock backend for testing trait contract.
    struct MockBackend {
        devices: Vec<CameraDevice>,
    }

    impl CameraBackend for MockBackend {
        fn enumerate_devices(&self) -> Result<Vec<CameraDevice>> {
            Ok(self.devices.clone())
        }

        fn watch_hotplug(&self, _callback: Box<dyn Fn(HotplugEvent) + Send>) -> Result<()> {
            Ok(())
        }

        fn get_controls(&self, _id: &DeviceId) -> Result<Vec<ControlDescriptor>> {
            Ok(vec![])
        }

        fn get_control(&self, _id: &DeviceId, _control: &ControlId) -> Result<ControlValue> {
            Err(CameraError::DeviceNotFound("mock".to_string()))
        }

        fn set_control(
            &self,
            _id: &DeviceId,
            _control: &ControlId,
            _value: ControlValue,
        ) -> Result<()> {
            Ok(())
        }

        fn get_formats(&self, _id: &DeviceId) -> Result<Vec<FormatDescriptor>> {
            Ok(vec![])
        }
    }

    #[test]
    fn mock_backend_enumerate_returns_devices() {
        let backend = MockBackend {
            devices: vec![CameraDevice {
                id: DeviceId::new("test:id"),
                name: "Test Camera".to_string(),
                device_path: "test-path".to_string(),
                is_connected: true,
            }],
        };

        let devices = backend.enumerate_devices().unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "Test Camera");
    }

    #[test]
    fn mock_backend_watch_hotplug_accepts_send_callback() {
        let backend = MockBackend { devices: vec![] };
        let result = backend.watch_hotplug(Box::new(|_event| {}));
        assert!(result.is_ok());
    }

    #[test]
    fn mock_backend_get_control_returns_error_for_unknown() {
        let backend = MockBackend { devices: vec![] };
        let result = backend.get_control(&DeviceId::new("unknown"), &ControlId::Brightness);
        assert!(result.is_err());
    }

    #[test]
    fn trait_object_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn CameraBackend>>();
    }
}
