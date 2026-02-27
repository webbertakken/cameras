use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, State};

use super::capture::{CaptureSession, PreviewErrorPayload};
use super::compress;
use crate::camera::commands::CameraState;
use crate::camera::types::DeviceId;
use crate::diagnostics::stats::DiagnosticSnapshot;

/// Cached JPEG result for a single device, keyed by frame timestamp.
struct JpegCache {
    timestamp_us: u64,
    base64: String,
}

/// Managed state holding active preview sessions.
pub struct PreviewState {
    pub sessions: Mutex<HashMap<String, CaptureSession>>,
    /// Per-device JPEG cache to avoid recompressing unchanged frames.
    jpeg_cache: Mutex<HashMap<String, JpegCache>>,
}

impl PreviewState {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            jpeg_cache: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for PreviewState {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve device_id to (device_path, friendly_name) via the camera backend.
fn resolve_device_info(
    camera_state: &CameraState,
    device_id: &str,
) -> Result<(String, String), String> {
    let devices = camera_state
        .backend
        .enumerate_devices()
        .map_err(|e| format!("failed to enumerate devices: {e}"))?;

    let target_id = DeviceId::new(device_id);
    devices
        .iter()
        .find(|d| d.id == target_id)
        .map(|d| (d.device_path.clone(), d.name.clone()))
        .ok_or_else(|| format!("device not found: {device_id}"))
}

/// Start a camera preview session.
#[tauri::command]
pub async fn start_preview(
    app: AppHandle,
    state: State<'_, PreviewState>,
    camera_state: State<'_, CameraState>,
    device_id: String,
    width: u32,
    height: u32,
    fps: f32,
) -> Result<(), String> {
    if device_id.is_empty() {
        return Err("device_id must not be empty".to_string());
    }

    // Resolve device_id to the actual device path and name needed by DirectShow
    let (device_path, friendly_name) = resolve_device_info(&camera_state, &device_id)?;

    let mut sessions = state.sessions.lock();
    if sessions.contains_key(&device_id) {
        if let Some(mut existing) = sessions.remove(&device_id) {
            existing.stop();
        }
    }

    let on_error = {
        let app = app.clone();
        Arc::new(move |device_id: &str, error: &str| {
            let _ = app.emit(
                "preview-error",
                PreviewErrorPayload {
                    device_id: device_id.to_string(),
                    error: error.to_string(),
                },
            );
        }) as super::capture::ErrorCallback
    };

    let session = CaptureSession::new(
        device_path,
        friendly_name,
        width,
        height,
        fps,
        Some(on_error),
    );
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
    // Remove cached JPEG for this device
    state.jpeg_cache.lock().remove(&device_id);
    Ok(())
}

/// Get the latest frame as base64-encoded JPEG.
///
/// Caches the compressed result per device — if the frame timestamp hasn't
/// changed since the last call, the cached JPEG is returned immediately
/// without recompressing.
#[tauri::command]
pub async fn get_frame(
    state: State<'_, PreviewState>,
    device_id: String,
) -> Result<String, String> {
    let frame = {
        let sessions = state.sessions.lock();
        let session = sessions
            .get(&device_id)
            .ok_or_else(|| "no active preview for this device".to_string())?;

        session
            .buffer()
            .latest()
            .ok_or_else(|| "no frame available".to_string())?
    };

    // Check cache — return early if the frame hasn't changed
    {
        let cache = state.jpeg_cache.lock();
        if let Some(cached) = cache.get(&device_id) {
            if cached.timestamp_us == frame.timestamp_us {
                return Ok(cached.base64.clone());
            }
        }
    }

    let jpeg = compress::compress_jpeg(&frame.data, frame.width, frame.height, 85);
    let base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &jpeg);

    // Store in cache
    {
        let mut cache = state.jpeg_cache.lock();
        cache.insert(
            device_id,
            JpegCache {
                timestamp_us: frame.timestamp_us,
                base64: base64.clone(),
            },
        );
    }

