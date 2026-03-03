// Async JPEG encoding worker thread.
//
// Receives raw RGB frames from the capture callback via a bounded channel,
// encodes them to JPEG (hardware or CPU fallback), and stores the result
// in a JpegFrameBuffer for the IPC layer to read.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use parking_lot::Mutex;
use serde::Serialize;
use tracing::{debug, info, trace, warn};

use crate::preview::capture::Frame;
use crate::preview::mf_jpeg::encoder::EncoderKind;

/// A single JPEG-encoded frame ready for IPC delivery.
pub struct JpegFrame {
    /// JPEG-compressed image data.
    pub jpeg_bytes: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Which encoder produced this frame.
    pub encoder_kind: EncoderKind,
}

/// Thread-safe buffer holding the latest JPEG frame for a camera.
///
/// Single-slot buffer (not a ring buffer) — we only ever need the most
/// recent encoded frame for IPC. Older frames are simply replaced.
pub struct JpegFrameBuffer {
    frame: Mutex<Option<Arc<JpegFrame>>>,
    /// Monotonic counter incremented on each update, used for cache
    /// invalidation in the IPC layer.
    sequence: AtomicU64,
}

impl JpegFrameBuffer {
    pub fn new() -> Self {
        Self {
            frame: Mutex::new(None),
            sequence: AtomicU64::new(0),
        }
    }

    /// Store a new JPEG frame, replacing the previous one.
    pub fn update(&self, frame: JpegFrame) {
        *self.frame.lock() = Some(Arc::new(frame));
        self.sequence.fetch_add(1, Ordering::Relaxed);
    }

    /// Get the latest JPEG frame, if any.
    pub fn latest(&self) -> Option<Arc<JpegFrame>> {
        self.frame.lock().clone()
    }

    /// Monotonic sequence number — increases by 1 for each update.
    pub fn sequence(&self) -> u64 {
        self.sequence.load(Ordering::Relaxed)
    }
}

/// Tracks encoding performance metrics for a single camera session.
struct EncodingStats {
    /// Total frames encoded since the worker started.
    frames_encoded: u64,
    /// Cumulative encoding time across all frames.
    total_encode_time_us: u64,
    /// Duration of the most recent encode operation.
    last_encode_us: u64,
}

impl EncodingStats {
    fn new() -> Self {
        Self {
            frames_encoded: 0,
            total_encode_time_us: 0,
            last_encode_us: 0,
        }
    }

    fn record_encode(&mut self, duration_us: u64) {
        self.frames_encoded += 1;
        self.total_encode_time_us += duration_us;
        self.last_encode_us = duration_us;
    }

    /// Average encode time per frame in microseconds.
    fn avg_encode_us(&self) -> f64 {
        if self.frames_encoded == 0 {
            return 0.0;
        }
        self.total_encode_time_us as f64 / self.frames_encoded as f64
    }
}

/// Serialisable snapshot of encoding stats for IPC delivery.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodingSnapshot {
    /// Which encoder backend is active.
    pub encoder_kind: EncoderKind,
    /// Total frames encoded since the worker started.
    pub frames_encoded: u64,
    /// Total frames dropped (channel was full).
    pub frames_dropped: u64,
    /// Average encode time per frame in milliseconds.
    pub avg_encode_ms: f64,
    /// Most recent encode time in milliseconds.
    pub last_encode_ms: f64,
}

/// Configuration for the encode worker.
pub struct WorkerConfig {
    /// JPEG quality (1-100). Default 75.
    pub quality: u8,
    /// Maximum pending frames in the channel before dropping.
    /// Keeps memory bounded and avoids encoding stale frames.
    pub channel_capacity: usize,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            quality: 75,
            channel_capacity: 2,
        }
    }
}

/// Handle for sending raw frames to the encode worker.
///
/// Clone this and use it from the DirectShow callback thread.
/// Sending is non-blocking — if the channel is full, the frame is dropped.
#[derive(Clone)]
pub struct FrameSender {
    tx: mpsc::SyncSender<Frame>,
    drop_count: Arc<AtomicU64>,
}

