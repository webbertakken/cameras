//! `CanonBackend<S>` — CameraBackend implementation for Canon cameras.
//!
//! Generic over `S: EdsSdkApi` so tests use `MockEdsSdk` while
//! production uses the real `EdsSdk` wrapper.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::camera::backend::CameraBackend;
use crate::camera::error::{CameraError, Result};
use crate::camera::types::{
    CameraDevice, ControlDescriptor, ControlId, ControlValue, DeviceId, FormatDescriptor,
    HotplugEvent,
};

use super::api::{CameraHandle, EdsSdkApi};
use super::controls::get_canon_controls;
use super::discovery::discover_cameras;
use super::types::*;

/// Internal state for a discovered Canon camera.
struct CanonCamera {
    handle: CameraHandle,
    device: CameraDevice,
}

/// Canon camera backend backed by an `EdsSdkApi` implementation.
pub struct CanonBackend<S: EdsSdkApi> {
    sdk: Arc<S>,
    cameras: Mutex<HashMap<DeviceId, CanonCamera>>,
}

impl<S: EdsSdkApi> CanonBackend<S> {
    /// Create a new Canon backend with the given SDK implementation.
    pub fn new(sdk: Arc<S>) -> Self {
        Self {
            sdk,
            cameras: Mutex::new(HashMap::new()),
        }
    }

    /// Look up the camera handle for a device ID.
    fn find_handle(&self, id: &DeviceId) -> Result<CameraHandle> {
        let cameras = self.cameras.lock().unwrap();
        cameras
            .get(id)
            .map(|c| c.handle)
            .ok_or_else(|| CameraError::DeviceNotFound(id.to_string()))
    }

    /// Map a Canon control ID string to an EDSDK property ID.
    fn control_to_property(control: &ControlId) -> Result<EdsPropertyID> {
        match control {
            ControlId::Iso => Ok(PROP_ID_ISO_SPEED),
            ControlId::Aperture => Ok(PROP_ID_AV),
            ControlId::ShutterSpeed => Ok(PROP_ID_TV),
            ControlId::ExposureCompensation => Ok(PROP_ID_EXPOSURE_COMPENSATION),
            ControlId::WhiteBalance => Ok(PROP_ID_WHITE_BALANCE),
            _ => Err(CameraError::ControlQuery(format!(
                "control '{:?}' is not a Canon property",
                control
            ))),
        }
    }
}

