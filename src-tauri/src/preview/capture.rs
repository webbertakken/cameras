use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

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
    #[allow(dead_code)]
    device_id: String,
    buffer: Arc<FrameBuffer>,
    running: Arc<AtomicBool>,
    #[allow(dead_code)]
    thread: Option<JoinHandle<()>>,
}

impl CaptureSession {
    /// Create a new capture session (does not start capture yet).
    pub fn new(device_id: String, width: u32, height: u32, _fps: f32) -> Self {
        let _ = (width, height); // Will be used by platform capture
        Self {
            device_id,
            buffer: Arc::new(FrameBuffer::new(3)),
            running: Arc::new(AtomicBool::new(false)),
            thread: None,
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
        let session = CaptureSession::new("test-device".to_string(), 1920, 1080, 30.0);
        assert!(!session.is_running());
        assert!(session.buffer().latest().is_none());
    }

    #[test]
    fn capture_session_stop_is_idempotent() {
        let mut session = CaptureSession::new("test-device".to_string(), 640, 480, 30.0);
        session.stop();
        session.stop(); // Should not panic
        assert!(!session.is_running());
    }
}
