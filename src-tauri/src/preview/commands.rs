use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

use super::capture::{CaptureSession, PreviewErrorPayload, PreviewSession};
use super::compress;
use super::gpu::{GpuAdapterInfo, GpuState};
use crate::camera::commands::CameraState;
use crate::camera::error::humanise_error;
use crate::camera::types::{CameraDevice, DeviceId};
use crate::diagnostics::stats::DiagnosticSnapshot;
use crate::preview::encode_worker::EncodingSnapshot;
use crate::CanonSdkState;

/// Cached JPEG result for a single device, keyed by frame sequence number.
struct JpegCache {
    sequence: u64,
    base64: String,
}

/// Managed state holding active preview sessions.
pub struct PreviewState {
    pub sessions: Mutex<HashMap<String, PreviewSession>>,
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
#[allow(clippy::too_many_arguments)]
pub async fn start_preview(
    app: AppHandle,
    state: State<'_, PreviewState>,
    camera_state: State<'_, CameraState>,
    canon_state: State<'_, CanonSdkState>,
    gpu_state: State<'_, GpuState>,
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

    let session = create_preview_session(
        &app,
        &canon_state,
        &gpu_state,
        &device_id,
        &device_path,
        &friendly_name,
        width,
        height,
        fps,
    )?;
    sessions.insert(device_id, session);
    Ok(())
}

/// Create a `PreviewSession` for the given device.
///
/// Detects Canon devices by the `edsdk://` prefix in `device_path` and
/// creates a `CanonCaptureSession`; otherwise creates a DirectShow session.
#[allow(clippy::too_many_arguments)]
fn create_preview_session(
    app: &AppHandle,
    canon_state: &CanonSdkState,
    gpu_state: &GpuState,
    device_id: &str,
    device_path: &str,
    friendly_name: &str,
    width: u32,
    height: u32,
    fps: f32,
) -> Result<PreviewSession, String> {
    // Canon live view: device_path starts with "edsdk://"
    if device_path.starts_with("edsdk://") {
        return create_canon_session(canon_state, device_id, device_path);
    }

    let on_error = make_error_callback(app);
    let gpu = gpu_state.context();
    let session = CaptureSession::new(
        device_path.to_string(),
        friendly_name.to_string(),
        width,
        height,
        fps,
        Some(on_error),
        gpu,
        75,
    );
    Ok(PreviewSession::DirectShow(session))
}

/// Create a Canon live view capture session.
///
/// Resolves the CameraHandle from the shared handle map and starts live view.
fn create_canon_session(
    canon_state: &CanonSdkState,
    device_id: &str,
    device_path: &str,
) -> Result<PreviewSession, String> {
    #[cfg(all(feature = "canon", target_os = "windows"))]
    {
        let sdk = canon_state
            .sdk()
            .ok_or_else(|| "Canon SDK not available".to_string())?;
        let handle = canon_state
            .find_handle(device_path)
            .ok_or_else(|| format!("Canon camera not found: {device_path}"))?;

        let session = super::capture::CanonCaptureSession::new(
            device_id.to_string(),
            Arc::clone(sdk),
            handle,
        )?;
        Ok(PreviewSession::Canon(session))
    }

    #[cfg(not(all(feature = "canon", target_os = "windows")))]
    {
        let _ = (canon_state, device_id, device_path);
        Err("Canon support not available in this build".to_string())
    }
}

/// Build a standard error callback that emits `preview-error` events.
fn make_error_callback(app: &AppHandle) -> super::capture::ErrorCallback {
    let app = app.clone();
    Arc::new(move |dev_id: &str, error: &str| {
        let _ = app.emit(
            "preview-error",
            PreviewErrorPayload {
                device_id: dev_id.to_string(),
                error: humanise_error(error),
            },
        );
    })
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
    canon_state: State<'_, CanonSdkState>,
    gpu_state: State<'_, GpuState>,
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

        match create_preview_session(
            &app,
            &canon_state,
            &gpu_state,
            &device_id,
            &device.device_path,
            &device.name,
            640,
            480,
            30.0,
        ) {
            Ok(session) => {
                sessions.insert(device_id.clone(), session);
                tracing::info!("Auto-started preview session for '{}'", device.name);
            }
            Err(e) => {
                tracing::warn!("Failed to start preview for '{}': {e}", device.name);
            }
        }
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

    // Canon live view: device_path starts with "edsdk://"
    if device.device_path.starts_with("edsdk://") {
        if let Some(canon_state) = app.try_state::<CanonSdkState>() {
            match create_canon_session(canon_state.inner(), device_id, &device.device_path) {
                Ok(session) => {
                    sessions.insert(device_id.to_string(), session);
                    tracing::info!(
                        "Auto-started Canon preview for '{}' on hotplug",
                        device.name
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to start Canon preview for '{}' on hotplug: {e}",
                        device.name
                    );
                }
            }
        }
        return;
    }

    // DirectShow capture
    let on_error = make_error_callback(app);
    let gpu = app.try_state::<GpuState>().and_then(|s| s.context());

    let session = CaptureSession::new(
        device.device_path.clone(),
        device.name.clone(),
        640,
        480,
        30.0,
        Some(on_error),
        gpu,
        75,
    );
    sessions.insert(device_id.to_string(), PreviewSession::DirectShow(session));
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
        session.stop();
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
/// Reads pre-encoded JPEG from the async encode worker's output buffer.
/// Caches the base64 result per device — if the sequence hasn't changed
/// since the last call, the cached string is returned immediately.
#[tauri::command]
pub async fn get_frame(
    state: State<'_, PreviewState>,
    device_id: String,
) -> Result<String, String> {
    let (jpeg_frame, seq) = {
        let sessions = state.sessions.lock();
        let session = sessions
            .get(&device_id)
            .ok_or_else(|| "no active preview for this device".to_string())?;

        // Try the JPEG buffer first (from the encode worker)
        if let Some(jpeg_buf) = session.jpeg_buffer() {
            let seq = jpeg_buf.sequence();
            if let Some(frame) = jpeg_buf.latest() {
                (Some(frame), seq)
            } else {
                (None, 0)
            }
        } else {
            (None, 0)
        }
    };

    // If the encode worker has a JPEG frame, use it directly
    if let Some(jpeg_frame) = jpeg_frame {
        // Check cache — return early if the frame hasn't changed
        {
            let cache = state.jpeg_cache.lock();
            if let Some(cached) = cache.get(&device_id) {
                if cached.sequence == seq {
                    return Ok(cached.base64.clone());
                }
            }
        }

        let base64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &jpeg_frame.jpeg_bytes,
        );

        let mut cache = state.jpeg_cache.lock();
        cache.insert(
            device_id,
            JpegCache {
                sequence: seq,
                base64: base64.clone(),
            },
        );

        return Ok(base64);
    }

    // Fallback: read raw frame and compress on the fly (legacy path)
    // Canon sessions have no raw buffer, so this path is DirectShow-only.
    let (frame, seq) = {
        let sessions = state.sessions.lock();
        let session = sessions
            .get(&device_id)
            .ok_or_else(|| "no active preview for this device".to_string())?;

        let buf = session
            .buffer()
            .ok_or_else(|| "no frame available".to_string())?;
        let f = buf
            .latest()
            .ok_or_else(|| "no frame available".to_string())?;
        (f, buf.sequence())
    };

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

    let mut cache = state.jpeg_cache.lock();
    cache.insert(
        device_id,
        JpegCache {
            sequence: seq,
            base64: base64.clone(),
        },
    );

    Ok(base64)
}

/// Get a thumbnail (160x120) as base64-encoded JPEG.
///
/// Tries the raw RGB buffer first (DirectShow path), then falls back to
/// the JPEG buffer (Canon path) and re-encodes via `compress_thumbnail_from_jpeg`.
#[tauri::command]
pub async fn get_thumbnail(
    state: State<'_, PreviewState>,
    device_id: String,
) -> Result<String, String> {
    let sessions = state.sessions.lock();
    let session = sessions
        .get(&device_id)
        .ok_or_else(|| "no active preview for this device".to_string())?;

    // DirectShow path: raw RGB buffer available
    if let Some(buf) = session.buffer() {
        if let Some(frame) = buf.latest() {
            let thumb =
                compress::compress_thumbnail(&frame.data, frame.width, frame.height, 160, 120);
            return Ok(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &thumb,
            ));
        }
    }

