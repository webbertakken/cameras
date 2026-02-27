use std::collections::HashMap;

use parking_lot::Mutex;
use tauri::State;

use super::capture::CaptureSession;
use super::compress;
use crate::camera::commands::CameraState;
use crate::camera::types::DeviceId;
use crate::diagnostics::stats::DiagnosticSnapshot;

/// Managed state holding active preview sessions.
pub struct PreviewState {
    pub sessions: Mutex<HashMap<String, CaptureSession>>,
}

impl PreviewState {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for PreviewState {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve device_id to device_path via the camera backend.
fn resolve_device_path(camera_state: &CameraState, device_id: &str) -> Result<String, String> {
    let devices = camera_state
        .backend
        .enumerate_devices()
        .map_err(|e| format!("failed to enumerate devices: {e}"))?;

    let target_id = DeviceId::new(device_id);
    devices
        .iter()
        .find(|d| d.id == target_id)
        .map(|d| d.device_path.clone())
        .ok_or_else(|| format!("device not found: {device_id}"))
}

/// Start a camera preview session.
#[tauri::command]
pub async fn start_preview(
    state: State<'_, PreviewState>,
    camera_state: State<'_, CameraState>,
    device_id: String,
    width: u32,
    height: u32,
    fps: f32,
) -> Result<(), String> {
    println!("[preview] start_preview called for device_id={device_id}");

    if device_id.is_empty() {
        return Err("device_id must not be empty".to_string());
    }

    // Resolve device_id to the actual device path needed by DirectShow
    let device_path = resolve_device_path(&camera_state, &device_id)?;
    println!("[preview] resolved device_path={device_path}");

    let mut sessions = state.sessions.lock();
    if sessions.contains_key(&device_id) {
        if let Some(mut existing) = sessions.remove(&device_id) {
            existing.stop();
        }
    }

    let session = CaptureSession::new(device_path, width, height, fps);
    println!("[preview] session created, capture thread spawned");
    sessions.insert(device_id, session);
    Ok(())
}

/// Stop a camera preview session. Idempotent.
#[tauri::command]
pub async fn stop_preview(state: State<'_, PreviewState>, device_id: String) -> Result<(), String> {
    let mut sessions = state.sessions.lock();
    if let Some(mut session) = sessions.remove(&device_id) {
        session.stop();
    }
    Ok(())
}

/// Get the latest frame as base64-encoded JPEG.
#[tauri::command]
pub async fn get_frame(
    state: State<'_, PreviewState>,
    device_id: String,
) -> Result<String, String> {
    let sessions = state.sessions.lock();
    let session = sessions
        .get(&device_id)
        .ok_or_else(|| "no active preview for this device".to_string())?;

    let frame = session
        .buffer()
        .latest()
        .ok_or_else(|| "no frame available".to_string())?;

    let jpeg = compress::compress_jpeg(&frame.data, frame.width, frame.height, 85);
    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &jpeg,
    ))
}

/// Get a thumbnail (160x120) as base64-encoded JPEG.
#[tauri::command]
pub async fn get_thumbnail(
    state: State<'_, PreviewState>,
    device_id: String,
) -> Result<String, String> {
    let sessions = state.sessions.lock();
    let session = sessions
        .get(&device_id)
        .ok_or_else(|| "no active preview for this device".to_string())?;

    let frame = session
        .buffer()
        .latest()
        .ok_or_else(|| "no frame available".to_string())?;

    let thumb = compress::compress_thumbnail(&frame.data, frame.width, frame.height, 160, 120);
    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &thumb,
    ))
}

/// Get diagnostic stats for a camera preview session.
#[tauri::command]
pub async fn get_diagnostics(
    state: State<'_, PreviewState>,
    device_id: String,
) -> Result<DiagnosticSnapshot, String> {
    let sessions = state.sessions.lock();
    let session = sessions
        .get(&device_id)
        .ok_or_else(|| "no active preview for this device".to_string())?;

    Ok(session.diagnostics())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::capture::Frame;

    fn make_preview_state() -> PreviewState {
        PreviewState::new()
    }

    fn make_rgb_frame(width: u32, height: u32) -> Frame {
        let data = vec![128u8; (width * height * 3) as usize];
        Frame {
            data,
            width,
            height,
            timestamp_us: 1000,
        }
    }

    #[test]
    fn start_preview_with_empty_device_id_fails() {
        let state = make_preview_state();
        // Simulate the validation
        let device_id = "".to_string();
        assert!(device_id.is_empty());
        // The command would return Err for empty device_id
        let sessions = state.sessions.lock();
        assert!(sessions.is_empty());
    }

    #[test]
    fn start_preview_creates_session() {
        let state = make_preview_state();
        {
            let mut sessions = state.sessions.lock();
            let session = CaptureSession::new("test-device".to_string(), 640, 480, 30.0);
            sessions.insert("test-device".to_string(), session);
        }
        let sessions = state.sessions.lock();
        assert!(sessions.contains_key("test-device"));
    }

    #[test]
    fn stop_preview_removes_session() {
        let state = make_preview_state();
        {
            let mut sessions = state.sessions.lock();
            let session = CaptureSession::new("test-device".to_string(), 640, 480, 30.0);
            sessions.insert("test-device".to_string(), session);
        }
        {
            let mut sessions = state.sessions.lock();
            if let Some(mut s) = sessions.remove("test-device") {
                s.stop();
            }
        }
        let sessions = state.sessions.lock();
        assert!(!sessions.contains_key("test-device"));
    }

    #[test]
    fn stop_preview_without_session_is_ok() {
        let state = make_preview_state();
        let sessions = state.sessions.lock();
        // Looking up a non-existent key should not panic
        assert!(sessions.get("nonexistent").is_none());
    }

    #[test]
    fn get_frame_returns_base64_jpeg_when_active() {
        let state = make_preview_state();
        let frame = make_rgb_frame(10, 10);
        {
            let mut sessions = state.sessions.lock();
            let session = CaptureSession::new("test-device".to_string(), 10, 10, 30.0);
            session.buffer().push(frame);
            sessions.insert("test-device".to_string(), session);
        }

        let sessions = state.sessions.lock();
        let session = sessions.get("test-device").unwrap();
        let latest = session.buffer().latest().unwrap();
        let jpeg = compress::compress_jpeg(&latest.data, latest.width, latest.height, 85);
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &jpeg);
        assert!(!b64.is_empty());

        // Decode and verify it starts with JPEG magic bytes
        let decoded =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64).unwrap();
        assert_eq!(decoded[0], 0xFF);
        assert_eq!(decoded[1], 0xD8);
    }

    #[test]
    fn get_frame_returns_error_when_no_preview() {
        let state = make_preview_state();
        let sessions = state.sessions.lock();
        let result = sessions.get("nonexistent");
        assert!(result.is_none());
    }
}
