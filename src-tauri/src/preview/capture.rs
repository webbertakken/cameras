use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
#[cfg(target_os = "windows")]
use tracing::{error, info};

use crate::diagnostics::stats::{DiagnosticSnapshot, DiagnosticStats};

/// Callback type for reporting capture errors to the frontend.
/// Arguments: (device_id, error_message).
pub type ErrorCallback = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// A single captured frame from the camera.
pub struct Frame {
    /// Raw pixel data (RGB).
    pub data: Vec<u8>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Capture timestamp in microseconds.
    pub timestamp_us: u64,
}

/// Thread-safe ring buffer for camera frames.
///
/// Stores up to `capacity` frames, overwriting the oldest when full.
/// Frames are wrapped in `Arc` so consumers get a cheap reference-counted
/// pointer instead of cloning multi-megabyte pixel buffers.
pub struct FrameBuffer {
    frames: Mutex<Vec<Option<Arc<Frame>>>>,
    capacity: usize,
    write_idx: Mutex<usize>,
    /// Monotonic counter incremented on each push — used for cache
    /// invalidation even when camera timestamps are unreliable (e.g. OBS
    /// Virtual Camera reports sample_time = 0 for every frame).
    sequence: AtomicU64,
}

impl FrameBuffer {
    /// Create a new ring buffer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let frames = (0..capacity).map(|_| None).collect();
        Self {
            frames: Mutex::new(frames),
            capacity,
            write_idx: Mutex::new(0),
            sequence: AtomicU64::new(0),
        }
    }

    /// Push a new frame into the buffer, overwriting the oldest if full.
    pub fn push(&self, frame: Frame) {
        let mut frames = self.frames.lock();
        let mut idx = self.write_idx.lock();
        frames[*idx] = Some(Arc::new(frame));
        *idx = (*idx + 1) % self.capacity;
        self.sequence.fetch_add(1, Ordering::Relaxed);
    }

    /// Return the monotonic sequence number. Increases by 1 for each
    /// pushed frame, regardless of the frame's own timestamp.
    pub fn sequence(&self) -> u64 {
        self.sequence.load(Ordering::Relaxed)
    }

    /// Get the most recently pushed frame, if any.
    ///
    /// Returns an `Arc<Frame>` — a cheap clone of a reference-counted pointer
    /// rather than copying the entire pixel buffer.
    pub fn latest(&self) -> Option<Arc<Frame>> {
        let frames = self.frames.lock();
        let idx = self.write_idx.lock();
        if self.capacity == 0 {
            return None;
        }
        let latest_idx = if *idx == 0 {
            self.capacity - 1
        } else {
            *idx - 1
        };
        frames[latest_idx].clone()
    }
}

/// Active capture session for a single camera.
pub struct CaptureSession {
    device_id: String,
    buffer: Arc<FrameBuffer>,
    running: Arc<AtomicBool>,
    /// Signals the watchdog to exit early during teardown.
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    watchdog: Option<JoinHandle<()>>,
    stats: Arc<Mutex<DiagnosticStats>>,
}

/// Payload emitted via the `preview-error` Tauri event when a capture
/// graph fails.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewErrorPayload {
    pub device_id: String,
    pub error: String,
}

/// Configuration for the frame watchdog timer.
struct WatchdogConfig {
    /// Maximum time to wait for the capture graph to set `running = true`.
    startup_timeout: std::time::Duration,
    /// Time to wait for the first frame after the graph is running.
    frame_timeout: std::time::Duration,
    /// Poll interval for the watchdog thread.
    poll_interval: std::time::Duration,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            startup_timeout: std::time::Duration::from_secs(30),
            frame_timeout: std::time::Duration::from_secs(5),
            poll_interval: std::time::Duration::from_millis(250),
        }
    }
}

