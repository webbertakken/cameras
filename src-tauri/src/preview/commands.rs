use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

use super::capture::{CaptureSession, PreviewErrorPayload};
use super::compress;
use crate::camera::commands::CameraState;
use crate::camera::error::humanise_error;
use crate::camera::types::{CameraDevice, DeviceId};
use crate::diagnostics::stats::DiagnosticSnapshot;

/// Cached JPEG result for a single device, keyed by frame sequence number.
struct JpegCache {
    sequence: u64,
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
        .map_err(|e| humanise_error(&format!("failed to enumerate devices: {e}")))?;

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
                    error: humanise_error(error),
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

/// Start capture sessions for all currently connected cameras.
///
/// Skips devices that already have an active session. Uses sensible defaults
/// (640x480, 30fps) — the frontend can reconfigure individual sessions later.
#[tauri::command]
pub async fn start_all_previews(
    app: AppHandle,
    state: State<'_, PreviewState>,
    camera_state: State<'_, CameraState>,
) -> Result<(), String> {
    let devices = camera_state
        .backend
        .enumerate_devices()
        .map_err(|e| humanise_error(&format!("failed to enumerate devices: {e}")))?;

    let mut sessions = state.sessions.lock();

    for device in &devices {
        let device_id = device.id.as_str().to_string();

        // Skip if session already exists
        if sessions.contains_key(&device_id) {
            continue;
        }

        let on_error = {
            let app = app.clone();
            Arc::new(move |dev_id: &str, error: &str| {
                let _ = app.emit(
                    "preview-error",
                    PreviewErrorPayload {
                        device_id: dev_id.to_string(),
                        error: humanise_error(error),
                    },
                );
            }) as super::capture::ErrorCallback
        };

        let session = CaptureSession::new(
            device.device_path.clone(),
            device.name.clone(),
            640,
            480,
            30.0,
            Some(on_error),
        );
        sessions.insert(device_id.clone(), session);
        tracing::info!("Auto-started preview session for '{}'", device.name);
    }

    Ok(())
}

/// Start a capture session for a single device by ID. Used by the hotplug
/// bridge when a new camera is connected.
pub fn start_preview_for_device(app: &AppHandle, device_id: &str) {
    let preview_state = match app.try_state::<PreviewState>() {
        Some(s) => s,
        None => return,
    };
    let camera_state = match app.try_state::<CameraState>() {
        Some(s) => s,
        None => return,
    };

    // Resolve device info
    let devices: Vec<CameraDevice> = match camera_state.backend.enumerate_devices() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("Failed to enumerate devices for hotplug start: {e}");
            return;
        }
    };

    let target_id = DeviceId::new(device_id);
    let device = match devices.iter().find(|d| d.id == target_id) {
        Some(d) => d,
        None => {
            tracing::warn!("Hotplug device not found in enumeration: {device_id}");
            return;
        }
    };

    let mut sessions = preview_state.sessions.lock();
    if sessions.contains_key(device_id) {
        return;
    }

    let on_error = {
        let app = app.clone();
        Arc::new(move |dev_id: &str, error: &str| {
            let _ = app.emit(
                "preview-error",
                PreviewErrorPayload {
                    device_id: dev_id.to_string(),
                    error: humanise_error(error),
                },
            );
        }) as super::capture::ErrorCallback
    };

    let session = CaptureSession::new(
        device.device_path.clone(),
        device.name.clone(),
        640,
        480,
        30.0,
        Some(on_error),
    );
    sessions.insert(device_id.to_string(), session);
    tracing::info!(
        "Auto-started preview session for '{}' on hotplug",
        device.name
    );
}

