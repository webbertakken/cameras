//! Frame pump: reads JPEG frames, decodes to RGB, converts to NV12,
//! and writes into shared memory for the COM media source DLL.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tracing::{error, info, trace};
use vcam_shared::SharedMemoryOwner;

use crate::preview::encode_worker::JpegFrameBuffer;
use crate::virtual_camera::nv12;

/// Run the frame pump loop on the current thread.
///
/// Polls `jpeg_buffer` for new JPEG frames, decodes them via turbojpeg,
/// converts RGB to NV12, and writes into the pre-created `SharedMemoryOwner`.
/// Exits when `running` is set to `false`.
pub fn run_frame_pump(
    jpeg_buffer: Arc<JpegFrameBuffer>,
    shm_owner: Arc<SharedMemoryOwner>,
    running: Arc<AtomicBool>,
) {
    info!("Frame pump started");

    let mut last_seq = 0u64;
    let mut frames_delivered = 0u64;

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

        // Decode JPEG to BGR via turbojpeg — NV12 conversion expects BGR byte
        // order to match the Windows MFVideoFormat_RGB24 convention (which is
        // physically BGR despite the name).
        let image = match turbojpeg::decompress(&frame.jpeg_bytes, turbojpeg::PixelFormat::BGR) {
            Ok(img) => img,
            Err(e) => {
                error!("Frame pump: JPEG decode failed: {e}");
                continue;
            }
        };

        let src_w = image.width as u32;
        let src_h = image.height as u32;
        let target_w = vcam_shared::DEFAULT_WIDTH;
        let target_h = vcam_shared::DEFAULT_HEIGHT;

        // Resize if the decoded frame doesn't match shared memory dimensions.
        // The resize operates on opaque 3-byte pixels (U8x3), so byte order is
        // preserved regardless of whether data is BGR or RGB.
        let bgr_data = if src_w == target_w && src_h == target_h {
            image.pixels
        } else {
            match resize_rgb(image.pixels, src_w, src_h, target_w, target_h) {
                Some(data) => data,
                None => {
                    error!("Frame pump: resize failed ({src_w}x{src_h} -> {target_w}x{target_h})");
                    continue;
                }
            }
        };

        let nv12_data = nv12::bgr_to_nv12(&bgr_data, target_w, target_h);
        shm_owner.write_frame(&nv12_data);

        frames_delivered += 1;
        if frames_delivered == 1 {
            if src_w != target_w || src_h != target_h {
                info!(
                    "Frame pump: first NV12 frame delivered ({src_w}x{src_h} -> {target_w}x{target_h}, seq={seq})"
                );
            } else {
                info!("Frame pump: first NV12 frame delivered ({target_w}x{target_h}, seq={seq})");
            }
        }
        trace!(
            "Frame pump: wrote NV12 frame seq={seq} {target_w}x{target_h} (total={frames_delivered})"
        );
    }

    info!("Frame pump stopped after {frames_delivered} frames delivered");
}

/// Resize an RGB buffer using SIMD-accelerated `fast_image_resize` with
/// aspect-ratio-preserving letterboxing.
///
/// Returns `None` if the source buffer has an invalid size or the resize fails.
fn resize_rgb(src: Vec<u8>, src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Option<Vec<u8>> {
    use fast_image_resize as fr;

    let src_image = fr::images::Image::from_vec_u8(src_w, src_h, src, fr::PixelType::U8x3).ok()?;

    // Calculate the largest rectangle that preserves source aspect ratio
    // while fitting within (dst_w, dst_h).
    let scale = f64::min(dst_w as f64 / src_w as f64, dst_h as f64 / src_h as f64);
    let inner_w = ((src_w as f64 * scale).round() as u32).max(1);
    let inner_h = ((src_h as f64 * scale).round() as u32).max(1);

    let mut inner_image = fr::images::Image::new(inner_w, inner_h, fr::PixelType::U8x3);
    let mut resizer = fr::Resizer::new();
    let options =
        fr::ResizeOptions::new().resize_alg(fr::ResizeAlg::Convolution(fr::FilterType::Bilinear));
    resizer
        .resize(&src_image, &mut inner_image, Some(&options))
        .ok()?;

    // Letterbox: place resized image centred on a black background
    if inner_w == dst_w && inner_h == dst_h {
        return Some(inner_image.into_vec());
    }

    let mut output = vec![0u8; (dst_w * dst_h * 3) as usize];
    let offset_x = ((dst_w - inner_w) / 2) as usize;
    let offset_y = ((dst_h - inner_h) / 2) as usize;
    let inner_buf = inner_image.into_vec();
    let inner_stride = inner_w as usize * 3;
    let dst_stride = dst_w as usize * 3;

    for row in 0..inner_h as usize {
        let src_start = row * inner_stride;
        let dst_start = (offset_y + row) * dst_stride + offset_x * 3;
        output[dst_start..dst_start + inner_stride]
            .copy_from_slice(&inner_buf[src_start..src_start + inner_stride]);
    }

    Some(output)
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

        // Source frame is smaller than target — the pump resizes to DEFAULT dimensions
        let src_w = 4u32;
        let src_h = 4u32;
        let target_w = vcam_shared::DEFAULT_WIDTH;
        let target_h = vcam_shared::DEFAULT_HEIGHT;
        let jpeg_bytes = make_test_jpeg(src_w, src_h);

        jpeg_buffer.update(JpegFrame {
            jpeg_bytes,
            width: src_w,
            height: src_h,
            encoder_kind: EncoderKind::CpuFallback,
        });

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pump_test_shm.bin");

        let shm_owner = Arc::new(SharedMemoryOwner::new(&path, target_w, target_h, 3).unwrap());
        let shm_reader = vcam_shared::SharedMemoryReader::open_file(&path).unwrap();

        let running_clone = Arc::clone(&running);
        let jpeg_buffer_clone = Arc::clone(&jpeg_buffer);
        let shm_owner_clone = Arc::clone(&shm_owner);

        let pump_thread = std::thread::spawn(move || {
            run_frame_pump(jpeg_buffer_clone, shm_owner_clone, running_clone);
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

        // Verify the frame was written with target dimensions (NV12 = w*h*3/2)
        let frame = shm_reader.read_frame().expect("should have a frame");
        let expected_size = (target_w as usize) * (target_h as usize) * 3 / 2;
        assert_eq!(frame.len(), expected_size);

        // Drop reader before owner so the file can be deleted.
        drop(shm_reader);
        drop(shm_owner);
    }

    #[test]
    fn make_test_jpeg_produces_valid_jpeg() {
        let jpeg = make_test_jpeg(8, 8);
        assert!(jpeg.len() > 2);
        assert_eq!(jpeg[0], 0xFF);
        assert_eq!(jpeg[1], 0xD8);
    }
}