impl CaptureSession {
    /// Create and start a capture session for the given device.
    ///
    /// On Windows, spawns a thread that builds a DirectShow filter graph
    /// (Source → SampleGrabber → NullRenderer) and delivers RGB24 frames
    /// into the shared FrameBuffer via an ISampleGrabberCB callback.
    ///
    /// If `on_error` is provided, it is called with `(device_id, error_msg)`
    /// when the capture graph fails, allowing the caller to surface errors
    /// to the frontend.
    pub fn new(
        device_id: String,
        friendly_name: String,
        width: u32,
        height: u32,
        _fps: f32,
        on_error: Option<ErrorCallback>,
    ) -> Self {
        let buffer = Arc::new(FrameBuffer::new(3));
        let running = Arc::new(AtomicBool::new(false));
        let shutdown = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(Mutex::new(DiagnosticStats::new()));

        // Clone on_error for the watchdog — the capture thread gets the original
        let on_error_wd = on_error.clone();

        let thread = {
            let device_id_clone = device_id.clone();
            let friendly_name_clone = friendly_name;
            let buffer_clone = Arc::clone(&buffer);
            let running_clone = Arc::clone(&running);
            let stats_clone = Arc::clone(&stats);

            #[cfg(target_os = "windows")]
            {
                Some(
                    std::thread::Builder::new()
                        .name(format!("capture-{}", &device_id))
                        .spawn(move || {
                            info!("capture thread starting for {device_id_clone}");
                            if let Err(e) = super::graph::directshow::run_capture_graph(
                                &device_id_clone,
                                &friendly_name_clone,
                                width,
                                height,
                                buffer_clone,
                                running_clone,
                                stats_clone,
                            ) {
                                error!("capture graph failed for {device_id_clone}: {e}");
                                if let Some(cb) = &on_error {
                                    cb(&device_id_clone, &e);
                                }
                            }
                            info!("capture thread exiting for {device_id_clone}");
                        })
                        .expect("failed to spawn capture thread"),
                )
            }

            #[cfg(not(target_os = "windows"))]
            {
                let _ = (
                    device_id_clone,
                    friendly_name_clone,
                    buffer_clone,
                    running_clone,
                    stats_clone,
                    width,
                    height,
                    on_error,
                );
                None
            }
        };

        let watchdog = {
            let device_id_wd = device_id.clone();
            let buffer_wd = Arc::clone(&buffer);
            let running_wd = Arc::clone(&running);
            let shutdown_wd = Arc::clone(&shutdown);

            Some(
                std::thread::Builder::new()
                    .name(format!("watchdog-{}", &device_id))
                    .spawn(move || {
                        Self::run_watchdog(
                            &device_id_wd,
                            &buffer_wd,
                            &running_wd,
                            &shutdown_wd,
                            on_error_wd.as_ref(),
                        );
                    })
                    .expect("failed to spawn watchdog thread"),
            )
        };

        Self {
            device_id,
            buffer,
            running,
            shutdown,
            thread,
            watchdog,
            stats,
        }
    }

    /// Get a reference to the frame buffer.
    pub fn buffer(&self) -> &Arc<FrameBuffer> {
        &self.buffer
    }

    /// Check if the capture session is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Return the device ID for this session.
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Take a snapshot of diagnostic stats for this session.
    pub fn diagnostics(&self) -> DiagnosticSnapshot {
        self.stats.lock().snapshot()
    }

    /// Watchdog: waits for the graph to start running, then checks that frames
    /// arrive within `FRAME_TIMEOUT`. Fires `on_error` and stops the session
    /// if the camera produces no frames.
    fn run_watchdog(
        device_id: &str,
        buffer: &FrameBuffer,
        running: &AtomicBool,
        shutdown: &AtomicBool,
        on_error: Option<&ErrorCallback>,
    ) {
        Self::run_watchdog_with_config(
            device_id,
            buffer,
            running,
            shutdown,
            on_error,
            WatchdogConfig::default(),
        );
    }