impl<S: EdsSdkApi + 'static> CameraBackend for CanonBackend<S> {
    fn enumerate_devices(&self) -> Result<Vec<CameraDevice>> {
        let discovered = discover_cameras(&*self.sdk)?;
        let mut cameras = self.cameras.lock().unwrap();

        // Build new map, preserving order
        let mut new_map = HashMap::new();
        let mut devices = Vec::new();

        for (handle, device) in discovered {
            devices.push(device.clone());
            new_map.insert(device.id.clone(), CanonCamera { handle, device });
        }

        *cameras = new_map;
        Ok(devices)
    }

    fn watch_hotplug(&self, _callback: Box<dyn Fn(HotplugEvent) + Send>) -> Result<()> {
        // Hotplug is handled by the composite backend level, or via
        // CanonHotplugWatcher for standalone use.
        Ok(())
    }

    fn get_controls(&self, id: &DeviceId) -> Result<Vec<ControlDescriptor>> {
        let handle = self.find_handle(id)?;
        get_canon_controls(&*self.sdk, handle)
    }

    fn get_control(&self, id: &DeviceId, control: &ControlId) -> Result<ControlValue> {
        let handle = self.find_handle(id)?;
        let prop = Self::control_to_property(control)?;
        let value = self.sdk.get_property(handle, prop)?;
        Ok(ControlValue::new(value, None, None))
    }

    fn set_control(&self, id: &DeviceId, control: &ControlId, value: ControlValue) -> Result<()> {
        let handle = self.find_handle(id)?;
        let prop = Self::control_to_property(control)?;
        self.sdk.set_property(handle, prop, value.value())
    }

    fn get_formats(&self, id: &DeviceId) -> Result<Vec<FormatDescriptor>> {
        // Verify device exists
        let _ = self.find_handle(id)?;

        // Canon live view is typically fixed resolution
        Ok(vec![FormatDescriptor {
            width: 960,
            height: 640,
            fps: 5.0,
            pixel_format: "JPEG".to_string(),
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::canon::mock::MockEdsSdk;
    use crate::camera::canon::types::PROP_ID_ISO_SPEED;

    fn make_backend() -> CanonBackend<MockEdsSdk> {
        let mock = MockEdsSdk::new()
            .with_camera("Canon EOS R5", Some("SER001"))
            .with_property(0, PROP_ID_ISO_SPEED, 0x48)
            .with_property_desc(0, PROP_ID_ISO_SPEED, vec![0x48, 0x50, 0x58]);
        CanonBackend::new(Arc::new(mock))
    }

    #[test]
    fn enumerate_devices_returns_canon_cameras() {
        let backend = make_backend();
        let devices = backend.enumerate_devices().unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "Canon EOS R5");
        assert_eq!(devices[0].id, DeviceId::new("canon:SER001"));
        assert!(devices[0].is_connected);
    }

    #[test]
    fn enumerate_updates_internal_map() {
        let backend = make_backend();

        // First enumeration populates the map
        backend.enumerate_devices().unwrap();

        // Subsequent calls to find_handle should work
        let handle = backend.find_handle(&DeviceId::new("canon:SER001"));
        assert!(handle.is_ok());
    }

    #[test]
    fn get_controls_returns_canon_controls() {
        let backend = make_backend();
        backend.enumerate_devices().unwrap();

        let controls = backend
            .get_controls(&DeviceId::new("canon:SER001"))
            .unwrap();
        assert!(!controls.is_empty());

        let iso = controls.iter().find(|c| c.id == "canon_iso");
        assert!(iso.is_some(), "should have ISO control");
    }

    #[test]
    fn get_control_reads_property_value() {
        let backend = make_backend();
        backend.enumerate_devices().unwrap();

        let value = backend
            .get_control(&DeviceId::new("canon:SER001"), &ControlId::Iso)
            .unwrap();
        assert_eq!(value.value(), 0x48);
    }

    #[test]
    fn set_control_writes_property_value() {
        let backend = make_backend();
        backend.enumerate_devices().unwrap();

        let result = backend.set_control(
            &DeviceId::new("canon:SER001"),
            &ControlId::Iso,
            ControlValue::new(0x50, None, None),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn get_formats_returns_live_view_format() {
        let backend = make_backend();
        backend.enumerate_devices().unwrap();

        let formats = backend.get_formats(&DeviceId::new("canon:SER001")).unwrap();
        assert_eq!(formats.len(), 1);
        assert_eq!(formats[0].pixel_format, "JPEG");
        assert_eq!(formats[0].width, 960);
    }

    #[test]
    fn unknown_device_returns_error() {
        let backend = make_backend();
        backend.enumerate_devices().unwrap();

        assert!(backend.get_controls(&DeviceId::new("nonexistent")).is_err());
        assert!(backend
            .get_control(&DeviceId::new("nonexistent"), &ControlId::Iso)
            .is_err());
        assert!(backend.get_formats(&DeviceId::new("nonexistent")).is_err());
    }

    #[test]
    fn non_canon_control_returns_error() {
        let backend = make_backend();
        backend.enumerate_devices().unwrap();

        let result = backend.get_control(&DeviceId::new("canon:SER001"), &ControlId::Brightness);
        assert!(result.is_err());
    }

    #[test]
    fn watch_hotplug_succeeds() {
        let backend = make_backend();
        let result = backend.watch_hotplug(Box::new(|_| {}));
        assert!(result.is_ok());
    }

    #[test]
    fn canon_backend_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CanonBackend<MockEdsSdk>>();
    }

    #[test]
    fn empty_backend_enumerates_zero_devices() {
        let mock = MockEdsSdk::new();
        let backend = CanonBackend::new(Arc::new(mock));
        let devices = backend.enumerate_devices().unwrap();
        assert!(devices.is_empty());
    }

    #[test]
    fn re_enumeration_updates_camera_list() {
        let mock = MockEdsSdk::new().with_cameras(2);
        let backend = CanonBackend::new(Arc::new(mock));

        let first = backend.enumerate_devices().unwrap();
        assert_eq!(first.len(), 2);

        let second = backend.enumerate_devices().unwrap();
        assert_eq!(second.len(), 2);
    }
}
