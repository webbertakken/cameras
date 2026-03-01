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

/// End-to-end smoke tests using `CanonBackend<MockEdsSdk>` inside a `CompositeBackend`.
///
/// These verify that the Canon backend integrates correctly with the composite
/// routing: enumeration merges, control routing, and device isolation all work
/// as expected with a realistic multi-backend setup.
#[cfg(test)]
#[cfg(feature = "canon")]
mod canon_e2e_tests {
    use super::*;
    use crate::camera::canon::backend::CanonBackend;
    use crate::camera::canon::mock::MockEdsSdk;
    use crate::camera::canon::types::PROP_ID_ISO_SPEED;
    use crate::camera::error::CameraError;
    use crate::camera::types::{
        CameraDevice, ControlDescriptor, ControlFlags, ControlId, ControlType, ControlValue,
        DeviceId, FormatDescriptor, HotplugEvent,
    };
    use std::sync::Arc;

    /// Stub backend representing a DirectShow (or similar) non-Canon camera.
    struct DirectShowStub {
        devices: Vec<CameraDevice>,
    }

    impl DirectShowStub {
        fn new() -> Self {
            Self {
                devices: vec![CameraDevice {
                    id: DeviceId::new("ds:logitech-brio"),
                    name: "Logitech BRIO".to_string(),
                    device_path: "ds://logitech".to_string(),
                    is_connected: true,
                }],
            }
        }
    }

    impl CameraBackend for DirectShowStub {
        fn enumerate_devices(&self) -> Result<Vec<CameraDevice>> {
            Ok(self.devices.clone())
        }

        fn watch_hotplug(&self, _cb: Box<dyn Fn(HotplugEvent) + Send>) -> Result<()> {
            Ok(())
        }

        fn get_controls(&self, id: &DeviceId) -> Result<Vec<ControlDescriptor>> {
            if self.devices.iter().any(|d| &d.id == id) {
                Ok(vec![ControlDescriptor {
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
                }])
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
                Ok(vec![FormatDescriptor {
                    width: 1920,
                    height: 1080,
                    fps: 30.0,
                    pixel_format: "MJPG".to_string(),
                }])
            } else {
                Err(CameraError::DeviceNotFound(id.to_string()))
            }
        }
    }

    /// Build a composite with one Canon camera (MockEdsSdk) and one DirectShow stub.
    fn make_composite() -> CompositeBackend {
        let mock = MockEdsSdk::new()
            .with_camera("Canon EOS R5", Some("SER001"))
            .with_property(0, PROP_ID_ISO_SPEED, 0x48)
            .with_property_desc(0, PROP_ID_ISO_SPEED, vec![0x48, 0x50, 0x58]);

        let canon: Box<dyn CameraBackend> = Box::new(CanonBackend::new(Arc::new(mock)));
        let ds: Box<dyn CameraBackend> = Box::new(DirectShowStub::new());

        CompositeBackend::new(vec![ds, canon])
    }

    #[test]
    fn enumerates_devices_from_both_backends() {
        let composite = make_composite();
        let devices = composite.enumerate_devices().unwrap();

        assert_eq!(devices.len(), 2, "should see one DS + one Canon device");

        let names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"Logitech BRIO"), "missing DS device");
        assert!(names.contains(&"Canon EOS R5"), "missing Canon device");
    }

    #[test]
    fn reads_canon_controls() {
        let composite = make_composite();
        composite.enumerate_devices().unwrap();

        let canon_id = DeviceId::new("canon:SER001");
        let controls = composite.get_controls(&canon_id).unwrap();

        assert!(!controls.is_empty(), "Canon should expose controls");

        let iso = controls.iter().find(|c| c.id == "canon_iso");
        assert!(iso.is_some(), "should have ISO control");

        let iso = iso.unwrap();
        assert_eq!(iso.control_type, ControlType::Select);
        assert!(iso.options.is_some(), "ISO should have selectable options");
        assert_eq!(iso.options.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn sets_and_reads_back_canon_control() {
        let composite = make_composite();
        composite.enumerate_devices().unwrap();

        let canon_id = DeviceId::new("canon:SER001");

        // Read initial ISO value
        let initial = composite.get_control(&canon_id, &ControlId::Iso).unwrap();
        assert_eq!(initial.value(), 0x48, "initial ISO should be 100 (0x48)");

        // Set ISO to 200 (0x50)
        let result = composite.set_control(
            &canon_id,
            &ControlId::Iso,
            ControlValue::new(0x50, None, None),
        );
        assert!(result.is_ok(), "setting ISO should succeed");

        // Read back the updated value
        let updated = composite.get_control(&canon_id, &ControlId::Iso).unwrap();
        assert_eq!(updated.value(), 0x50, "ISO should now be 200 (0x50)");
    }

    #[test]
    fn routes_ds_controls_to_ds_backend() {
        let composite = make_composite();
        composite.enumerate_devices().unwrap();

        let ds_id = DeviceId::new("ds:logitech-brio");
        let controls = composite.get_controls(&ds_id).unwrap();

        assert!(!controls.is_empty(), "DS device should have controls");
        assert_eq!(controls[0].id, "brightness");
    }

    #[test]
    fn unknown_device_returns_device_not_found() {
        let composite = make_composite();
        composite.enumerate_devices().unwrap();

        let unknown_id = DeviceId::new("nonexistent:device");

        let result = composite.get_controls(&unknown_id);
        assert!(result.is_err(), "unknown device should fail");

        match result.unwrap_err() {
            CameraError::DeviceNotFound(_) => {} // expected
            other => panic!("expected DeviceNotFound, got: {other}"),
        }
    }

    #[test]
    fn canon_controls_do_not_leak_to_ds_device() {
        let composite = make_composite();
        composite.enumerate_devices().unwrap();

        let ds_id = DeviceId::new("ds:logitech-brio");

        // Attempting to read a Canon-specific control on the DS device should fail
        // because the DS stub does not know about ISO (it returns a generic value,
        // but the control routing should hit the DS backend which doesn't error
        // for its own devices). The key test is that it does NOT accidentally hit
        // the Canon backend.
        let controls = composite.get_controls(&ds_id).unwrap();
        let has_canon_control = controls.iter().any(|c| c.id.starts_with("canon_"));
        assert!(
            !has_canon_control,
            "DS device should not expose Canon-specific controls"
        );
    }
}