    /// Watchdog implementation with configurable timeouts (for testing).
    fn run_watchdog_with_config(
        device_id: &str,
        buffer: &FrameBuffer,
        running: &AtomicBool,
        shutdown: &AtomicBool,
        on_error: Option<&ErrorCallback>,
        config: WatchdogConfig,
    ) {
        let WatchdogConfig {
            startup_timeout,
            frame_timeout,
            poll_interval,
        } = config;
        // Phase 1: wait for `running` to become true (graph startup)
        let start = std::time::Instant::now();
        loop {
            if shutdown.load(Ordering::Relaxed) {
                return;
            }
            if running.load(Ordering::Relaxed) {
                break;
            }
            if start.elapsed() >= startup_timeout {
                // Graph never started — the capture thread will report its own error
                return;
            }
            std::thread::sleep(poll_interval);
        }

        // Phase 2: wait for at least one frame within frame_timeout
        let deadline = std::time::Instant::now() + frame_timeout;
        loop {
            if shutdown.load(Ordering::Relaxed) || !running.load(Ordering::Relaxed) {
                return;
            }
            if buffer.sequence() > 0 {
                // Frames are arriving — camera is healthy
                return;
            }
            if std::time::Instant::now() >= deadline {
                tracing::warn!(
                    "watchdog: no frames received within {}s for {device_id}",
                    frame_timeout.as_secs()
                );
                if let Some(cb) = on_error {
                    cb(
                        device_id,
                        &format!(
                            "Camera produces no frames ({}s timeout)",
                            frame_timeout.as_secs()
                        ),
                    );
                }
                running.store(false, Ordering::Relaxed);
                return;
            }
            std::thread::sleep(poll_interval);
        }
    }

    /// Stop the capture session. Idempotent — calling stop twice does not panic.
    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.watchdog.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(value: u8, timestamp: u64) -> Frame {
        Frame {
            data: vec![value; 100],
            width: 10,
            height: 10,
            timestamp_us: timestamp,
        }
    }

    #[test]
    fn frame_buffer_returns_none_when_empty() {
        let buf = FrameBuffer::new(3);
        assert!(buf.latest().is_none());
    }

    #[test]
    fn frame_buffer_stores_and_retrieves_latest() {
        let buf = FrameBuffer::new(3);
        buf.push(make_frame(1, 100));
        buf.push(make_frame(2, 200));

        let latest = buf.latest().unwrap();
        assert_eq!(latest.data[0], 2);
        assert_eq!(latest.timestamp_us, 200);
    }

    #[test]
    fn frame_buffer_overwrites_oldest_when_full() {
        let buf = FrameBuffer::new(3);
        buf.push(make_frame(1, 100));
        buf.push(make_frame(2, 200));
        buf.push(make_frame(3, 300));
        // Buffer is now full [1, 2, 3]; pushing again overwrites slot 0
        buf.push(make_frame(4, 400));

        let latest = buf.latest().unwrap();
        assert_eq!(latest.data[0], 4);
        assert_eq!(latest.timestamp_us, 400);
    }

