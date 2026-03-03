use tauri::State;

use super::VirtualCameraState;
use crate::preview::commands::PreviewState;

/// Start a virtual camera output for the given device.
///
/// Requires an active preview session — the virtual camera reads JPEG frames
/// from the session's buffer. Returns an error if no preview exists or if the
/// platform sink fails to start.
#[tauri::command]
pub async fn start_virtual_camera(
    device_id: String,
    preview_state: State<'_, PreviewState>,
    vcam_state: State<'_, VirtualCameraState>,
) -> Result<(), String> {
    // Verify an active preview session exists for this device
    {
        let sessions = preview_state.sessions.lock();
        if !sessions.contains_key(&device_id) {
            return Err(format!("no active preview for device: {device_id}"));
        }
    }

    let sink = super::create_sink();
    vcam_state.start(device_id, sink)
}

/// Stop the virtual camera output for the given device. Idempotent.
#[tauri::command]
pub async fn stop_virtual_camera(
    device_id: String,
    vcam_state: State<'_, VirtualCameraState>,
) -> Result<(), String> {
    vcam_state.stop(&device_id)
}

/// Check whether a virtual camera is active for the given device.
#[tauri::command]
pub async fn get_virtual_camera_status(
    device_id: String,
    vcam_state: State<'_, VirtualCameraState>,
) -> Result<bool, String> {
    Ok(vcam_state.is_active(&device_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::capture::{CaptureSession, PreviewSession};
    use crate::virtual_camera::VirtualCameraSink;

    /// Mock sink that tracks start/stop calls.
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

    fn make_preview_state() -> PreviewState {
        PreviewState::new()
    }

    fn make_vcam_state() -> VirtualCameraState {
        VirtualCameraState::new()
    }

    #[test]
    fn start_without_preview_returns_error() {
        let preview_state = make_preview_state();
        let vcam_state = make_vcam_state();

        // No preview session exists — should fail
        let sessions = preview_state.sessions.lock();
        let has_session = sessions.contains_key("nonexistent");
        assert!(!has_session);
        drop(sessions);

        // Simulate command logic: check session, then try start
        let result = {
            let sessions = preview_state.sessions.lock();
            if !sessions.contains_key("nonexistent") {
                Err(format!("no active preview for device: nonexistent"))
            } else {
                let sink = super::super::create_sink();
                vcam_state.start("nonexistent".to_string(), sink)
            }
        };

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("no active preview"),
            "expected 'no active preview' error, got: {err}"
        );
    }

    #[test]
    fn stop_is_idempotent() {
        let vcam_state = make_vcam_state();
        assert!(vcam_state.stop("nonexistent").is_ok());
        assert!(vcam_state.stop("nonexistent").is_ok());
    }

    #[test]
    fn status_reflects_active_state() {
        let preview_state = make_preview_state();
        let vcam_state = make_vcam_state();

        // Initially inactive
        assert!(!vcam_state.is_active("cam-1"));

        // Add a preview session so the check would pass
        {
            let mut sessions = preview_state.sessions.lock();
            let session = CaptureSession::new(
                "cam-1".to_string(),
                String::new(),
                640,
                480,
                30.0,
                None,
                None,
                75,
            );
            sessions.insert("cam-1".to_string(), PreviewSession::DirectShow(session));
        }

        // Start a mock sink directly
        vcam_state
            .start("cam-1".to_string(), Box::new(MockSink::new()))
            .unwrap();
        assert!(vcam_state.is_active("cam-1"));

        // Stop it
        vcam_state.stop("cam-1").unwrap();
        assert!(!vcam_state.is_active("cam-1"));
    }
}