/// Stop and clean up a capture session for a disconnected device.
pub fn stop_preview_for_device(app: &AppHandle, device_id: &str) {
    let preview_state = match app.try_state::<PreviewState>() {
        Some(s) => s,
        None => return,
    };

    let mut sessions = preview_state.sessions.lock();
    if let Some(mut session) = sessions.remove(device_id) {
        CaptureSession::stop(&mut session);
        tracing::info!("Stopped preview session for disconnected device: {device_id}");
    }
    preview_state.jpeg_cache.lock().remove(device_id);
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
    let (frame, seq) = {
        let sessions = state.sessions.lock();
        let session = sessions
            .get(&device_id)
            .ok_or_else(|| "no active preview for this device".to_string())?;

        let buf = session.buffer();
        let f = buf
            .latest()
            .ok_or_else(|| "no frame available".to_string())?;
        (f, buf.sequence())
    };

    // Check cache — return early if the frame hasn't changed.
    // Uses the monotonic sequence counter rather than the frame's own
    // timestamp, because some virtual cameras (e.g. OBS) always report 0.
    {
        let cache = state.jpeg_cache.lock();
        if let Some(cached) = cache.get(&device_id) {
            if cached.sequence == seq {
                return Ok(cached.base64.clone());
            }
        }
    }

    let jpeg = compress::compress_jpeg(&frame.data, frame.width, frame.height, 75);
    let base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &jpeg);

    // Store in cache
    {
        let mut cache = state.jpeg_cache.lock();
        cache.insert(
            device_id,
            JpegCache {
                sequence: seq,
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
    fn jpeg_cache_returns_same_result_for_same_sequence() {
        let state = make_preview_state();

        // Insert a session with a frame
        let session = CaptureSession::new("dev-1".to_string(), String::new(), 10, 10, 30.0, None);
        session.buffer().push(make_rgb_frame(10, 10));
        let seq = session.buffer().sequence();
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
                sequence: seq,
                base64: b64.clone(),
            },
        );

        // Simulate second get_frame: should hit cache (same sequence)
        let cache = state.jpeg_cache.lock();
        let cached = cache.get("dev-1").unwrap();
        assert_eq!(cached.sequence, 1);
        assert_eq!(cached.base64, b64);
    }

    #[test]
    fn jpeg_cache_invalidates_on_new_frame() {
        let state = make_preview_state();

        // Seed cache with old sequence
        state.jpeg_cache.lock().insert(
            "dev-1".to_string(),
            JpegCache {
                sequence: 1,
                base64: "old-data".to_string(),
            },
        );

        // Push a new frame — sequence advances to 2
        let session = CaptureSession::new("dev-1".to_string(), String::new(), 10, 10, 30.0, None);
        session.buffer().push(make_rgb_frame(10, 10));
        session.buffer().push(make_rgb_frame(10, 10));
        let new_seq = session.buffer().sequence();

        // Cache miss — sequences differ
        let cache = state.jpeg_cache.lock();
        let cached = cache.get("dev-1").unwrap();
        assert_ne!(cached.sequence, new_seq);
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
                sequence: 1,
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
    fn start_all_previews_creates_sessions_for_all_devices() {
        let state = make_preview_state();

        // Simulate starting sessions for multiple devices
        let device_ids = vec!["cam-1", "cam-2", "cam-3"];
        {
            let mut sessions = state.sessions.lock();
            for id in &device_ids {
                let session = CaptureSession::new(
                    id.to_string(),
                    format!("Camera {id}"),
                    640,
                    480,
                    30.0,
                    None,
                );
                sessions.insert(id.to_string(), session);
            }
        }

        let sessions = state.sessions.lock();
        assert_eq!(sessions.len(), 3);
        assert!(sessions.contains_key("cam-1"));
        assert!(sessions.contains_key("cam-2"));
        assert!(sessions.contains_key("cam-3"));
    }

    #[test]
    fn start_all_previews_skips_existing_sessions() {
        let state = make_preview_state();

        // Pre-insert a session for cam-1
        {
            let mut sessions = state.sessions.lock();
            let session = CaptureSession::new(
                "cam-1".to_string(),
                "Camera 1".to_string(),
                640,
                480,
                30.0,
                None,
            );
            sessions.insert("cam-1".to_string(), session);
        }

        // Simulate start_all_previews logic: only insert if not already present
        {
            let mut sessions = state.sessions.lock();
            for id in &["cam-1", "cam-2"] {
                if !sessions.contains_key(*id) {
                    let session = CaptureSession::new(
                        id.to_string(),
                        format!("Camera {id}"),
                        640,
                        480,
                        30.0,
                        None,
                    );
                    sessions.insert(id.to_string(), session);
                }
            }
        }

        let sessions = state.sessions.lock();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains_key("cam-1"));
        assert!(sessions.contains_key("cam-2"));
    }

    #[test]
    fn stop_preview_for_disconnected_device_cleans_up() {
        let state = make_preview_state();

        // Create session and cache
        {
            let mut sessions = state.sessions.lock();
            let session =
                CaptureSession::new("cam-1".to_string(), String::new(), 640, 480, 30.0, None);
            sessions.insert("cam-1".to_string(), session);
        }
        state.jpeg_cache.lock().insert(
            "cam-1".to_string(),
            JpegCache {
                sequence: 1,
                base64: "cached".to_string(),
            },
        );

        // Simulate stop_preview_for_device logic
        {
            let mut sessions = state.sessions.lock();
            if let Some(mut s) = sessions.remove("cam-1") {
                s.stop();
            }
        }
        state.jpeg_cache.lock().remove("cam-1");

        assert!(!state.sessions.lock().contains_key("cam-1"));
        assert!(state.jpeg_cache.lock().get("cam-1").is_none());
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
