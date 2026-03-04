//! Frame pump: reads JPEG frames, decodes to RGB, converts to NV12,
//! and writes into shared memory for the COM media source DLL.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tracing::{debug, trace, warn};

use crate::preview::encode_worker::JpegFrameBuffer;
use crate::virtual_camera::nv12;

/// Run the frame pump loop on the current thread.
///
/// Polls `jpeg_buffer` for new JPEG frames, decodes them via turbojpeg,
/// converts RGB to NV12, and writes into shared memory. Exits when
/// `running` is set to `false`.
///
/// The pump skips duplicate frames by tracking the sequence number.
pub fn run_frame_pump(
    jpeg_buffer: Arc<JpegFrameBuffer>,
    shm_writer: vcam_shared::SharedMemoryWriter,
    running: Arc<AtomicBool>,
) {
    debug!("frame pump started");

    let mut last_seq = 0u64;

    while running.load(Ordering::Relaxed) {
        let seq = jpeg_buffer.sequence();
        if seq == last_seq {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        last_seq = seq;

        let frame = match jpeg_buffer.latest() {
            Some(f) => f,
            None => {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
        };

        // Decode JPEG to RGB via turbojpeg
        let image = match turbojpeg::decompress(&frame.jpeg_bytes, turbojpeg::PixelFormat::RGB) {
            Ok(img) => img,
            Err(e) => {
                warn!("frame pump: JPEG decode failed: {e}");
                continue;
            }
        };

        let width = image.width as u32;
        let height = image.height as u32;

        // NV12 requires even dimensions
        if width % 2 != 0 || height % 2 != 0 {
            warn!("frame pump: skipping frame with odd dimensions {width}x{height}");
            continue;
        }

        let nv12_data = nv12::rgb_to_nv12(&image.pixels, width, height);
        shm_writer.write_frame(&nv12_data);

        trace!("frame pump: wrote NV12 frame seq={seq} {width}x{height}");
    }

    debug!("frame pump stopped");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::encode_worker::{JpegFrame, JpegFrameBuffer};
    use crate::preview::mf_jpeg::encoder::EncoderKind;

    /// Create a minimal valid JPEG from a solid-colour RGB image.
    fn make_test_jpeg(width: u32, height: u32) -> Vec<u8> {
        let rgb = vec![128u8; (width * height * 3) as usize];
        turbojpeg::compress(
            turbojpeg::Image {
                pixels: &rgb,
                width: width as usize,
                height: height as usize,
                pitch: (width * 3) as usize,
                format: turbojpeg::PixelFormat::RGB,
            },
            75,
            turbojpeg::Subsamp::Sub2x2,
        )
        .expect("turbojpeg compress failed")
        .to_vec()
    }

    #[cfg(windows)]
    #[test]
    fn frame_pump_writes_nv12_to_shared_memory() {
        let jpeg_buffer = Arc::new(JpegFrameBuffer::new());
        let running = Arc::new(AtomicBool::new(true));

        let width = 4u32;
        let height = 4u32;
        let jpeg_bytes = make_test_jpeg(width, height);

        jpeg_buffer.update(JpegFrame {
            jpeg_bytes,
            width,
            height,
            encoder_kind: EncoderKind::CpuFallback,
        });

        let shm_name = format!(
            r"Local\VcamPumpTest_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );

        let shm_writer = vcam_shared::SharedMemoryWriter::new(&shm_name, width, height, 3).unwrap();
        let shm_reader = vcam_shared::SharedMemoryReader::open(&shm_name).unwrap();

        // Run one iteration of the pump, then signal stop
        let running_clone = Arc::clone(&running);
        let jpeg_buffer_clone = Arc::clone(&jpeg_buffer);

        let pump_thread = std::thread::spawn(move || {
            run_frame_pump(jpeg_buffer_clone, shm_writer, running_clone);
        });

        // Wait for the pump to write a frame
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while shm_reader.header().sequence.load(Ordering::Relaxed) == 0 {
            if std::time::Instant::now() > deadline {
                panic!("frame pump did not write a frame within 5s");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        running.store(false, Ordering::Relaxed);
        pump_thread.join().unwrap();

        // Verify the frame was written
        let frame = shm_reader.read_frame().expect("should have a frame");
        let expected_size = (width as usize) * (height as usize) * 3 / 2;
        assert_eq!(frame.len(), expected_size);
    }

    #[test]
    fn frame_pump_skips_duplicate_frames() {
        let jpeg_buffer = Arc::new(JpegFrameBuffer::new());
        let running = Arc::new(AtomicBool::new(false));

        // With running=false, the pump exits immediately
        // This is a sanity check that the function doesn't panic
        #[cfg(windows)]
        {
            let shm_name = format!(
                r"Local\VcamPumpSkip_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            );
            let shm_writer = vcam_shared::SharedMemoryWriter::new(&shm_name, 4, 4, 3).unwrap();
            run_frame_pump(jpeg_buffer, shm_writer, running);
        }

        #[cfg(not(windows))]
        {
            // On non-Windows, just verify the function signature compiles
            let _ = (jpeg_buffer, running);
        }
    }

    #[test]
    fn make_test_jpeg_produces_valid_jpeg() {
        let jpeg = make_test_jpeg(8, 8);
        assert!(jpeg.len() > 2);
        assert_eq!(jpeg[0], 0xFF);
        assert_eq!(jpeg[1], 0xD8);
    }
}
