//! Composite backend — merges device lists from multiple backends.
//!
//! Routes control operations to the correct backend by trying each
//! until one succeeds (the backend that owns the device will succeed,
//! others will return `DeviceNotFound`).

use crate::camera::backend::CameraBackend;
use crate::camera::error::{CameraError, Result};
use crate::camera::types::{
    CameraDevice, ControlDescriptor, ControlId, ControlValue, DeviceId, FormatDescriptor,
    HotplugEvent,
};

/// A camera backend that delegates to multiple sub-backends.
///
/// `enumerate_devices` merges results from all backends (logging failures).
/// Control operations are routed by trying each backend until one succeeds.
pub struct CompositeBackend {
    backends: Vec<Box<dyn CameraBackend>>,
}

impl CompositeBackend {
    /// Create a new composite from the given backends.
    pub fn new(backends: Vec<Box<dyn CameraBackend>>) -> Self {
        Self { backends }
    }
}

impl CameraBackend for CompositeBackend {
    fn enumerate_devices(&self) -> Result<Vec<CameraDevice>> {
        let mut all = Vec::new();
        for backend in &self.backends {
            match backend.enumerate_devices() {
                Ok(devices) => all.extend(devices),
                Err(e) => tracing::warn!("Backend enumeration failed: {e}"),
            }
        }
        Ok(all)
    }

    fn watch_hotplug(&self, callback: Box<dyn Fn(HotplugEvent) + Send>) -> Result<()> {
        // Share the callback across all backends via Arc.
        // We need Send + Sync for sharing across threads, so wrap in a
        // Mutex to satisfy Sync.
        let callback = std::sync::Arc::new(std::sync::Mutex::new(callback));
        for backend in &self.backends {
            let cb = std::sync::Arc::clone(&callback);
            let result = backend.watch_hotplug(Box::new(move |event| {
                if let Ok(cb) = cb.lock() {
                    cb(event);
                }
            }));
            if let Err(e) = result {
                tracing::warn!("Backend hotplug registration failed: {e}");
            }
        }
        Ok(())
    }

    fn get_controls(&self, id: &DeviceId) -> Result<Vec<ControlDescriptor>> {
        route_to_backend(&self.backends, |b| b.get_controls(id), id)
    }

    fn get_control(&self, id: &DeviceId, control: &ControlId) -> Result<ControlValue> {
        route_to_backend(&self.backends, |b| b.get_control(id, control), id)
    }

    fn set_control(&self, id: &DeviceId, control: &ControlId, value: ControlValue) -> Result<()> {
        route_to_backend(&self.backends, |b| b.set_control(id, control, value), id)
    }

    fn get_formats(&self, id: &DeviceId) -> Result<Vec<FormatDescriptor>> {
        route_to_backend(&self.backends, |b| b.get_formats(id), id)
    }
}

