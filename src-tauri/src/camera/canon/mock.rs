//! Mock EDSDK implementation for testing without real Canon DLLs.
//!
//! Uses a builder pattern to configure cameras, properties, live view
//! frames, and error injection.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::camera::error::{CameraError, Result};

use super::api::{CameraHandle, EdsSdkApi};
use super::types::{EdsDeviceInfo, EdsPropertyDesc, EdsPropertyID};

/// A simulated Canon camera in the mock.
#[derive(Debug, Clone)]
struct MockCamera {
    model: String,
    serial: Option<String>,
    properties: HashMap<EdsPropertyID, i32>,
    property_descs: HashMap<EdsPropertyID, Vec<i32>>,
    session_open: bool,
}

/// Configurable error injection for a specific operation.
#[derive(Debug, Clone)]
struct ErrorInjection {
    operation: &'static str,
    error: CameraError,
}

/// Mock EDSDK implementation.
///
/// All state is behind a `Mutex` so the mock satisfies `Send + Sync`.
pub struct MockEdsSdk {
    state: Mutex<MockState>,
}

#[derive(Debug)]
struct MockState {
    cameras: Vec<MockCamera>,
    live_view_frame: Option<Vec<u8>>,
    live_view_active: HashMap<usize, bool>,
    error_injections: Vec<ErrorInjection>,
    events_processed: u32,
}

impl MockEdsSdk {
    /// Create a new empty mock (no cameras).
    pub fn new() -> Self {
        Self {
            state: Mutex::new(MockState {
                cameras: Vec::new(),
                live_view_frame: None,
                live_view_active: HashMap::new(),
                error_injections: Vec::new(),
                events_processed: 0,
            }),
        }
    }

    /// Add a camera with a specific model name and serial number.
    pub fn with_camera(self, model: &str, serial: Option<&str>) -> Self {
        let mut state = self.state.lock().unwrap();
        state.cameras.push(MockCamera {
            model: model.to_string(),
            serial: serial.map(|s| s.to_string()),
            properties: HashMap::new(),
            property_descs: HashMap::new(),
            session_open: false,
        });
        drop(state);
        self
    }

    /// Add N cameras with auto-generated names and serials.
    pub fn with_cameras(self, count: usize) -> Self {
        let mut result = self;
        for i in 0..count {
            result = result.with_camera(
                &format!("Canon EOS Mock {}", i + 1),
                Some(&format!("MOCK{:04}", i + 1)),
            );
        }
        result
    }

    /// Set the live view frame JPEG data returned by `download_evf_image`.
    pub fn with_live_view_frame(self, jpeg_bytes: Vec<u8>) -> Self {
        let mut state = self.state.lock().unwrap();
        state.live_view_frame = Some(jpeg_bytes);
        drop(state);
        self
    }

    /// Set a property value on a specific camera (by index).
    pub fn with_property(self, camera_idx: usize, prop: EdsPropertyID, value: i32) -> Self {
        let mut state = self.state.lock().unwrap();
        if let Some(cam) = state.cameras.get_mut(camera_idx) {
            cam.properties.insert(prop, value);
        }
        drop(state);
        self
    }

    /// Set the allowed values for a property on a specific camera.
    pub fn with_property_desc(
        self,
        camera_idx: usize,
        prop: EdsPropertyID,
        values: Vec<i32>,
    ) -> Self {
        let mut state = self.state.lock().unwrap();
        if let Some(cam) = state.cameras.get_mut(camera_idx) {
            cam.property_descs.insert(prop, values);
        }
        drop(state);
        self
    }

    /// Inject an error for a specific operation name.
    ///
    /// Operation names: `"camera_list"`, `"open_session"`, `"close_session"`,
    /// `"get_device_info"`, `"start_live_view"`, `"stop_live_view"`,
    /// `"download_evf_image"`, `"get_property"`, `"set_property"`,
    /// `"get_property_desc"`, `"get_event"`.
    pub fn with_error(self, operation: &'static str, error: CameraError) -> Self {
        let mut state = self.state.lock().unwrap();
        state
            .error_injections
            .push(ErrorInjection { operation, error });
        drop(state);
        self
    }

    /// Return the number of `get_event` calls processed.
    pub fn events_processed(&self) -> u32 {
        self.state.lock().unwrap().events_processed
    }
}

impl MockState {
    /// Check for injected errors for the given operation.
    fn check_error(&mut self, operation: &str) -> Result<()> {
        if let Some(pos) = self
            .error_injections
            .iter()
            .position(|e| e.operation == operation)
        {
            let injection = self.error_injections.remove(pos);
            return Err(injection.error);
        }
        Ok(())
    }

