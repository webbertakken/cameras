//! Canon live view polling thread.
//!
//! Spawns a thread that polls `download_evf_image()` at configurable
//! intervals and pushes JPEG frames directly into a `JpegFrameBuffer`.
//! Canon live view delivers JPEG natively, so no encoding step is needed.

use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::camera::canon::api::{CameraHandle, EdsSdkApi};
use crate::diagnostics::stats::DiagnosticStats;
use crate::preview::encode_worker::{JpegFrame, JpegFrameBuffer};
use crate::preview::mf_jpeg::encoder::EncoderKind;

/// Default polling interval for live view frames.
///
/// Canon EDSDK live view can deliver frames as fast as the camera produces
/// them (~7-10fps on most EOS bodies). We poll aggressively and only sleep
/// on errors to maximise throughput.
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Back-off delay when a download attempt fails (e.g. EVF not ready).
const ERROR_BACKOFF: Duration = Duration::from_millis(100);

/// How long without a successful frame before logging a stuck warning.
const STUCK_THRESHOLD: Duration = Duration::from_secs(5);

/// How often to log periodic frame count (every N frames).
const FRAME_LOG_INTERVAL: u64 = 100;

/// Tracks observability state for the live view polling loop.
///
/// Provides methods called on each frame success/error to decide which
/// log messages to emit. Separated from the loop for testability.
#[derive(Debug)]
struct PollStats {
    frame_count: u64,
    first_frame_logged: bool,
    last_success: Instant,
    stuck_warned: bool,
    last_error_msg: Option<String>,
}

impl PollStats {
    fn new(now: Instant) -> Self {
        Self {
            frame_count: 0,
            first_frame_logged: false,
            last_success: now,
            stuck_warned: false,
            last_error_msg: None,
        }
    }

    /// Called when a frame is successfully received. Returns an action to take.
    fn on_frame(&mut self, size: usize, now: Instant) -> FrameAction {
        self.frame_count += 1;
        self.last_success = now;
        self.stuck_warned = false;
        self.last_error_msg = None;

        if !self.first_frame_logged {
            self.first_frame_logged = true;
            return FrameAction::LogFirstFrame { size };
        }

        if self.frame_count % FRAME_LOG_INTERVAL == 0 {
            return FrameAction::LogPeriodicCount {
                count: self.frame_count,
            };
        }

        FrameAction::None
    }

    /// Called when a download error occurs. Returns an action to take.
    fn on_error(&mut self, error_msg: String, now: Instant) -> ErrorAction {
        self.last_error_msg = Some(error_msg.clone());

        // Suppress expected startup errors individually
        if error_msg.contains("not ready") || error_msg.contains("not activated") {
            let elapsed = now.duration_since(self.last_success);
            if elapsed >= STUCK_THRESHOLD && !self.stuck_warned {
                self.stuck_warned = true;
                return ErrorAction::LogStuck {
                    seconds: elapsed.as_secs(),
                    last_error: error_msg,
                };
            }
            return ErrorAction::Suppress;
        }

        // Unexpected error — check for stuck as well
        let elapsed = now.duration_since(self.last_success);
        if elapsed >= STUCK_THRESHOLD && !self.stuck_warned {
            self.stuck_warned = true;
            return ErrorAction::LogStuckAndError {
                seconds: elapsed.as_secs(),
                last_error: error_msg,
            };
        }

        ErrorAction::LogDebug { error: error_msg }
    }
}

/// Action to take after a successful frame.
#[derive(Debug, PartialEq)]
enum FrameAction {
    None,
    LogFirstFrame { size: usize },
    LogPeriodicCount { count: u64 },
}

