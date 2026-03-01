//! Canon live view polling thread.
//!
//! Spawns a thread that polls `download_evf_image()` at configurable
//! intervals and pushes JPEG frames into a `FrameBuffer`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::camera::canon::api::{CameraHandle, EdsSdkApi};
use crate::preview::capture::{Frame, FrameBuffer};

/// Default polling interval for live view frames (~5fps).
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Active live view session for a Canon camera.
pub struct LiveViewSession {
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    buffer: Arc<FrameBuffer>,
}

impl LiveViewSession {
    /// Start a live view session.
    ///
    /// Sends the EVF start command to the camera, then spawns a polling
    /// thread that downloads JPEG frames into the provided buffer.
    pub fn start<S: EdsSdkApi + 'static>(
        sdk: Arc<S>,
        camera: CameraHandle,
        buffer: Arc<FrameBuffer>,
    ) -> crate::camera::error::Result<Self> {
        Self::start_with_interval(sdk, camera, buffer, DEFAULT_POLL_INTERVAL)
    }

    /// Start with a custom polling interval (useful for testing).
    pub fn start_with_interval<S: EdsSdkApi + 'static>(
        sdk: Arc<S>,
        camera: CameraHandle,
        buffer: Arc<FrameBuffer>,
        interval: Duration,
    ) -> crate::camera::error::Result<Self> {
        sdk.start_live_view(camera)?;

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let buffer_clone = Arc::clone(&buffer);

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
            buffer,
        })
    }

    /// Get a reference to the frame buffer.
    pub fn buffer(&self) -> &Arc<FrameBuffer> {
        &self.buffer
    }

    /// Check if the session is still running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Stop the live view session.
    pub fn stop<S: EdsSdkApi>(self, sdk: &S, camera: CameraHandle) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread {
            let _ = handle.join();
        }
        let _ = sdk.stop_live_view(camera);
    }
}

/// Polling loop that runs on the live view thread.
fn poll_live_view<S: EdsSdkApi>(
    sdk: &S,
    camera: CameraHandle,
    buffer: &FrameBuffer,
    running: &AtomicBool,
    interval: Duration,
) {
    let mut seq: u64 = 0;
    while running.load(Ordering::Relaxed) {
        match sdk.download_evf_image(camera) {
            Ok(jpeg_data) => {
                seq += 1;
                // Canon live view delivers JPEG natively — we store the
                // raw bytes as "pixel data" since the frontend already
                // expects base64-encoded JPEG from the preview pipeline.
                buffer.push(Frame {
                    data: jpeg_data,
                    width: 0,  // Unknown for JPEG passthrough
                    height: 0, // Unknown for JPEG passthrough
                    timestamp_us: seq * interval.as_micros() as u64,
                });
            }
            Err(e) => {
                let msg = e.to_string();
                // OBJECT_NOTREADY is expected during live view startup
                if !msg.contains("not ready") && !msg.contains("not active") {
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
    fn live_view_session_pushes_frames_to_buffer() {
        let mock = Arc::new(
            MockEdsSdk::new()
                .with_cameras(1)
                .with_live_view_frame(test_jpeg()),
        );
        let buffer = Arc::new(FrameBuffer::new(3));
        let camera = CameraHandle(0);

        let session = LiveViewSession::start_with_interval(
            Arc::clone(&mock),
            camera,
            Arc::clone(&buffer),
            Duration::from_millis(10),
        )
        .unwrap();

        // Wait for a few frames
        std::thread::sleep(Duration::from_millis(50));

        assert!(session.is_running());
        assert!(buffer.sequence() > 0, "frames should have been pushed");

        let frame = buffer.latest().unwrap();
        assert_eq!(frame.data, test_jpeg());

        session.stop(&*mock, camera);
    }

    #[test]
    fn live_view_session_stops_cleanly() {
        let mock = Arc::new(
            MockEdsSdk::new()
                .with_cameras(1)
                .with_live_view_frame(test_jpeg()),
        );
        let buffer = Arc::new(FrameBuffer::new(3));
        let camera = CameraHandle(0);

        let session = LiveViewSession::start_with_interval(
            Arc::clone(&mock),
            camera,
            Arc::clone(&buffer),
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
        let buffer = Arc::new(FrameBuffer::new(3));
        let camera = CameraHandle(0);

        // Manually start live view on mock so download attempts proceed
        mock.start_live_view(camera).unwrap();

        let session = LiveViewSession {
            running: Arc::new(AtomicBool::new(true)),
            thread: None,
            buffer: Arc::clone(&buffer),
        };

        // Simulate a few poll cycles
        let running = Arc::clone(&session.running);
        let mock_clone = Arc::clone(&mock);
        let buffer_clone = Arc::clone(&buffer);

        let handle = std::thread::spawn(move || {
            poll_live_view(
                &*mock_clone,
                camera,
                &buffer_clone,
                &running,
                Duration::from_millis(5),
            );
        });

        std::thread::sleep(Duration::from_millis(30));
        session.running.store(false, Ordering::Relaxed);
        handle.join().unwrap();

        // No frames should have been pushed (no frame data configured)
        assert_eq!(buffer.sequence(), 0);
    }
}