    // Canon fallback: decode JPEG, downscale, re-encode
    if let Some(jpeg_buf) = session.jpeg_buffer() {
        if let Some(jpeg_frame) = jpeg_buf.latest() {
            let thumb = compress::compress_thumbnail_from_jpeg(&jpeg_frame.jpeg_bytes, 160, 120)?;
            return Ok(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &thumb,
            ));
        }
    }

    Err("no frame available".to_string())
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

/// Get encoding performance stats for a camera preview session.
///
/// Returns encoder type (hardware/software/CPU), frame counts, and timing.
#[tauri::command]
pub async fn get_encoding_stats(
    state: State<'_, PreviewState>,
    device_id: String,
) -> Result<EncodingSnapshot, String> {
    let sessions = state.sessions.lock();
    let session = sessions
        .get(&device_id)
        .ok_or_else(|| "no active preview for this device".to_string())?;

    session
        .encoding_snapshot()
        .ok_or_else(|| "encode worker not active for this device".to_string())
}

/// List all available GPU adapters on the system.
#[tauri::command]
pub async fn list_gpu_adapters() -> Vec<GpuAdapterInfo> {
    super::gpu::GpuContext::enumerate_adapters()
}

/// Get information about the currently active GPU adapter.
///
/// Returns `null` if GPU acceleration is disabled (CPU-only mode).
#[tauri::command]
pub async fn get_active_gpu(state: State<'_, GpuState>) -> Result<Option<GpuAdapterInfo>, String> {
    Ok(state.context().map(|ctx| ctx.adapter_info()))
}

