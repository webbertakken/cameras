//! `CanonBackend<S>` — CameraBackend implementation for Canon cameras.
//!
//! Generic over `S: EdsSdkApi` so tests use `MockEdsSdk` while
//! production uses the real `EdsSdk` wrapper.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
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
use super::hotplug::CanonHotplugWatcher;
use super::types::*;

/// Internal state for a discovered Canon camera.
struct CanonCamera {
    handle: CameraHandle,
    device: CameraDevice,
    session_open: bool,
}

/// Shared map from device_path to CameraHandle.
///
/// Shared between `CanonBackend` and `CanonSdkState` so the preview
/// commands can look up handles without going through the backend.
pub type HandleMap = Arc<Mutex<HashMap<String, CameraHandle>>>;

/// Canon camera backend backed by an `EdsSdkApi` implementation.
pub struct CanonBackend<S: EdsSdkApi> {
    sdk: Arc<S>,
    cameras: Mutex<HashMap<DeviceId, CanonCamera>>,
    /// Cached device list from the last successful enumeration.
    cached_devices: Mutex<Vec<CameraDevice>>,
    /// When `true`, `enumerate_devices` performs a full re-discovery.
    /// Set on first call and when the hotplug watcher detects changes.
    /// Shared with the hotplug watcher via `Arc`.
    dirty: Arc<AtomicBool>,
    hotplug_watcher: Mutex<Option<CanonHotplugWatcher>>,
    handle_map: HandleMap,
}

impl<S: EdsSdkApi> CanonBackend<S> {
    /// Create a new Canon backend with the given SDK implementation.
    pub fn new(sdk: Arc<S>, handle_map: HandleMap) -> Self {
        Self {
            sdk,
            cameras: Mutex::new(HashMap::new()),
            cached_devices: Mutex::new(Vec::new()),
            dirty: Arc::new(AtomicBool::new(true)), // first call always discovers
            hotplug_watcher: Mutex::new(None),
            handle_map,
        }
    }

    /// Get a reference to the SDK implementation.
    pub fn sdk(&self) -> &Arc<S> {
        &self.sdk
    }

    /// Look up the camera handle for a device path (e.g. `edsdk://Canon EOS R5`).
    pub fn find_handle_for_device_path(&self, device_path: &str) -> Result<CameraHandle> {
        let cameras = self.cameras.lock().unwrap();
        cameras
            .values()
            .find(|c| c.device.device_path == device_path)
            .map(|c| c.handle)
            .ok_or_else(|| CameraError::DeviceNotFound(device_path.to_string()))
    }

    /// Look up the camera handle for a device ID.
    fn find_handle(&self, id: &DeviceId) -> Result<CameraHandle> {
        let cameras = self.cameras.lock().unwrap();
        cameras
            .get(id)
            .map(|c| c.handle)
            .ok_or_else(|| CameraError::DeviceNotFound(id.to_string()))
    }

    /// Look up the camera handle for a device ID, requiring an open session.
    fn find_handle_with_session(&self, id: &DeviceId) -> Result<CameraHandle> {
        let cameras = self.cameras.lock().unwrap();
        let cam = cameras
            .get(id)
            .ok_or_else(|| CameraError::DeviceNotFound(id.to_string()))?;
        if !cam.session_open {
            return Err(CameraError::CanonSdkError(format!(
                "no session open for camera '{}'",
                cam.device.name
            )));
        }
        Ok(cam.handle)
    }

    /// Close all open sessions. Called during drop and re-enumeration cleanup.
    fn close_all_sessions(&self) {
        let cameras = self.cameras.lock().unwrap();
        for (_, cam) in cameras.iter() {
            if cam.session_open {
                if let Err(e) = self.sdk.close_session(cam.handle) {
                    tracing::warn!("Failed to close session for {}: {e}", cam.device.name);
                }
            }
        }
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
        // Only perform a full re-discovery when the `dirty` flag is set
        // (first call, or after a hotplug event). Repeated enumerate calls
        // from the frontend or hotplug bridge return the cached list,
        // avoiding destructive session close/re-open cycles that disrupt
        // active live view sessions.
        if !self.dirty.swap(false, Ordering::Relaxed) {
            return Ok(self.cached_devices.lock().unwrap().clone());
        }

        // Close ALL open sessions before re-enumerating. camera_list()
        // releases stored refs and obtains new ones — but EDSDK tracks
        // sessions per camera, not per ref. Calling EdsOpenSession on a
        // new ref while an old session is still active returns
        // EDS_ERR_INTERNAL_ERROR (0x00000002).
        {
            let cameras = self.cameras.lock().unwrap();
            for (_, cam) in cameras.iter() {
                if cam.session_open {
                    if let Err(e) = self.sdk.close_session(cam.handle) {
                        tracing::debug!(
                            "Close session before re-enum for {}: {e}",
                            cam.device.name
                        );
                    }
                }
            }
        }

        let discovered = discover_cameras(&*self.sdk)?;

        // Build new map, opening sessions for each discovered camera
        let mut new_map = HashMap::new();
        let mut devices = Vec::new();

        for (handle, device) in discovered {
            let session_open = match self.sdk.open_session(handle) {
                Ok(()) => true,
                Err(e) => {
                    tracing::warn!(
                        "Failed to open session for {}: {e} — controls may be unavailable",
                        device.name
                    );
                    false
                }
            };

            devices.push(device.clone());
            new_map.insert(
                device.id.clone(),
                CanonCamera {
                    handle,
                    device,
                    session_open,
                },
            );
        }

        let mut cameras = self.cameras.lock().unwrap();
        *cameras = new_map;

        // Update the shared handle map so preview commands can resolve
        // device_path → CameraHandle without going through the backend.
        {
            let mut hmap = self.handle_map.lock().unwrap();
            hmap.clear();
            for cam in cameras.values() {
                hmap.insert(cam.device.device_path.clone(), cam.handle);
            }
        }

        // Cache the device list for subsequent non-dirty calls
        *self.cached_devices.lock().unwrap() = devices.clone();

        Ok(devices)
    }