    #[test]
    fn frame_buffer_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FrameBuffer>();
    }

    #[test]
    fn frame_buffer_latest_returns_arc_not_clone() {
        let buf = FrameBuffer::new(3);
        buf.push(make_frame(42, 100));

        let a = buf.latest().unwrap();
        let b = buf.latest().unwrap();

        // Both should point to the same allocation — no deep copy
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(a.data[0], 42);
    }

    #[test]
    fn capture_session_can_be_created() {
        let session = CaptureSession::new(
            "test-device".to_string(),
            String::new(),
            1920,
            1080,
            30.0,
            None,
        );
        assert!(!session.is_running());
        assert!(session.buffer().latest().is_none());
    }

    #[test]
    fn capture_session_stop_is_idempotent() {
        let mut session = CaptureSession::new(
            "test-device".to_string(),
            String::new(),
            640,
            480,
            30.0,
            None,
        );
        session.stop();
        session.stop(); // Should not panic
        assert!(!session.is_running());
    }

    #[test]
    fn preview_error_payload_serialises_correctly() {
        let payload = PreviewErrorPayload {
            device_id: "test-device".to_string(),
            error: "capture graph failed: 0x800705AA".to_string(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["deviceId"], "test-device");
        assert_eq!(json["error"], "capture graph failed: 0x800705AA");
    }

    #[test]
    fn error_callback_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ErrorCallback>();
    }

    #[test]
    fn capture_session_with_error_callback() {
        let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = Arc::clone(&called);
        let on_error: ErrorCallback = Arc::new(move |_device_id, _error| {
            called_clone.store(true, Ordering::Relaxed);
        });
        let session = CaptureSession::new(
            "test-device".to_string(),
            String::new(),
            640,
            480,
            30.0,
            Some(on_error),
        );
        // On non-Windows, no capture thread spawns, so callback won't fire
        // but the session should still be valid
        assert!(!session.is_running());
    }

    /// Short durations for watchdog tests — keeps tests under 200ms.
    fn fast_watchdog() -> WatchdogConfig {
        WatchdogConfig {
            startup_timeout: std::time::Duration::from_millis(50),
            frame_timeout: std::time::Duration::from_millis(50),
            poll_interval: std::time::Duration::from_millis(10),
        }
    }

    #[test]
    fn watchdog_does_not_fire_when_frames_arrive() {
        let buffer = FrameBuffer::new(3);
        let running = AtomicBool::new(true);
        let shutdown = AtomicBool::new(false);
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);
        let on_error: ErrorCallback = Arc::new(move |_, _| {
            called_clone.store(true, Ordering::Relaxed);
        });

        // Push a frame before the watchdog checks
        buffer.push(make_frame(1, 100));

        CaptureSession::run_watchdog_with_config(
            "test",
            &buffer,
            &running,
            &shutdown,
            Some(&on_error),
            fast_watchdog(),
        );

        assert!(!called.load(Ordering::Relaxed));
        assert!(running.load(Ordering::Relaxed));
    }

    #[test]
    fn watchdog_fires_when_no_frames_arrive() {
        let buffer = FrameBuffer::new(3);
        let running = AtomicBool::new(true);
        let shutdown = AtomicBool::new(false);
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);
        let on_error: ErrorCallback = Arc::new(move |_, _| {
            called_clone.store(true, Ordering::Relaxed);
        });

        CaptureSession::run_watchdog_with_config(
            "test",
            &buffer,
            &running,
            &shutdown,
            Some(&on_error),
            fast_watchdog(),
        );

        assert!(called.load(Ordering::Relaxed));
        assert!(!running.load(Ordering::Relaxed));
    }

    #[test]
    fn watchdog_exits_early_when_shutdown_signalled() {
        let buffer = FrameBuffer::new(3);
        let running = AtomicBool::new(false);
        let shutdown = AtomicBool::new(true);
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);
        let on_error: ErrorCallback = Arc::new(move |_, _| {
            called_clone.store(true, Ordering::Relaxed);
        });

        CaptureSession::run_watchdog_with_config(
            "test",
            &buffer,
            &running,
            &shutdown,
            Some(&on_error),
            fast_watchdog(),
        );

        assert!(!called.load(Ordering::Relaxed));
    }

    #[test]
    fn watchdog_exits_if_graph_never_starts() {
        let buffer = FrameBuffer::new(3);
        let running = AtomicBool::new(false);
        let shutdown = AtomicBool::new(false);
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = Arc::clone(&called);
        let on_error: ErrorCallback = Arc::new(move |_, _| {
            called_clone.store(true, Ordering::Relaxed);
        });

        CaptureSession::run_watchdog_with_config(
            "test",
            &buffer,
            &running,
            &shutdown,
            Some(&on_error),
            fast_watchdog(),
        );

        // Graph never ran — watchdog exits via startup timeout, not via error
        assert!(!called.load(Ordering::Relaxed));
    }
}