impl FrameSender {
    /// Send a raw RGB frame to the worker for encoding.
    ///
    /// Returns `true` if the frame was enqueued, `false` if it was dropped
    /// because the worker is busy (channel full).
    pub fn send(&self, frame: Frame) -> bool {
        match self.tx.try_send(frame) {
            Ok(()) => true,
            Err(mpsc::TrySendError::Full(_)) => {
                self.drop_count.fetch_add(1, Ordering::Relaxed);
                trace!("encode worker channel full, dropping frame");
                false
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                warn!("encode worker channel disconnected");
                false
            }
        }
    }
}

/// Active encode worker for a single camera session.
///
/// Spawns a dedicated thread that:
/// 1. Receives raw RGB frames from the channel
/// 2. Encodes them to JPEG via Media Foundation (HW) or CPU fallback
/// 3. Stores the result in the JpegFrameBuffer
pub struct EncodeWorker {
    jpeg_buffer: Arc<JpegFrameBuffer>,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    /// Which encoder backend the worker is using (set after first encode).
    encoder_kind: Arc<Mutex<EncoderKind>>,
    /// Encoding performance stats (shared with the worker thread).
    stats: Arc<Mutex<EncodingStats>>,
    /// Frame drop counter (shared with FrameSender).
    drop_count: Arc<AtomicU64>,
}

impl EncodeWorker {
    /// Spawn a new encode worker thread.
    ///
    /// Returns `(worker, sender)` — the sender should be given to the capture
    /// callback; the worker owns the JPEG output buffer.
    pub fn spawn(config: WorkerConfig) -> (Self, FrameSender) {
        let (tx, rx) = mpsc::sync_channel::<Frame>(config.channel_capacity);
        let jpeg_buffer = Arc::new(JpegFrameBuffer::new());
        let running = Arc::new(AtomicBool::new(true));
        let encoder_kind = Arc::new(Mutex::new(EncoderKind::CpuFallback));
        let stats = Arc::new(Mutex::new(EncodingStats::new()));
        let drop_count = Arc::new(AtomicU64::new(0));

        let thread = {
            let jpeg_buffer = Arc::clone(&jpeg_buffer);
            let running = Arc::clone(&running);
            let encoder_kind = Arc::clone(&encoder_kind);
            let stats = Arc::clone(&stats);

            std::thread::Builder::new()
                .name("encode-worker".to_string())
                .spawn(move || {
                    Self::run(
                        rx,
                        &jpeg_buffer,
                        &running,
                        &encoder_kind,
                        &stats,
                        config.quality,
                    );
                })
                .expect("failed to spawn encode worker thread")
        };

        let worker = Self {
            jpeg_buffer,
            running,
            thread: Some(thread),
            encoder_kind,
            stats,
            drop_count: Arc::clone(&drop_count),
        };

        let sender = FrameSender { tx, drop_count };

        (worker, sender)
    }

    /// Get a reference to the JPEG output buffer.
    pub fn jpeg_buffer(&self) -> &Arc<JpegFrameBuffer> {
        &self.jpeg_buffer
    }

    /// Which encoder backend the worker is using.
    pub fn encoder_kind(&self) -> EncoderKind {
        *self.encoder_kind.lock()
    }

    /// Take a serialisable snapshot of encoding performance stats.
    pub fn encoding_snapshot(&self) -> EncodingSnapshot {
        let stats = self.stats.lock();
        EncodingSnapshot {
            encoder_kind: *self.encoder_kind.lock(),
            frames_encoded: stats.frames_encoded,
            frames_dropped: self.drop_count.load(Ordering::Relaxed),
            avg_encode_ms: stats.avg_encode_us() / 1000.0,
            last_encode_ms: stats.last_encode_us as f64 / 1000.0,
        }
    }

    /// Stop the worker thread. Idempotent.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }

    /// Worker thread main loop.
    fn run(
        rx: mpsc::Receiver<Frame>,
        jpeg_buffer: &JpegFrameBuffer,
        running: &AtomicBool,
        encoder_kind: &Mutex<EncoderKind>,
        stats: &Mutex<EncodingStats>,
        quality: u8,
    ) {
        info!("encode worker started (quality={quality})");

        // Try to create a persistent MF encoder. If it fails, we use CPU fallback
        // for every frame. The encoder is created lazily on the worker thread
        // because COM objects must be used on the thread that created them.
        #[cfg(target_os = "windows")]
        let mf_encoder = {
            // Initialise COM on the worker thread
            unsafe {
                let _ = windows::Win32::System::Com::CoInitializeEx(
                    None,
                    windows::Win32::System::Com::COINIT_MULTITHREADED,
                );
            }
            None::<crate::preview::mf_jpeg::encoder::JpegEncoder>
        };

        #[cfg(not(target_os = "windows"))]
        let mf_encoder = None::<()>;

        // Will be initialised on the first frame (we need width/height)
        #[allow(unused_mut)]
        let mut mf_encoder = mf_encoder;
        let mut encoder_initialised = false;

        while running.load(Ordering::Relaxed) {
            // Block up to 100ms waiting for a frame, then recheck `running`
            let frame = match rx.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(f) => f,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    debug!("encode worker channel disconnected, exiting");
                    break;
                }
            };