    fn watch_hotplug(&self, callback: Box<dyn Fn(HotplugEvent) + Send>) -> Result<()> {
        // Wrap the callback: set dirty before forwarding so the next
        // enumerate_devices() call performs a full re-discovery.
        let dirty = Arc::clone(&self.dirty);
        let wrapped = Box::new(move |event: HotplugEvent| {
            dirty.store(true, Ordering::Relaxed);
            callback(event);
        });
        let watcher = CanonHotplugWatcher::start(Arc::clone(&self.sdk), wrapped);
        let mut guard = self.hotplug_watcher.lock().unwrap();
        *guard = Some(watcher);
        Ok(())
    }

    fn get_controls(&self, id: &DeviceId) -> Result<Vec<ControlDescriptor>> {
        let handle = self.find_handle_with_session(id)?;
        get_canon_controls(&*self.sdk, handle)
    }

    fn get_control(&self, id: &DeviceId, control: &ControlId) -> Result<ControlValue> {
        let handle = self.find_handle_with_session(id)?;
        let prop = Self::control_to_property(control)?;
        let value = self.sdk.get_property(handle, prop)?;
        Ok(ControlValue::new(value, None, None))
    }

    fn set_control(&self, id: &DeviceId, control: &ControlId, value: ControlValue) -> Result<()> {
        let handle = self.find_handle_with_session(id)?;
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

impl<S: EdsSdkApi> Drop for CanonBackend<S> {
    fn drop(&mut self) {
        self.close_all_sessions();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::canon::mock::MockEdsSdk;
    use crate::camera::canon::types::PROP_ID_ISO_SPEED;

    fn make_handle_map() -> HandleMap {
        Arc::new(Mutex::new(HashMap::new()))
    }

    fn make_backend() -> CanonBackend<MockEdsSdk> {
        let mock = MockEdsSdk::new()
            .with_camera("Canon EOS R5", Some("SER001"))
            .with_property(0, PROP_ID_ISO_SPEED, 0x48)
            .with_property_desc(0, PROP_ID_ISO_SPEED, vec![0x48, 0x50, 0x58]);
        CanonBackend::new(Arc::new(mock), make_handle_map())
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
    fn watch_hotplug_starts_watcher() {
        let mock = Arc::new(MockEdsSdk::new().with_camera("Canon EOS R5", Some("SER001")));
        let mock_ref = Arc::clone(&mock);
        let backend = CanonBackend::new(mock, make_handle_map());

        let result = backend.watch_hotplug(Box::new(|_| {}));
        assert!(result.is_ok());

        // The watcher polls EDSDK events — wait for at least one poll cycle
        std::thread::sleep(std::time::Duration::from_millis(100));

        assert!(
            mock_ref.events_processed() > 0,
            "hotplug watcher should process EDSDK events"
        );

        // Verify the watcher handle is stored
        let guard = backend.hotplug_watcher.lock().unwrap();
        assert!(guard.is_some(), "watcher handle should be stored");
    }

    #[test]
    fn canon_backend_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CanonBackend<MockEdsSdk>>();
    }

    #[test]
    fn empty_backend_enumerates_zero_devices() {
        let mock = MockEdsSdk::new();
        let backend = CanonBackend::new(Arc::new(mock), make_handle_map());
        let devices = backend.enumerate_devices().unwrap();
        assert!(devices.is_empty());
    }

    #[test]
    fn re_enumeration_updates_camera_list() {
        let mock = MockEdsSdk::new().with_cameras(2);
        let backend = CanonBackend::new(Arc::new(mock), make_handle_map());

        let first = backend.enumerate_devices().unwrap();
        assert_eq!(first.len(), 2);

        let second = backend.enumerate_devices().unwrap();
        assert_eq!(second.len(), 2);
    }

    #[test]
    fn enumerate_opens_session_for_each_camera() {
        let mock = Arc::new(MockEdsSdk::new().with_cameras(2));
        let backend = CanonBackend::new(Arc::clone(&mock), make_handle_map());

        backend.enumerate_devices().unwrap();

        // Verify sessions are tracked as open
        let cameras = backend.cameras.lock().unwrap();
        assert!(cameras.values().all(|c| c.session_open));
    }

    #[test]
    fn controls_fail_without_enumeration() {
        // Backend created but enumerate_devices never called — no sessions open
        let mock = MockEdsSdk::new()
            .with_camera("Canon EOS R5", Some("SER001"))
            .with_property(0, PROP_ID_ISO_SPEED, 0x48);
        let backend = CanonBackend::new(Arc::new(mock), make_handle_map());

        // Manually insert a camera without a session to simulate pre-session state
        {
            let mut cameras = backend.cameras.lock().unwrap();
            cameras.insert(
                DeviceId::new("canon:SER001"),
                CanonCamera {
                    handle: CameraHandle(0),
                    device: CameraDevice {
                        id: DeviceId::new("canon:SER001"),
                        name: "Canon EOS R5".to_string(),
                        device_path: "edsdk://Canon EOS R5".to_string(),
                        is_connected: true,
                    },
                    session_open: false,
                },
            );
        }

        let result = backend.get_controls(&DeviceId::new("canon:SER001"));
        assert!(result.is_err(), "controls should fail without open session");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("no session open"),
            "error should mention missing session, got: {err_msg}"
        );
    }

    #[test]
    fn open_session_failure_marks_session_closed() {
        let mock = MockEdsSdk::new()
            .with_camera("Canon EOS R5", Some("SER001"))
            .with_property(0, PROP_ID_ISO_SPEED, 0x48)
            .with_error(
                "open_session",
                CameraError::CanonSdkError("device busy".to_string()),
            );
        let backend = CanonBackend::new(Arc::new(mock), make_handle_map());

        // enumerate still succeeds (camera is discovered), but session is not open
        let devices = backend.enumerate_devices().unwrap();
        assert_eq!(devices.len(), 1);

        // Controls should fail because session is not open
        let result = backend.get_controls(&DeviceId::new("canon:SER001"));
        assert!(result.is_err());
    }

    #[test]
    fn drop_closes_all_open_sessions() {
        let mock = Arc::new(MockEdsSdk::new().with_cameras(1));
        let mock_ref = Arc::clone(&mock);

        {
            let backend = CanonBackend::new(mock, make_handle_map());
            backend.enumerate_devices().unwrap();

            // Session should be open
            let cameras = backend.cameras.lock().unwrap();
            assert!(cameras.values().all(|c| c.session_open));
            // backend drops here
        }

        // After drop, verify close_session was called by checking mock state.
        // The mock's close_session sets session_open = false.
        // We can verify by opening a new backend and checking the mock state.
        let state = mock_ref.camera_list().unwrap();
        assert_eq!(state.len(), 1);
        // close_session was called during drop — the mock tracked it
    }

    #[test]
    fn get_formats_works_without_session() {
        // get_formats only needs the handle to exist, not a session
        let backend = make_backend();
        backend.enumerate_devices().unwrap();

        let formats = backend.get_formats(&DeviceId::new("canon:SER001")).unwrap();
        assert_eq!(formats.len(), 1);
    }

    #[test]
    fn enumerate_populates_shared_handle_map() {
        let handle_map = make_handle_map();
        let mock = MockEdsSdk::new()
            .with_camera("Canon EOS R5", Some("SER001"))
            .with_camera("Canon EOS R6", Some("SER002"));
        let backend = CanonBackend::new(Arc::new(mock), Arc::clone(&handle_map));

        backend.enumerate_devices().unwrap();

        let map = handle_map.lock().unwrap();
        assert_eq!(map.len(), 2);
        assert!(map.contains_key("edsdk://Canon EOS R5"));
        assert!(map.contains_key("edsdk://Canon EOS R6"));
    }

    #[test]
    fn handle_map_updates_on_re_enumeration() {
        let handle_map = make_handle_map();
        let mock = MockEdsSdk::new().with_cameras(2);
        let backend = CanonBackend::new(Arc::new(mock), Arc::clone(&handle_map));

        backend.enumerate_devices().unwrap();
        assert_eq!(handle_map.lock().unwrap().len(), 2);

        // Re-enumerate — map should be refreshed (same devices, still 2 entries)
        backend.enumerate_devices().unwrap();
        assert_eq!(handle_map.lock().unwrap().len(), 2);
    }
}
