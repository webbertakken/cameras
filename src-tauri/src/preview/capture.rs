use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
#[cfg(target_os = "windows")]
use tracing::{error, info};

use crate::diagnostics::stats::{DiagnosticSnapshot, DiagnosticStats};

/// A single captured frame from the camera.
#[derive(Clone)]
pub struct Frame {
    /// Raw pixel data (RGB) or compressed JPEG bytes.
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
/// Designed for a single-producer (capture thread) / single-consumer (IPC) pattern.
pub struct FrameBuffer {
    frames: Mutex<Vec<Option<Frame>>>,
    capacity: usize,
    write_idx: Mutex<usize>,
}

impl FrameBuffer {
    /// Create a new ring buffer with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let frames = (0..capacity).map(|_| None).collect();
        Self {
            frames: Mutex::new(frames),
            capacity,
            write_idx: Mutex::new(0),
        }
    }

    /// Push a new frame into the buffer, overwriting the oldest if full.
    pub fn push(&self, frame: Frame) {
        let mut frames = self.frames.lock();
        let mut idx = self.write_idx.lock();
        frames[*idx] = Some(frame);
        *idx = (*idx + 1) % self.capacity;
    }

    /// Get the most recently pushed frame, if any.
    pub fn latest(&self) -> Option<Frame> {
        let frames = self.frames.lock();
        let idx = self.write_idx.lock();
        // The latest frame is at (write_idx - 1) mod capacity
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
    thread: Option<JoinHandle<()>>,
    stats: Arc<Mutex<DiagnosticStats>>,
}

impl CaptureSession {
    /// Create and start a capture session for the given device.
    ///
    /// On Windows, spawns a thread that builds a DirectShow filter graph
    /// (Source → SampleGrabber → NullRenderer) and delivers RGB24 frames
    /// into the shared FrameBuffer via an ISampleGrabberCB callback.
    pub fn new(
        device_id: String,
        friendly_name: String,
        width: u32,
        height: u32,
        _fps: f32,
    ) -> Self {
        let buffer = Arc::new(FrameBuffer::new(3));
        let running = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(Mutex::new(DiagnosticStats::new()));

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
                );
                None
            }
        };

        Self {
            device_id,
            buffer,
            running,
            thread,
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

    /// Stop the capture session. Idempotent — calling stop twice does not panic.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
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
    fn capture_session_can_be_created() {
        let session =
            CaptureSession::new("test-device".to_string(), String::new(), 1920, 1080, 30.0);
        assert!(!session.is_running());
        assert!(session.buffer().latest().is_none());
    }

    #[test]
    fn capture_session_stop_is_idempotent() {
        let mut session =
            CaptureSession::new("test-device".to_string(), String::new(), 640, 480, 30.0);
        session.stop();
        session.stop(); // Should not panic
        assert!(!session.is_running());
    }
}