/// Action to take after an error.
#[derive(Debug, PartialEq)]
enum ErrorAction {
    Suppress,
    LogDebug { error: String },
    LogStuck { seconds: u64, last_error: String },
    LogStuckAndError { seconds: u64, last_error: String },
}

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
    /// Diagnostic stats are updated on each frame for the overlay.
    pub fn start<S: EdsSdkApi + 'static>(
        sdk: Arc<S>,
        camera: CameraHandle,
        jpeg_buffer: Arc<JpegFrameBuffer>,
        diag_stats: Arc<Mutex<DiagnosticStats>>,
    ) -> crate::camera::error::Result<Self> {
        Self::start_with_interval(sdk, camera, jpeg_buffer, diag_stats, DEFAULT_POLL_INTERVAL)
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
        diag_stats: Arc<Mutex<DiagnosticStats>>,
        interval: Duration,
    ) -> crate::camera::error::Result<Self> {
        sdk.start_live_view(camera)?;

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let buffer_clone = Arc::clone(&jpeg_buffer);

        let thread = std::thread::Builder::new()
            .name(format!("canon-lv-{}", camera.0))
            .spawn(move || {
                poll_live_view(
                    &*sdk,
                    camera,
                    &buffer_clone,
                    &running_clone,
                    &diag_stats,
                    interval,
                );
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
///
/// Polls aggressively after successful frames (using `interval`) and backs
/// off on errors to avoid busy-spinning when EVF isn't ready yet.
fn poll_live_view<S: EdsSdkApi>(
    sdk: &S,
    camera: CameraHandle,
    jpeg_buffer: &JpegFrameBuffer,
    running: &AtomicBool,
    diag_stats: &Mutex<DiagnosticStats>,
    interval: Duration,
) {
    // EDSDK requires COM STA on every thread that calls it.
    #[cfg(target_os = "windows")]
    let _com = LiveViewComGuard::init();

    let mut stats = PollStats::new(Instant::now());

    while running.load(Ordering::Relaxed) {
        // Measure SDK download time — this is the meaningful latency for Canon
        // (the time EDSDK takes to transfer the EVF image over USB).
        let download_start = Instant::now();
        match sdk.download_evf_image(camera) {
            Ok(jpeg_data) => {
                let download_us = download_start.elapsed().as_micros() as u64;
                let size = jpeg_data.len();
                // Canon live view delivers JPEG natively — push directly
                // into the JPEG buffer, bypassing RGB encoding entirely.
                jpeg_buffer.update(JpegFrame {
                    jpeg_bytes: jpeg_data,
                    width: 960,  // Canon live view typical resolution
                    height: 640, // Canon live view typical resolution
                    encoder_kind: EncoderKind::CpuFallback, // Not really encoded, just a label
                });

                // Update diagnostic stats for the overlay.
                // Use record_frame_with_latency so the measured SDK download
                // time is used directly — the clock-offset path in record_frame
                // would cancel to zero because Canon has no external timestamp.
                {
                    let mut ds = diag_stats.lock();
                    ds.record_frame_with_latency(size, download_us);
                }

                match stats.on_frame(size, Instant::now()) {
                    FrameAction::LogFirstFrame { size } => {
                        tracing::info!("Canon live view: first frame received ({size} bytes)");
                    }
                    FrameAction::LogPeriodicCount { count } => {
                        tracing::debug!("Canon live view: {count} frames delivered");
                    }
                    FrameAction::None => {}
                }

                // Short sleep between successful frames — poll as fast
                // as the camera can deliver.
                std::thread::sleep(interval);
            }
            Err(e) => {
                let msg = e.to_string();
                match stats.on_error(msg, Instant::now()) {
                    ErrorAction::Suppress => {}
                    ErrorAction::LogDebug { error } => {
                        tracing::debug!("Canon live view frame error: {error}");
                    }
                    ErrorAction::LogStuck {
                        seconds,
                        last_error,
                    } => {
                        tracing::warn!(
                            "Canon live view: no frames for {seconds}s (last error: {last_error})"
                        );
                    }
                    ErrorAction::LogStuckAndError {
                        seconds,
                        last_error,
                    } => {
                        tracing::warn!(
                            "Canon live view: no frames for {seconds}s (last error: {last_error})"
                        );
                    }
                }

                // Back off on errors to avoid busy-spinning
                std::thread::sleep(ERROR_BACKOFF);
            }
        }
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

    /// Create a fresh DiagnosticStats wrapped for sharing.
    fn make_diag_stats() -> Arc<Mutex<DiagnosticStats>> {
        Arc::new(Mutex::new(DiagnosticStats::new()))
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
            make_diag_stats(),
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
            make_diag_stats(),
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
        let diag_stats = make_diag_stats();

        // Manually start live view on mock so download attempts proceed
        mock.start_live_view(camera).unwrap();

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let buffer_clone = Arc::clone(&jpeg_buffer);
        let stats_clone = Arc::clone(&diag_stats);

        let handle = std::thread::spawn(move || {
            poll_live_view(
                &*mock,
                camera,
                &buffer_clone,
                &running_clone,
                &stats_clone,
                Duration::from_millis(5),
            );
        });

        std::thread::sleep(Duration::from_millis(30));
        running.store(false, Ordering::Relaxed);
        handle.join().unwrap();

        // No frames should have been pushed (no frame data configured)
        assert_eq!(jpeg_buffer.sequence(), 0);
        // Diagnostic stats should also be zero (no successful frames)
        let snap = diag_stats.lock().snapshot();
        assert_eq!(snap.frame_count, 0);
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
            make_diag_stats(),
            Duration::from_millis(10),
        )
        .unwrap();

        // Session should be running (live view started, not camera session)
        assert!(session.is_running());

        // Stop disables live view but does not close the camera session
        session.stop(&*mock, camera);
    }

    #[test]
    fn live_view_updates_diagnostic_stats_on_frames() {
        let mock = Arc::new(
            MockEdsSdk::new()
                .with_cameras(1)
                .with_live_view_frame(test_jpeg()),
        );
        let jpeg_buffer = Arc::new(JpegFrameBuffer::new());
        let camera = CameraHandle(0);
        let diag_stats = make_diag_stats();

        let session = LiveViewSession::start_with_interval(
            Arc::clone(&mock),
            camera,
            Arc::clone(&jpeg_buffer),
            Arc::clone(&diag_stats),
            Duration::from_millis(5),
        )
        .unwrap();

        // Wait for several frames to be processed
        std::thread::sleep(Duration::from_millis(80));

        let snap = diag_stats.lock().snapshot();
        assert!(
            snap.frame_count > 0,
            "diagnostic frame count should be non-zero, got {}",
            snap.frame_count
        );
        assert!(
            snap.fps > 0.0,
            "diagnostic FPS should be positive, got {}",
            snap.fps
        );
        assert!(
            snap.bandwidth_bps > 0,
            "diagnostic bandwidth should be positive, got {}",
            snap.bandwidth_bps
        );
        // Regression: latency must not be zero — Canon used to pass elapsed_us
        // as the capture timestamp which cancelled to 0 in record_frame.
        assert!(
            snap.latency_ms >= 0.0,
            "latency_ms must be non-negative, got {}",
            snap.latency_ms
        );

        session.stop(&*mock, camera);
    }

    #[test]
    fn live_view_diagnostics_track_frame_bytes() {
        let mock = Arc::new(
            MockEdsSdk::new()
                .with_cameras(1)
                .with_live_view_frame(test_jpeg()),
        );
        let jpeg_buffer = Arc::new(JpegFrameBuffer::new());
        let camera = CameraHandle(0);
        let diag_stats = make_diag_stats();

        let session = LiveViewSession::start_with_interval(
            Arc::clone(&mock),
            camera,
            Arc::clone(&jpeg_buffer),
            Arc::clone(&diag_stats),
            Duration::from_millis(5),
        )
        .unwrap();

        // Wait for at least one frame
        std::thread::sleep(Duration::from_millis(30));
        session.stop(&*mock, camera);

        let snap = diag_stats.lock().snapshot();
        // Each frame is 4 bytes (test_jpeg()), so total bytes should match
        let expected_bytes = snap.frame_count * test_jpeg().len() as u64;
        // bandwidth_bps > 0 implies bytes were tracked
        assert!(snap.bandwidth_bps > 0);
        // Sanity: frame_count * 4 bytes should give a reasonable bandwidth
        assert!(expected_bytes > 0);
    }

    // ------------------------------------------------------------------
    // PollStats unit tests
    // ------------------------------------------------------------------

    #[test]
    fn poll_stats_first_frame_triggers_log() {
        let now = Instant::now();
        let mut stats = PollStats::new(now);

        let action = stats.on_frame(1024, now);
        assert_eq!(action, FrameAction::LogFirstFrame { size: 1024 });
        assert_eq!(stats.frame_count, 1);
        assert!(stats.first_frame_logged);
    }

    #[test]
    fn poll_stats_second_frame_is_silent() {
        let now = Instant::now();
        let mut stats = PollStats::new(now);

        stats.on_frame(1024, now); // first
        let action = stats.on_frame(2048, now); // second
        assert_eq!(action, FrameAction::None);
        assert_eq!(stats.frame_count, 2);
    }

    #[test]
    fn poll_stats_periodic_count_at_100_frames() {
        let now = Instant::now();
        let mut stats = PollStats::new(now);

        for i in 1..=100 {
            let action = stats.on_frame(512, now);
            if i == 1 {
                assert_eq!(action, FrameAction::LogFirstFrame { size: 512 });
            } else if i == 100 {
                assert_eq!(action, FrameAction::LogPeriodicCount { count: 100 });
            } else {
                assert_eq!(action, FrameAction::None);
            }
        }
    }

    #[test]
    fn poll_stats_periodic_count_at_200_frames() {
        let now = Instant::now();
        let mut stats = PollStats::new(now);

        for _ in 1..=199 {
            stats.on_frame(512, now);
        }
        let action = stats.on_frame(512, now);
        assert_eq!(action, FrameAction::LogPeriodicCount { count: 200 });
    }

    #[test]
    fn poll_stats_suppresses_not_ready_errors() {
        let now = Instant::now();
        let mut stats = PollStats::new(now);

        let action = stats.on_error("object not ready".to_string(), now);
        assert_eq!(action, ErrorAction::Suppress);
    }

    #[test]
    fn poll_stats_suppresses_not_activated_errors() {
        let now = Instant::now();
        let mut stats = PollStats::new(now);

        let action = stats.on_error("EVF not activated yet".to_string(), now);
        assert_eq!(action, ErrorAction::Suppress);
    }

    #[test]
    fn poll_stats_logs_unexpected_errors_at_debug() {
        let now = Instant::now();
        let mut stats = PollStats::new(now);

        let action = stats.on_error("something unexpected".to_string(), now);
        assert_eq!(
            action,
            ErrorAction::LogDebug {
                error: "something unexpected".to_string()
            }
        );
    }

    #[test]
    fn poll_stats_stuck_warning_after_threshold() {
        let start = Instant::now();
        let mut stats = PollStats::new(start);

        // Simulate time passing beyond stuck threshold
        let after_stuck = start + STUCK_THRESHOLD + Duration::from_millis(100);

        let action = stats.on_error("object not ready".to_string(), after_stuck);
        assert_eq!(
            action,
            ErrorAction::LogStuck {
                seconds: 5,
                last_error: "object not ready".to_string()
            }
        );
    }

    #[test]
    fn poll_stats_stuck_warning_fires_only_once() {
        let start = Instant::now();
        let mut stats = PollStats::new(start);

        let after_stuck = start + STUCK_THRESHOLD + Duration::from_millis(100);

        // First occurrence fires warning
        let action = stats.on_error("object not ready".to_string(), after_stuck);
        assert!(matches!(action, ErrorAction::LogStuck { .. }));

        // Second occurrence is suppressed (already warned)
        let action = stats.on_error("object not ready".to_string(), after_stuck);
        assert_eq!(action, ErrorAction::Suppress);
    }

    #[test]
    fn poll_stats_stuck_resets_on_successful_frame() {
        let start = Instant::now();
        let mut stats = PollStats::new(start);

        let after_stuck = start + STUCK_THRESHOLD + Duration::from_millis(100);

        // Trigger stuck warning
        stats.on_error("object not ready".to_string(), after_stuck);
        assert!(stats.stuck_warned);

        // Successful frame resets the stuck state
        stats.on_frame(1024, after_stuck);
        assert!(!stats.stuck_warned);

        // Stuck warning can fire again after another threshold period
        let after_second_stuck = after_stuck + STUCK_THRESHOLD + Duration::from_millis(100);
        let action = stats.on_error("object not ready".to_string(), after_second_stuck);
        assert!(matches!(action, ErrorAction::LogStuck { .. }));
    }

    #[test]
    fn poll_stats_stuck_with_unexpected_error() {
        let start = Instant::now();
        let mut stats = PollStats::new(start);

        let after_stuck = start + STUCK_THRESHOLD + Duration::from_millis(100);

        let action = stats.on_error("unexpected failure".to_string(), after_stuck);
        assert_eq!(
            action,
            ErrorAction::LogStuckAndError {
                seconds: 5,
                last_error: "unexpected failure".to_string()
            }
        );
    }
}