            // Drain any stale frames — only encode the freshest
            let frame = drain_to_latest(frame, &rx);

            // Lazily initialise the MF encoder on the first frame
            #[cfg(target_os = "windows")]
            if !encoder_initialised {
                match crate::preview::mf_jpeg::encoder::JpegEncoder::new(
                    frame.width,
                    frame.height,
                    quality,
                ) {
                    Some(enc) => {
                        info!(
                            "encode worker: using {} for {}x{}",
                            enc.kind(),
                            frame.width,
                            frame.height
                        );
                        *encoder_kind.lock() = enc.kind();
                        mf_encoder = Some(enc);
                    }
                    None => {
                        info!("encode worker: no MF encoder, using CPU fallback");
                        *encoder_kind.lock() = EncoderKind::CpuFallback;
                    }
                }
                encoder_initialised = true;
            }

            #[cfg(not(target_os = "windows"))]
            if !encoder_initialised {
                *encoder_kind.lock() = EncoderKind::CpuFallback;
                encoder_initialised = true;
                let _ = &mf_encoder; // suppress unused warning
            }

            // Encode the frame with timing
            let t0 = Instant::now();
            let (jpeg_bytes, kind) = encode_frame(&frame, &mf_encoder, quality);
            let encode_us = t0.elapsed().as_micros() as u64;

            stats.lock().record_encode(encode_us);

            jpeg_buffer.update(JpegFrame {
                jpeg_bytes,
                width: frame.width,
                height: frame.height,
                encoder_kind: kind,
            });
        }

        #[cfg(target_os = "windows")]
        unsafe {
            windows::Win32::System::Com::CoUninitialize();
        }

        info!("encode worker stopped");
    }
}

/// Drain any pending frames from the receiver, returning the most recent.
///
/// When the encoder is slower than the capture rate, multiple frames
/// can queue up. We only care about the latest one — encoding stale
/// frames wastes CPU/GPU time.
fn drain_to_latest(initial: Frame, rx: &mpsc::Receiver<Frame>) -> Frame {
    let mut latest = initial;
    loop {
        match rx.try_recv() {
            Ok(newer) => latest = newer,
            Err(_) => return latest,
        }
    }
}

/// Encode a single frame using the best available encoder.
#[cfg(target_os = "windows")]
fn encode_frame(
    frame: &Frame,
    mf_encoder: &Option<crate::preview::mf_jpeg::encoder::JpegEncoder>,
    quality: u8,
) -> (Vec<u8>, EncoderKind) {
    if let Some(enc) = mf_encoder {
        match enc.encode(&frame.data, frame.width, frame.height) {
            Ok(jpeg) => return (jpeg, enc.kind()),
            Err(e) => {
                warn!("MF encode failed, falling back to CPU: {e}");
            }
        }
    }

    // CPU fallback
    let jpeg =
        crate::preview::compress::compress_jpeg(&frame.data, frame.width, frame.height, quality);
    (jpeg, EncoderKind::CpuFallback)
}