/// Try each backend until one succeeds. Returns the first success or
/// `DeviceNotFound` if none match.
fn route_to_backend<T, F>(
    backends: &[Box<dyn CameraBackend>],
    operation: F,
    id: &DeviceId,
) -> Result<T>
where
    F: Fn(&dyn CameraBackend) -> Result<T>,
{
    for backend in backends {
        match operation(backend.as_ref()) {
            Ok(result) => return Ok(result),
            Err(CameraError::DeviceNotFound(_)) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(CameraError::DeviceNotFound(id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::error::CameraError;
    use crate::camera::types::{
        CameraDevice, ControlDescriptor, ControlFlags, ControlType, DeviceId, FormatDescriptor,
        HotplugEvent,
    };
    use std::sync::{Arc, Mutex};

    /// Simple test backend that returns pre-configured devices.
    struct StubBackend {
        devices: Vec<CameraDevice>,
        controls: Vec<ControlDescriptor>,
        formats: Vec<FormatDescriptor>,
    }

    impl StubBackend {
        fn new(prefix: &str, name: &str) -> Self {
            Self {
                devices: vec![CameraDevice {
                    id: DeviceId::new(format!("{prefix}:device1")),
                    name: name.to_string(),
                    device_path: format!("{prefix}://path"),
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
                    options: None,
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
    }

    impl CameraBackend for StubBackend {
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

        fn get_control(&self, id: &DeviceId, _control: &ControlId) -> Result<ControlValue> {
            if self.devices.iter().any(|d| &d.id == id) {
                Ok(ControlValue::new(128, Some(0), Some(255)))
            } else {
                Err(CameraError::DeviceNotFound(id.to_string()))
            }
        }

        fn set_control(
            &self,
            id: &DeviceId,
            _control: &ControlId,
            _value: ControlValue,
        ) -> Result<()> {
            if self.devices.iter().any(|d| &d.id == id) {
                Ok(())
            } else {
                Err(CameraError::DeviceNotFound(id.to_string()))
            }
        }

        fn get_formats(&self, id: &DeviceId) -> Result<Vec<FormatDescriptor>> {
            if self.devices.iter().any(|d| &d.id == id) {
                Ok(self.formats.clone())
            } else {
                Err(CameraError::DeviceNotFound(id.to_string()))
            }
        }
    }

    /// Backend that always fails enumeration.
    struct FailingBackend;

    impl CameraBackend for FailingBackend {
        fn enumerate_devices(&self) -> Result<Vec<CameraDevice>> {
            Err(CameraError::Enumeration("backend unavailable".to_string()))
        }
        fn watch_hotplug(&self, _callback: Box<dyn Fn(HotplugEvent) + Send>) -> Result<()> {
            Err(CameraError::Hotplug("unavailable".to_string()))
        }
        fn get_controls(&self, id: &DeviceId) -> Result<Vec<ControlDescriptor>> {
            Err(CameraError::DeviceNotFound(id.to_string()))
        }
        fn get_control(&self, id: &DeviceId, _control: &ControlId) -> Result<ControlValue> {
            Err(CameraError::DeviceNotFound(id.to_string()))
        }
        fn set_control(
            &self,
            id: &DeviceId,
            _control: &ControlId,
            _value: ControlValue,
        ) -> Result<()> {
            Err(CameraError::DeviceNotFound(id.to_string()))
        }
        fn get_formats(&self, id: &DeviceId) -> Result<Vec<FormatDescriptor>> {
            Err(CameraError::DeviceNotFound(id.to_string()))
        }
    }

    #[test]
    fn merges_device_lists_from_multiple_backends() {
        let composite = CompositeBackend::new(vec![
            Box::new(StubBackend::new("ds", "Logitech BRIO")),
            Box::new(StubBackend::new("canon", "Canon EOS R5")),
        ]);

        let devices = composite.enumerate_devices().unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].name, "Logitech BRIO");
        assert_eq!(devices[1].name, "Canon EOS R5");
    }

    #[test]
    fn failing_backend_does_not_block_enumeration() {
        let composite = CompositeBackend::new(vec![
            Box::new(FailingBackend),
            Box::new(StubBackend::new("ds", "Logitech BRIO")),
        ]);

        let devices = composite.enumerate_devices().unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "Logitech BRIO");
    }

    #[test]
    fn routes_controls_to_correct_backend() {
        let composite = CompositeBackend::new(vec![
            Box::new(StubBackend::new("ds", "Logitech BRIO")),
            Box::new(StubBackend::new("canon", "Canon EOS R5")),
        ]);

        // Should route to the canon backend
        let controls = composite
            .get_controls(&DeviceId::new("canon:device1"))
            .unwrap();
        assert!(!controls.is_empty());

        // Should route to the ds backend
        let controls = composite
            .get_controls(&DeviceId::new("ds:device1"))
            .unwrap();
        assert!(!controls.is_empty());
    }

    #[test]
    fn unknown_device_returns_not_found() {
        let composite =
            CompositeBackend::new(vec![Box::new(StubBackend::new("ds", "Logitech BRIO"))]);

        let result = composite.get_controls(&DeviceId::new("nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn routes_get_control() {
        let composite =
            CompositeBackend::new(vec![Box::new(StubBackend::new("ds", "Logitech BRIO"))]);

        let value = composite
            .get_control(&DeviceId::new("ds:device1"), &ControlId::Brightness)
            .unwrap();
        assert_eq!(value.value(), 128);
    }

    #[test]
    fn routes_set_control() {
        let composite =
            CompositeBackend::new(vec![Box::new(StubBackend::new("ds", "Logitech BRIO"))]);

        let result = composite.set_control(
            &DeviceId::new("ds:device1"),
            &ControlId::Brightness,
            ControlValue::new(200, Some(0), Some(255)),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn routes_get_formats() {
        let composite =
            CompositeBackend::new(vec![Box::new(StubBackend::new("ds", "Logitech BRIO"))]);

        let formats = composite.get_formats(&DeviceId::new("ds:device1")).unwrap();
        assert_eq!(formats.len(), 1);
    }

    #[test]
    fn hotplug_registered_on_all_backends() {
        let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);

        let composite =
            CompositeBackend::new(vec![Box::new(StubBackend::new("ds", "Logitech BRIO"))]);

        let result = composite.watch_hotplug(Box::new(move |_event| {
            events_clone.lock().unwrap().push("event".to_string());
        }));
        assert!(result.is_ok());
    }

    #[test]
    fn hotplug_failure_in_one_backend_does_not_block_others() {
        let composite = CompositeBackend::new(vec![
            Box::new(FailingBackend),
            Box::new(StubBackend::new("ds", "Logitech BRIO")),
        ]);

        // Should still succeed overall
        let result = composite.watch_hotplug(Box::new(|_| {}));
        assert!(result.is_ok());
    }

    #[test]
    fn empty_composite_enumerates_zero_devices() {
        let composite = CompositeBackend::new(vec![]);
        let devices = composite.enumerate_devices().unwrap();
        assert!(devices.is_empty());
    }

    #[test]
    fn composite_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CompositeBackend>();
    }
}
