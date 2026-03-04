pub mod commands;
pub mod nv12;
pub mod stub;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "windows")]
pub mod windows;

use std::collections::HashMap;

use parking_lot::Mutex;

/// Trait for virtual camera output sinks.
///
/// Implementations push frames to a virtual camera device (e.g. OBS Virtual
/// Camera, v4l2loopback) so that other applications can consume the feed.
pub trait VirtualCameraSink: Send + Sync {
    /// Start the virtual camera output.
    fn start(&mut self) -> Result<(), String>;
    /// Stop the virtual camera output. Idempotent.
    fn stop(&mut self) -> Result<(), String>;
    /// Whether the sink is currently outputting frames.
    fn is_running(&self) -> bool;
}

/// Managed state holding active virtual camera sinks, keyed by device ID.
pub struct VirtualCameraState {
    sinks: Mutex<HashMap<String, Box<dyn VirtualCameraSink>>>,
}

impl VirtualCameraState {
    pub fn new() -> Self {
        Self {
            sinks: Mutex::new(HashMap::new()),
        }
    }

    /// Start a virtual camera sink for the given device.
    ///
    /// Replaces any existing sink for the same device (stopping it first).
    pub fn start(
        &self,
        device_id: String,
        mut sink: Box<dyn VirtualCameraSink>,
    ) -> Result<(), String> {
        let mut sinks = self.sinks.lock();
        if let Some(mut existing) = sinks.remove(&device_id) {
            let _ = existing.stop();
        }
        sink.start()?;
        sinks.insert(device_id, sink);
        Ok(())
    }

    /// Stop and remove the virtual camera sink for the given device.
    ///
    /// Idempotent — returns `Ok(())` if no sink exists for the device.
    pub fn stop(&self, device_id: &str) -> Result<(), String> {
        let mut sinks = self.sinks.lock();
        if let Some(mut sink) = sinks.remove(device_id) {
            sink.stop()?;
        }
        Ok(())
    }

    /// Check whether a virtual camera sink is active for the given device.
    pub fn is_active(&self, device_id: &str) -> bool {
        let sinks = self.sinks.lock();
        sinks
            .get(device_id)
            .map(|s| s.is_running())
            .unwrap_or(false)
    }
}

impl Default for VirtualCameraState {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a platform-appropriate virtual camera sink.
pub fn create_sink() -> Box<dyn VirtualCameraSink> {
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::DirectShowVirtualCamera)
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::V4l2LoopbackSink)
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Box::new(stub::StubSink)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock sink for testing VirtualCameraState without a real device.
    struct MockSink {
        running: bool,
    }

    impl MockSink {
        fn new() -> Self {
            Self { running: false }
        }
    }

    impl VirtualCameraSink for MockSink {
        fn start(&mut self) -> Result<(), String> {
            self.running = true;
            Ok(())
        }

        fn stop(&mut self) -> Result<(), String> {
            self.running = false;
            Ok(())
        }

        fn is_running(&self) -> bool {
            self.running
        }
    }

    #[test]
    fn virtual_camera_state_starts_empty() {
        let state = VirtualCameraState::new();
        assert!(!state.is_active("any-device"));
    }

    #[test]
    fn virtual_camera_state_tracks_active_devices() {
        let state = VirtualCameraState::new();

        // Start a mock sink
        state
            .start("cam-1".to_string(), Box::new(MockSink::new()))
            .unwrap();
        assert!(state.is_active("cam-1"));
        assert!(!state.is_active("cam-2"));

        // Stop it
        state.stop("cam-1").unwrap();
        assert!(!state.is_active("cam-1"));
    }

    #[test]
    fn trait_object_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<VirtualCameraState>();
    }

    #[test]
    fn stop_is_idempotent() {
        let state = VirtualCameraState::new();
        // Stopping a non-existent device should be fine
        assert!(state.stop("nonexistent").is_ok());
        assert!(state.stop("nonexistent").is_ok());
    }

    #[test]
    fn create_sink_returns_platform_appropriate() {
        let mut sink = create_sink();

        // All current platform sinks are stubs that return errors
        let err = sink.start().unwrap_err();
        assert!(
            err.contains("not yet"),
            "expected stub error message, got: {err}"
        );
        assert!(!sink.is_running());

        // On Windows specifically, verify it's the DirectShow variant
        #[cfg(target_os = "windows")]
        {
            assert!(
                err.contains("DirectShow"),
                "expected DirectShow sink on Windows, got: {err}"
            );
        }
    }
}