#[cfg(not(target_os = "windows"))]
fn encode_frame(frame: &Frame, _mf_encoder: &Option<()>, quality: u8) -> (Vec<u8>, EncoderKind) {
    let jpeg =
        crate::preview::compress::compress_jpeg(&frame.data, frame.width, frame.height, quality);
    (jpeg, EncoderKind::CpuFallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(width: u32, height: u32, value: u8) -> Frame {
        Frame {
            data: vec![value; (width * height * 3) as usize],
            width,
            height,
            timestamp_us: 1000,
        }
    }

    fn make_rgb_frame(width: u32, height: u32) -> Frame {
        // Create a gradient pattern that compresses to valid JPEG
        let mut data = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            for x in 0..width {
                data.push((x % 256) as u8);
                data.push((y % 256) as u8);
                data.push(128);
            }
        }
        Frame {
            data,
            width,
            height,
            timestamp_us: 1000,
        }
    }

    #[test]
    fn jpeg_frame_buffer_returns_none_when_empty() {
        let buf = JpegFrameBuffer::new();
        assert!(buf.latest().is_none());
        assert_eq!(buf.sequence(), 0);
    }

    #[test]
    fn jpeg_frame_buffer_stores_and_retrieves() {
        let buf = JpegFrameBuffer::new();
        buf.update(JpegFrame {
            jpeg_bytes: vec![0xFF, 0xD8, 0x00],
            width: 640,
            height: 480,
            encoder_kind: EncoderKind::CpuFallback,
        });

        let latest = buf.latest().unwrap();
        assert_eq!(latest.jpeg_bytes[0], 0xFF);
        assert_eq!(latest.width, 640);
        assert_eq!(buf.sequence(), 1);
    }

    #[test]
    fn jpeg_frame_buffer_replaces_on_update() {
        let buf = JpegFrameBuffer::new();
        buf.update(JpegFrame {
            jpeg_bytes: vec![1],
            width: 10,
            height: 10,
            encoder_kind: EncoderKind::CpuFallback,
        });
        buf.update(JpegFrame {
            jpeg_bytes: vec![2],
            width: 20,
            height: 20,
            encoder_kind: EncoderKind::CpuFallback,
        });

        let latest = buf.latest().unwrap();
        assert_eq!(latest.jpeg_bytes[0], 2);
        assert_eq!(latest.width, 20);
        assert_eq!(buf.sequence(), 2);
    }

    #[test]
    fn jpeg_frame_buffer_latest_returns_arc() {
        let buf = JpegFrameBuffer::new();
        buf.update(JpegFrame {
            jpeg_bytes: vec![42],
            width: 10,
            height: 10,
            encoder_kind: EncoderKind::CpuFallback,
        });

        let a = buf.latest().unwrap();
        let b = buf.latest().unwrap();
        assert!(Arc::ptr_eq(&a, &b));
    }

    fn make_sender(tx: mpsc::SyncSender<Frame>) -> FrameSender {
        FrameSender {
            tx,
            drop_count: Arc::new(AtomicU64::new(0)),
        }
    }

    #[test]
    fn frame_sender_send_returns_true_when_space() {
        let (tx, _rx) = mpsc::sync_channel::<Frame>(2);
        let sender = make_sender(tx);
        assert!(sender.send(make_frame(10, 10, 128)));
    }

    #[test]
    fn frame_sender_send_returns_false_when_full() {
        let (tx, _rx) = mpsc::sync_channel::<Frame>(1);
        let sender = make_sender(tx);
        assert!(sender.send(make_frame(10, 10, 1)));
        // Channel is full — second send should return false
        assert!(!sender.send(make_frame(10, 10, 2)));
    }

    #[test]
    fn frame_sender_send_returns_false_when_disconnected() {
        let (tx, rx) = mpsc::sync_channel::<Frame>(2);
        let sender = make_sender(tx);
        drop(rx);
        assert!(!sender.send(make_frame(10, 10, 128)));
    }

    #[test]
    fn frame_sender_tracks_drop_count() {
        let (tx, _rx) = mpsc::sync_channel::<Frame>(1);
        let sender = make_sender(tx);
        assert!(sender.send(make_frame(10, 10, 1)));
        // Channel is full — drop counter should increment
        assert!(!sender.send(make_frame(10, 10, 2)));
        assert!(!sender.send(make_frame(10, 10, 3)));
        assert_eq!(sender.drop_count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn drain_to_latest_returns_only_frame_when_channel_empty() {
        let (_tx, rx) = mpsc::sync_channel::<Frame>(2);
        let frame = make_frame(10, 10, 42);
        let result = drain_to_latest(frame, &rx);
        assert_eq!(result.data[0], 42);
    }

    #[test]
    fn drain_to_latest_returns_newest_frame() {
        let (tx, rx) = mpsc::sync_channel::<Frame>(4);
        tx.send(make_frame(10, 10, 2)).unwrap();
        tx.send(make_frame(10, 10, 3)).unwrap();
        tx.send(make_frame(10, 10, 4)).unwrap();

        let initial = make_frame(10, 10, 1);
        let result = drain_to_latest(initial, &rx);
        assert_eq!(result.data[0], 4, "should return the newest frame");
    }

    #[test]
    fn encode_worker_spawn_and_stop() {
        let (mut worker, _sender) = EncodeWorker::spawn(WorkerConfig::default());
        assert!(worker.jpeg_buffer().latest().is_none());
        worker.stop();
    }

    #[test]
    fn encode_worker_encodes_frame() {
        let (mut worker, sender) = EncodeWorker::spawn(WorkerConfig {
            quality: 75,
            channel_capacity: 2,
        });

        let frame = make_rgb_frame(64, 64);
        assert!(sender.send(frame));

        // Wait for the worker to encode the frame
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while worker.jpeg_buffer().latest().is_none() {
            if std::time::Instant::now() > deadline {
                panic!("encode worker did not produce a frame within 5s");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let jpeg_frame = worker.jpeg_buffer().latest().unwrap();
        assert_eq!(jpeg_frame.jpeg_bytes[0], 0xFF, "missing JPEG SOI");
        assert_eq!(jpeg_frame.jpeg_bytes[1], 0xD8, "missing JPEG SOI");
        assert_eq!(jpeg_frame.width, 64);
        assert_eq!(jpeg_frame.height, 64);

        worker.stop();
    }

    #[test]
    fn encode_worker_multiple_frames_sequence_increments() {
        let (mut worker, sender) = EncodeWorker::spawn(WorkerConfig {
            quality: 75,
            channel_capacity: 4,
        });

        // Send frames with a delay so the worker processes each one
        // individually rather than draining them all at once.
        for _ in 0..3 {
            sender.send(make_rgb_frame(32, 32));
            // Wait for the worker to process before sending the next
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            let target_seq = worker.jpeg_buffer().sequence() + 1;
            while worker.jpeg_buffer().sequence() < target_seq {
                if std::time::Instant::now() > deadline {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }

        assert!(
            worker.jpeg_buffer().sequence() >= 3,
            "expected >= 3 encoded frames, got {}",
            worker.jpeg_buffer().sequence()
        );

        worker.stop();
    }

    #[test]
    fn encode_worker_config_defaults() {
        let config = WorkerConfig::default();
        assert_eq!(config.quality, 75);
        assert_eq!(config.channel_capacity, 2);
    }

    #[test]
    fn encoding_stats_records_encode_time() {
        let mut stats = EncodingStats::new();
        assert_eq!(stats.frames_encoded, 0);
        assert_eq!(stats.avg_encode_us(), 0.0);

        stats.record_encode(1000);
        stats.record_encode(3000);
        assert_eq!(stats.frames_encoded, 2);
        assert_eq!(stats.last_encode_us, 3000);
        assert!((stats.avg_encode_us() - 2000.0).abs() < 0.1);
    }

    #[test]
    fn encoding_snapshot_serialises_to_camel_case() {
        let snap = EncodingSnapshot {
            encoder_kind: EncoderKind::CpuFallback,
            frames_encoded: 100,
            frames_dropped: 5,
            avg_encode_ms: 1.5,
            last_encode_ms: 1.2,
        };
        let json = serde_json::to_value(&snap).unwrap();
        assert!(json["encoderKind"].is_string());
        assert_eq!(json["framesEncoded"], 100);
        assert_eq!(json["framesDropped"], 5);
        assert!(json["avgEncodeMs"].is_number());
        assert!(json["lastEncodeMs"].is_number());
    }

    #[test]
    fn encode_worker_snapshot_tracks_frames() {
        let (mut worker, sender) = EncodeWorker::spawn(WorkerConfig {
            quality: 75,
            channel_capacity: 2,
        });

        // Initial snapshot — no frames yet
        let snap = worker.encoding_snapshot();
        assert_eq!(snap.frames_encoded, 0);
        assert_eq!(snap.frames_dropped, 0);

        // Send a frame and wait for it to be encoded
        sender.send(make_rgb_frame(32, 32));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while worker.jpeg_buffer().sequence() < 1 {
            if std::time::Instant::now() > deadline {
                panic!("encode worker did not produce a frame within 5s");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let snap = worker.encoding_snapshot();
        assert_eq!(snap.frames_encoded, 1);
        assert!(snap.avg_encode_ms > 0.0, "encode time should be positive");
        assert!(
            snap.last_encode_ms > 0.0,
            "last encode time should be positive"
        );

        worker.stop();
    }
}