    fn get_camera(&self, handle: CameraHandle) -> Result<&MockCamera> {
        self.cameras
            .get(handle.0)
            .ok_or_else(|| CameraError::DeviceNotFound(format!("mock camera {}", handle.0)))
    }

    fn get_camera_mut(&mut self, handle: CameraHandle) -> Result<&mut MockCamera> {
        self.cameras
            .get_mut(handle.0)
            .ok_or_else(|| CameraError::DeviceNotFound(format!("mock camera {}", handle.0)))
    }
}

impl EdsSdkApi for MockEdsSdk {
    fn camera_list(&self) -> Result<Vec<CameraHandle>> {
        let mut state = self.state.lock().unwrap();
        state.check_error("camera_list")?;
        Ok((0..state.cameras.len()).map(CameraHandle).collect())
    }

    fn open_session(&self, camera: CameraHandle) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.check_error("open_session")?;
        let cam = state.get_camera_mut(camera)?;
        cam.session_open = true;
        Ok(())
    }

    fn close_session(&self, camera: CameraHandle) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.check_error("close_session")?;
        let cam = state.get_camera_mut(camera)?;
        cam.session_open = false;
        Ok(())
    }

    fn get_device_info(&self, camera: CameraHandle) -> Result<EdsDeviceInfo> {
        let mut state = self.state.lock().unwrap();
        state.check_error("get_device_info")?;
        let cam = state.get_camera(camera)?.clone();

        let mut info = EdsDeviceInfo {
            device_description: [0u8; 256],
            body_id_ex: [0u8; 256],
            reserved1: 0,
            reserved2: 0,
        };

        let model_bytes = cam.model.as_bytes();
        let len = model_bytes.len().min(255);
        info.device_description[..len].copy_from_slice(&model_bytes[..len]);

        if let Some(ref serial) = cam.serial {
            let serial_bytes = serial.as_bytes();
            let slen = serial_bytes.len().min(255);
            info.body_id_ex[..slen].copy_from_slice(&serial_bytes[..slen]);
        }

        Ok(info)
    }

    fn start_live_view(&self, camera: CameraHandle) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.check_error("start_live_view")?;
        // Verify camera exists
        let _ = state.get_camera(camera)?;
        state.live_view_active.insert(camera.0, true);
        Ok(())
    }

    fn stop_live_view(&self, camera: CameraHandle) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.check_error("stop_live_view")?;
        state.live_view_active.insert(camera.0, false);
        Ok(())
    }

    fn download_evf_image(&self, camera: CameraHandle) -> Result<Vec<u8>> {
        let mut state = self.state.lock().unwrap();
        state.check_error("download_evf_image")?;

        // Verify camera exists
        let _ = state.get_camera(camera)?;

        let is_active = state
            .live_view_active
            .get(&camera.0)
            .copied()
            .unwrap_or(false);
        if !is_active {
            return Err(CameraError::CanonSdkError(
                "live view not active".to_string(),
            ));
        }

        state
            .live_view_frame
            .clone()
            .ok_or_else(|| CameraError::CanonSdkError("no live view frame configured".to_string()))
    }

    fn get_property(&self, camera: CameraHandle, prop: EdsPropertyID) -> Result<i32> {
        let mut state = self.state.lock().unwrap();
        state.check_error("get_property")?;
        let cam = state.get_camera(camera)?.clone();
        cam.properties
            .get(&prop)
            .copied()
            .ok_or_else(|| CameraError::ControlQuery(format!("property 0x{prop:04X} not set")))
    }

    fn set_property(&self, camera: CameraHandle, prop: EdsPropertyID, value: i32) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.check_error("set_property")?;
        let cam = state.get_camera_mut(camera)?;
        cam.properties.insert(prop, value);
        Ok(())
    }

    fn get_property_desc(
        &self,
        camera: CameraHandle,
        prop: EdsPropertyID,
    ) -> Result<EdsPropertyDesc> {
        let mut state = self.state.lock().unwrap();
        state.check_error("get_property_desc")?;
        let cam = state.get_camera(camera)?.clone();
        let values = cam.property_descs.get(&prop).cloned().unwrap_or_default();
        Ok(EdsPropertyDesc {
            num_elements: values.len(),
            prop_desc: values,
        })
    }

    fn get_event(&self) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.check_error("get_event")?;
        state.events_processed += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::canon::types::PROP_ID_ISO_SPEED;

    #[test]
    fn empty_mock_returns_no_cameras() {
        let mock = MockEdsSdk::new();
        let cameras = mock.camera_list().unwrap();
        assert!(cameras.is_empty());
    }

    #[test]
    fn with_cameras_returns_correct_count() {
        let mock = MockEdsSdk::new().with_cameras(3);
        let cameras = mock.camera_list().unwrap();
        assert_eq!(cameras.len(), 3);
    }

    #[test]
    fn with_camera_returns_device_info() {
        let mock = MockEdsSdk::new().with_camera("Canon EOS R5", Some("ABC123"));
        let cameras = mock.camera_list().unwrap();
        let info = mock.get_device_info(cameras[0]).unwrap();
        assert_eq!(info.model_name(), "Canon EOS R5");
        assert_eq!(info.serial_number(), Some("ABC123".to_string()));
    }

    #[test]
    fn camera_without_serial_returns_none() {
        let mock = MockEdsSdk::new().with_camera("Canon EOS R6", None);
        let cameras = mock.camera_list().unwrap();
        let info = mock.get_device_info(cameras[0]).unwrap();
        assert_eq!(info.model_name(), "Canon EOS R6");
        assert_eq!(info.serial_number(), None);
    }

    #[test]
    fn session_open_and_close() {
        let mock = MockEdsSdk::new().with_cameras(1);
        let handle = CameraHandle(0);
        mock.open_session(handle).unwrap();
        mock.close_session(handle).unwrap();
    }

    #[test]
    fn property_read_write() {
        let mock = MockEdsSdk::new()
            .with_cameras(1)
            .with_property(0, PROP_ID_ISO_SPEED, 0x48);
        let handle = CameraHandle(0);
        let value = mock.get_property(handle, PROP_ID_ISO_SPEED).unwrap();
        assert_eq!(value, 0x48);

        mock.set_property(handle, PROP_ID_ISO_SPEED, 0x50).unwrap();
        let updated = mock.get_property(handle, PROP_ID_ISO_SPEED).unwrap();
        assert_eq!(updated, 0x50);
    }

    #[test]
    fn property_desc_returns_configured_values() {
        let mock = MockEdsSdk::new().with_cameras(1).with_property_desc(
            0,
            PROP_ID_ISO_SPEED,
            vec![0x48, 0x50, 0x58],
        );
        let handle = CameraHandle(0);
        let desc = mock.get_property_desc(handle, PROP_ID_ISO_SPEED).unwrap();
        assert_eq!(desc.num_elements, 3);
        assert_eq!(desc.prop_desc, vec![0x48, 0x50, 0x58]);
    }

    #[test]
    fn live_view_requires_start() {
        let mock = MockEdsSdk::new()
            .with_cameras(1)
            .with_live_view_frame(vec![0xFF, 0xD8, 0xFF, 0xD9]);
        let handle = CameraHandle(0);

        // Should fail before starting live view
        assert!(mock.download_evf_image(handle).is_err());

        mock.start_live_view(handle).unwrap();
        let frame = mock.download_evf_image(handle).unwrap();
        assert_eq!(frame, vec![0xFF, 0xD8, 0xFF, 0xD9]);

        mock.stop_live_view(handle).unwrap();
        assert!(mock.download_evf_image(handle).is_err());
    }

    #[test]
    fn error_injection_fires_once() {
        let mock = MockEdsSdk::new().with_cameras(1).with_error(
            "camera_list",
            CameraError::CanonSdkError("injected".to_string()),
        );

        // First call should fail
        assert!(mock.camera_list().is_err());

        // Second call should succeed (error consumed)
        assert!(mock.camera_list().is_ok());
    }

    #[test]
    fn get_event_increments_counter() {
        let mock = MockEdsSdk::new();
        assert_eq!(mock.events_processed(), 0);
        mock.get_event().unwrap();
        mock.get_event().unwrap();
        assert_eq!(mock.events_processed(), 2);
    }

    #[test]
    fn invalid_camera_handle_returns_error() {
        let mock = MockEdsSdk::new();
        assert!(mock.get_device_info(CameraHandle(99)).is_err());
    }

    #[test]
    fn unset_property_returns_error() {
        let mock = MockEdsSdk::new().with_cameras(1);
        let result = mock.get_property(CameraHandle(0), 0xFFFF);
        assert!(result.is_err());
    }

    #[test]
    fn mock_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockEdsSdk>();
    }
}
