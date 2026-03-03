//! Canon live view polling thread.
//!
//! Spawns a thread that polls `download_evf_image()` at configurable
//! intervals and pushes JPEG frames directly into a `JpegFrameBuffer`.
//! Canon live view delivers JPEG natively, so no encoding step is needed.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::camera::canon::api::{CameraHandle, EdsSdkApi};
use crate::preview::encode_worker::{JpegFrame, JpegFrameBuffer};
use crate::preview::mf_jpeg::encoder::EncoderKind;

/// Default polling interval for live view frames (~5fps).
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Active live view session for a Canon camera.
pub struct LiveViewSession {
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    jpeg_buffer: Arc<JpegFrameBuffer>,
}

impl LiveViewSession {
    /// Start a live view session.
    ///
    /// Opens a session with the camera, enables EVF output, then spawns a
    /// polling thread that downloads JPEG frames into the provided buffer.
    pub fn start<S: EdsSdkApi + 'static>(
        sdk: Arc<S>,
        camera: CameraHandle,
        jpeg_buffer: Arc<JpegFrameBuffer>,
    ) -> crate::camera::error::Result<Self> {
        Self::start_with_interval(sdk, camera, jpeg_buffer, DEFAULT_POLL_INTERVAL)
    }

    /// Start with a custom polling interval (useful for testing).
    ///
    /// The caller must ensure that a session is already open for the camera
    /// (e.g. via `CanonBackend::enumerate_devices`). This method only starts
    /// live view output and the polling thread.
    pub fn start_with_interval<S: EdsSdkApi + 'static>(
        sdk: Arc<S>,
        camera: CameraHandle,
        jpeg_buffer: Arc<JpegFrameBuffer>,
        interval: Duration,
    ) -> crate::camera::error::Result<Self> {
        sdk.start_live_view(camera)?;

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let buffer_clone = Arc::clone(&jpeg_buffer);

        let thread = std::thread::Builder::new()
            .name(format!("canon-lv-{}", camera.0))
            .spawn(move || {
                poll_live_view(&*sdk, camera, &buffer_clone, &running_clone, interval);
            })
            .map_err(|e| {
                crate::camera::error::CameraError::CanonSdkError(format!(
                    "failed to spawn live view thread: {e}"
                ))
            })?;

        Ok(Self {
            running,
            thread: Some(thread),
            jpeg_buffer,
        })
    }

    /// Get a reference to the JPEG frame buffer.
    pub fn jpeg_buffer(&self) -> &Arc<JpegFrameBuffer> {
        &self.jpeg_buffer
    }

    /// Check if the session is still running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Stop the live view session.
    ///
    /// Stops the polling thread and disables EVF output. Does not close the
    /// camera session — that is managed by `CanonBackend`.
    pub fn stop<S: EdsSdkApi>(self, sdk: &S, camera: CameraHandle) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread {
            let _ = handle.join();
        }
        let _ = sdk.stop_live_view(camera);
    }
}

/// RAII guard for COM on the live view thread.
#[cfg(target_os = "windows")]
struct LiveViewComGuard {
    owns_init: bool,
}