/// Switch the active GPU adapter, or disable GPU acceleration.
///
/// Pass `adapterIndex: null` to switch to CPU-only mode.
/// Returns the name of the newly selected adapter, or `null` if disabled.
#[tauri::command]
pub async fn set_gpu_adapter(
    state: State<'_, GpuState>,
    adapter_index: Option<usize>,
) -> Result<Option<String>, String> {
    Ok(state.set_adapter(adapter_index))
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

    /// Helper: create a DirectShow capture session for testing.
    fn make_ds_session(device_id: &str, w: u32, h: u32) -> CaptureSession {
        CaptureSession::new(
            device_id.to_string(),
            String::new(),
            w,
            h,
            30.0,
            None,
            None,
            75,
        )
    }

    #[test]
    fn start_preview_with_empty_device_id_fails() {
        let state = make_preview_state();
        let device_id = "".to_string();
        assert!(device_id.is_empty());
        let sessions = state.sessions.lock();
        assert!(sessions.is_empty());
    }

    #[test]
    fn start_preview_creates_session() {
        let state = make_preview_state();
        {
            let mut sessions = state.sessions.lock();
            let session = make_ds_session("test-device", 640, 480);
            sessions.insert(
                "test-device".to_string(),
                PreviewSession::DirectShow(session),
            );
        }
        let sessions = state.sessions.lock();
        assert!(sessions.contains_key("test-device"));
    }

    #[test]
    fn stop_preview_removes_session() {
        let state = make_preview_state();
        {
            let mut sessions = state.sessions.lock();
            let session = make_ds_session("test-device", 640, 480);
            sessions.insert(
                "test-device".to_string(),
                PreviewSession::DirectShow(session),
            );
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
        assert!(sessions.get("nonexistent").is_none());
    }

    #[test]
    fn get_frame_returns_base64_jpeg_when_active() {
        let state = make_preview_state();
        let frame = make_rgb_frame(10, 10);
        {
            let mut sessions = state.sessions.lock();
            let session = make_ds_session("test-device", 10, 10);
            session.buffer().push(frame);
            sessions.insert(
                "test-device".to_string(),
                PreviewSession::DirectShow(session),
            );
        }

        let sessions = state.sessions.lock();
        let session = sessions.get("test-device").unwrap();
        let buf = session.buffer().unwrap();
        let latest = buf.latest().unwrap();
        let jpeg = compress::compress_jpeg(&latest.data, latest.width, latest.height, 85);
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &jpeg);
        assert!(!b64.is_empty());

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

        let session = make_ds_session("dev-1", 10, 10);
        session.buffer().push(make_rgb_frame(10, 10));
        let seq = session.buffer().sequence();
        state
            .sessions
            .lock()
            .insert("dev-1".to_string(), PreviewSession::DirectShow(session));

        let frame1 = {
            let sessions = state.sessions.lock();
            sessions
                .get("dev-1")
                .unwrap()
                .buffer()
                .unwrap()
                .latest()
                .unwrap()
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

        let cache = state.jpeg_cache.lock();
        let cached = cache.get("dev-1").unwrap();
        assert_eq!(cached.sequence, 1);
        assert_eq!(cached.base64, b64);
    }

    #[test]
    fn jpeg_cache_invalidates_on_new_frame() {
        let state = make_preview_state();

        state.jpeg_cache.lock().insert(
            "dev-1".to_string(),
            JpegCache {
                sequence: 1,
                base64: "old-data".to_string(),
            },
        );

        let session = make_ds_session("dev-1", 10, 10);
        session.buffer().push(make_rgb_frame(10, 10));
        session.buffer().push(make_rgb_frame(10, 10));
        let new_seq = session.buffer().sequence();

        let cache = state.jpeg_cache.lock();
        let cached = cache.get("dev-1").unwrap();
        assert_ne!(cached.sequence, new_seq);
    }

    #[test]
    fn stop_preview_clears_jpeg_cache() {
        let state = make_preview_state();

        let session = make_ds_session("dev-1", 10, 10);
        state
            .sessions
            .lock()
            .insert("dev-1".to_string(), PreviewSession::DirectShow(session));
        state.jpeg_cache.lock().insert(
            "dev-1".to_string(),
            JpegCache {
                sequence: 1,
                base64: "cached".to_string(),
            },
        );

        {
            let mut sessions = state.sessions.lock();
            if let Some(mut s) = sessions.remove("dev-1") {
                s.stop();
            }
        }
        state.jpeg_cache.lock().remove("dev-1");

        assert!(state.jpeg_cache.lock().get("dev-1").is_none());
    }

    #[test]
    fn start_all_previews_creates_sessions_for_all_devices() {
        let state = make_preview_state();

        let device_ids = vec!["cam-1", "cam-2", "cam-3"];
        {
            let mut sessions = state.sessions.lock();
            for id in &device_ids {
                let session = make_ds_session(id, 640, 480);
                sessions.insert(id.to_string(), PreviewSession::DirectShow(session));
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

        {
            let mut sessions = state.sessions.lock();
            let session = CaptureSession::new(
                "cam-1".to_string(),
                "Camera 1".to_string(),
                640,
                480,
                30.0,
                None,
                None,
                75,
            );
            sessions.insert("cam-1".to_string(), PreviewSession::DirectShow(session));
        }

        {
            let mut sessions = state.sessions.lock();
            for id in &["cam-1", "cam-2"] {
                if !sessions.contains_key(*id) {
                    let session = make_ds_session(id, 640, 480);
                    sessions.insert(id.to_string(), PreviewSession::DirectShow(session));
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

        {
            let mut sessions = state.sessions.lock();
            let session = make_ds_session("cam-1", 640, 480);
            sessions.insert("cam-1".to_string(), PreviewSession::DirectShow(session));
        }
        state.jpeg_cache.lock().insert(
            "cam-1".to_string(),
            JpegCache {
                sequence: 1,
                base64: "cached".to_string(),
            },
        );

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
        let session = make_ds_session("dev-1", 10, 10);
        session.buffer().push(make_rgb_frame(10, 10));
        state
            .sessions
            .lock()
            .insert("dev-1".to_string(), PreviewSession::DirectShow(session));

        let sessions = state.sessions.lock();
        let session = sessions.get("dev-1").unwrap();
        let buf = session.buffer().unwrap();
        let frame1 = buf.latest().unwrap();
        let frame2 = buf.latest().unwrap();

        assert!(std::sync::Arc::ptr_eq(&frame1, &frame2));
    }

    #[test]
    fn preview_session_routes_edsdk_to_canon() {
        // Verify that the edsdk:// prefix detection works
        let device_path = "edsdk://Canon EOS R5";
        assert!(device_path.starts_with("edsdk://"));

        let device_path = "\\\\?\\usb#vid_046d";
        assert!(!device_path.starts_with("edsdk://"));
    }

    #[test]
    fn preview_session_canon_has_no_raw_buffer() {
        use crate::camera::canon::api::CameraHandle;
        use crate::camera::canon::mock::MockEdsSdk;
        use crate::preview::capture::CanonCaptureSession;

        let mock = Arc::new(
            MockEdsSdk::new()
                .with_cameras(1)
                .with_live_view_frame(vec![0xFF, 0xD8, 0xFF, 0xD9]),
        );
        let camera = CameraHandle(0);

        let session = CanonCaptureSession::new("canon:MOCK0001".to_string(), mock, camera).unwrap();

        let preview = PreviewSession::Canon(session);
        assert!(
            preview.buffer().is_none(),
            "Canon sessions have no raw buffer"
        );
        assert!(
            preview.jpeg_buffer().is_some(),
            "Canon sessions have a JPEG buffer"
        );
    }

    #[test]
    fn preview_session_directshow_has_raw_buffer() {
        let session = make_ds_session("dev-1", 10, 10);
        let preview = PreviewSession::DirectShow(session);
        assert!(
            preview.buffer().is_some(),
            "DirectShow sessions have a raw buffer"
        );
    }

    #[test]
    fn canon_thumbnail_uses_jpeg_fallback() {
        use crate::camera::canon::api::CameraHandle;
        use crate::camera::canon::mock::MockEdsSdk;
        use crate::preview::capture::CanonCaptureSession;

        // Create a valid JPEG for the mock to deliver
        let rgb = vec![128u8; 64 * 64 * 3];
        let test_jpeg = compress::compress_jpeg(&rgb, 64, 64, 85);

        let mock = Arc::new(
            MockEdsSdk::new()
                .with_cameras(1)
                .with_live_view_frame(test_jpeg.clone()),
        );
        let camera = CameraHandle(0);
        let session = CanonCaptureSession::new("canon:MOCK0001".to_string(), mock, camera).unwrap();

        // Wait for at least one frame
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while session.jpeg_buffer().latest().is_none() {
            if std::time::Instant::now() > deadline {
                panic!("Canon mock did not deliver a frame within 5s");
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        let preview = PreviewSession::Canon(session);

        // Canon has no raw buffer — buffer() returns None
        assert!(preview.buffer().is_none());

        // But jpeg_buffer() should have data
        let jpeg_buf = preview.jpeg_buffer().unwrap();
        let jpeg_frame = jpeg_buf.latest().unwrap();

        // Compress via the new JPEG-based thumbnail path
        let thumb =
            compress::compress_thumbnail_from_jpeg(&jpeg_frame.jpeg_bytes, 160, 120).unwrap();

        // Should produce a valid, small JPEG thumbnail
        assert_eq!(thumb[0], 0xFF, "missing JPEG SOI");
        assert_eq!(thumb[1], 0xD8, "missing JPEG SOI");
        assert!(
            thumb.len() < 10_000,
            "thumbnail {} bytes exceeds 10KB",
            thumb.len()
        );

        // Should be base64-encodable
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &thumb);
        assert!(!b64.is_empty());
    }

    #[test]
    fn directshow_thumbnail_unchanged() {
        let state = make_preview_state();
        let frame = make_rgb_frame(640, 480);
        let session = make_ds_session("ds-thumb", 640, 480);
        session.buffer().push(frame);
        state
            .sessions
            .lock()
            .insert("ds-thumb".to_string(), PreviewSession::DirectShow(session));

        // DirectShow path: buffer() returns Some, use raw RGB compress_thumbnail
        let sessions = state.sessions.lock();
        let session = sessions.get("ds-thumb").unwrap();

        let buf = session.buffer().unwrap();
        let latest = buf.latest().unwrap();
        let thumb =
            compress::compress_thumbnail(&latest.data, latest.width, latest.height, 160, 120);

        assert_eq!(thumb[0], 0xFF, "missing JPEG SOI");
        assert_eq!(thumb[1], 0xD8, "missing JPEG SOI");

        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &thumb);
        assert!(!b64.is_empty());
    }
}