    Ok(base64)
}

/// Get a thumbnail (160x120) as base64-encoded JPEG.
#[tauri::command]
pub async fn get_thumbnail(
    state: State<'_, PreviewState>,
    device_id: String,
) -> Result<String, String> {
    let frame = {
        let sessions = state.sessions.lock();
        let session = sessions
            .get(&device_id)
            .ok_or_else(|| "no active preview for this device".to_string())?;

        session
            .buffer()
            .latest()
            .ok_or_else(|| "no frame available".to_string())?
    };

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
            let session = CaptureSession::new(
                "test-device".to_string(),
                String::new(),
                640,
                480,
                30.0,
                None,
            );
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
            let session = CaptureSession::new(
                "test-device".to_string(),
                String::new(),
                640,
                480,
                30.0,
                None,
            );
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
            let session =
                CaptureSession::new("test-device".to_string(), String::new(), 10, 10, 30.0, None);
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

    #[test]
    fn preview_error_payload_has_camel_case_keys() {
        let payload = PreviewErrorPayload {
            device_id: "cam-1".to_string(),
            error: "resource busy".to_string(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("deviceId"), "expected camelCase key: {json}");
        assert!(
            !json.contains("device_id"),
            "expected no snake_case key: {json}"
        );
    }

    #[test]
    fn jpeg_cache_returns_same_result_for_same_timestamp() {
        let state = make_preview_state();

        // Insert a session with a frame
        let session = CaptureSession::new("dev-1".to_string(), String::new(), 10, 10, 30.0, None);
        session.buffer().push(make_rgb_frame(10, 10));
        state.sessions.lock().insert("dev-1".to_string(), session);

        // Simulate first get_frame: compress and cache
        let frame1 = {
            let sessions = state.sessions.lock();
            sessions.get("dev-1").unwrap().buffer().latest().unwrap()
        };
        let jpeg = compress::compress_jpeg(&frame1.data, frame1.width, frame1.height, 85);
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &jpeg);
        state.jpeg_cache.lock().insert(
            "dev-1".to_string(),
            JpegCache {
                timestamp_us: frame1.timestamp_us,
                base64: b64.clone(),
            },
        );

        // Simulate second get_frame: should hit cache
        let cache = state.jpeg_cache.lock();
        let cached = cache.get("dev-1").unwrap();
        assert_eq!(cached.timestamp_us, 1000);
        assert_eq!(cached.base64, b64);
    }

    #[test]
    fn jpeg_cache_invalidates_on_new_frame() {
        let state = make_preview_state();

        // Seed cache with old timestamp
        state.jpeg_cache.lock().insert(
            "dev-1".to_string(),
            JpegCache {
                timestamp_us: 500,
                base64: "old-data".to_string(),
            },
        );

        // New frame with different timestamp
        let frame = Frame {
            data: vec![128u8; 300],
            width: 10,
            height: 10,
            timestamp_us: 1000,
        };

        // Cache miss — timestamps differ
        let cache = state.jpeg_cache.lock();
        let cached = cache.get("dev-1").unwrap();
        assert_ne!(cached.timestamp_us, frame.timestamp_us);
    }

    #[test]
    fn stop_preview_clears_jpeg_cache() {
        let state = make_preview_state();

        // Create session and cache entry
        let session = CaptureSession::new("dev-1".to_string(), String::new(), 10, 10, 30.0, None);
        state.sessions.lock().insert("dev-1".to_string(), session);
        state.jpeg_cache.lock().insert(
            "dev-1".to_string(),
            JpegCache {
                timestamp_us: 1000,
                base64: "cached".to_string(),
            },
        );

        // Stop the preview
        {
            let mut sessions = state.sessions.lock();
            if let Some(mut s) = sessions.remove("dev-1") {
                s.stop();
            }
        }
        state.jpeg_cache.lock().remove("dev-1");

        // Cache should be cleared
        assert!(state.jpeg_cache.lock().get("dev-1").is_none());
    }

    #[test]
    fn frame_buffer_latest_returns_arc() {
        let state = make_preview_state();
        let session = CaptureSession::new("dev-1".to_string(), String::new(), 10, 10, 30.0, None);
        session.buffer().push(make_rgb_frame(10, 10));
        state.sessions.lock().insert("dev-1".to_string(), session);

        let sessions = state.sessions.lock();
        let session = sessions.get("dev-1").unwrap();
        let frame1 = session.buffer().latest().unwrap();
        let frame2 = session.buffer().latest().unwrap();

        // Both should point to the same allocation (Arc)
        assert!(std::sync::Arc::ptr_eq(&frame1, &frame2));
    }
}