#[cfg(target_os = "windows")]
impl LiveViewComGuard {
    fn init() -> Self {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if hr.is_err() {
            tracing::debug!("Live view thread: COM already initialised (hr={hr:?}), continuing");
            Self { owns_init: false }
        } else {
            tracing::debug!("Live view thread: COM STA initialised");
            Self { owns_init: true }
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for LiveViewComGuard {
    fn drop(&mut self) {
        if self.owns_init {
            unsafe {
                windows::Win32::System::Com::CoUninitialize();
            }
        }
    }
}

/// Polling loop that runs on the live view thread.
///
/// Initialises COM STA on this thread (EDSDK requires COM on every calling
/// thread) before entering the download loop.
fn poll_live_view<S: EdsSdkApi>(
    sdk: &S,
    camera: CameraHandle,
    jpeg_buffer: &JpegFrameBuffer,
    running: &AtomicBool,
    interval: Duration,
) {
    // EDSDK requires COM STA on every thread that calls it.
    #[cfg(target_os = "windows")]
    let _com = LiveViewComGuard::init();

    while running.load(Ordering::Relaxed) {
        match sdk.download_evf_image(camera) {
            Ok(jpeg_data) => {
                // Canon live view delivers JPEG natively — push directly
                // into the JPEG buffer, bypassing RGB encoding entirely.
                jpeg_buffer.update(JpegFrame {
                    jpeg_bytes: jpeg_data,
                    width: 960,  // Canon live view typical resolution
                    height: 640, // Canon live view typical resolution
                    encoder_kind: EncoderKind::CpuFallback, // Not really encoded, just a label
                });
            }
            Err(e) => {
                let msg = e.to_string();
                // These errors are expected during live view startup:
                // - OBJECT_NOTREADY: camera hasn't produced first frame yet
                // - EVF_NOT_ACTIVATED: EVF is still warming up after enable
                if !msg.contains("not ready") && !msg.contains("not activated") {
                    tracing::debug!("Canon live view frame error: {e}");
                }
            }
        }
        std::thread::sleep(interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::canon::mock::MockEdsSdk;

    /// Minimal valid JPEG for testing.
    fn test_jpeg() -> Vec<u8> {
        vec![0xFF, 0xD8, 0xFF, 0xD9]
    }

    #[test]
    fn live_view_session_pushes_frames_to_jpeg_buffer() {
        let mock = Arc::new(
            MockEdsSdk::new()
                .with_cameras(1)
                .with_live_view_frame(test_jpeg()),
        );
        let jpeg_buffer = Arc::new(JpegFrameBuffer::new());
        let camera = CameraHandle(0);

        let session = LiveViewSession::start_with_interval(
            Arc::clone(&mock),
            camera,
            Arc::clone(&jpeg_buffer),
            Duration::from_millis(10),
        )
        .unwrap();

        // Wait for a few frames
        std::thread::sleep(Duration::from_millis(50));

        assert!(session.is_running());
        assert!(jpeg_buffer.sequence() > 0, "frames should have been pushed");

        let frame = jpeg_buffer.latest().unwrap();
        assert_eq!(frame.jpeg_bytes, test_jpeg());

        session.stop(&*mock, camera);
    }

    #[test]
    fn live_view_session_stops_cleanly() {
        let mock = Arc::new(
            MockEdsSdk::new()
                .with_cameras(1)
                .with_live_view_frame(test_jpeg()),
        );
        let jpeg_buffer = Arc::new(JpegFrameBuffer::new());
        let camera = CameraHandle(0);

        let session = LiveViewSession::start_with_interval(
            Arc::clone(&mock),
            camera,
            Arc::clone(&jpeg_buffer),
            Duration::from_millis(10),
        )
        .unwrap();

        session.stop(&*mock, camera);
        // Should not panic or hang
    }

    #[test]
    fn live_view_handles_object_not_ready() {
        // No live view frame configured — download_evf_image will return error
        // but the session should not crash
        let mock = Arc::new(MockEdsSdk::new().with_cameras(1));
        let jpeg_buffer = Arc::new(JpegFrameBuffer::new());
        let camera = CameraHandle(0);

        // Manually start live view on mock so download attempts proceed
        mock.start_live_view(camera).unwrap();

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let buffer_clone = Arc::clone(&jpeg_buffer);

        let handle = std::thread::spawn(move || {
            poll_live_view(
                &*mock,
                camera,
                &buffer_clone,
                &running_clone,
                Duration::from_millis(5),
            );
        });

        std::thread::sleep(Duration::from_millis(30));
        running.store(false, Ordering::Relaxed);
        handle.join().unwrap();

        // No frames should have been pushed (no frame data configured)
        assert_eq!(jpeg_buffer.sequence(), 0);
    }

    #[test]
    fn live_view_starts_and_stops_without_managing_session() {
        let mock = Arc::new(
            MockEdsSdk::new()
                .with_cameras(1)
                .with_live_view_frame(test_jpeg()),
        );
        let jpeg_buffer = Arc::new(JpegFrameBuffer::new());
        let camera = CameraHandle(0);

        let session = LiveViewSession::start_with_interval(
            Arc::clone(&mock),
            camera,
            Arc::clone(&jpeg_buffer),
            Duration::from_millis(10),
        )
        .unwrap();

        // Session should be running (live view started, not camera session)
        assert!(session.is_running());

        // Stop disables live view but does not close the camera session
        session.stop(&*mock, camera);
    }
}
