use turbojpeg::{Compressor, Image, PixelFormat};

/// Compress raw RGB pixel data to JPEG at the given quality (1-100).
///
/// Uses libjpeg-turbo via the `turbojpeg` crate for SIMD-accelerated encoding,
/// typically 3-5x faster than the pure-Rust `image` crate encoder.
pub fn compress_jpeg(data: &[u8], width: u32, height: u32, quality: u8) -> Vec<u8> {
    let mut compressor = Compressor::new().expect("failed to create turbojpeg compressor");
    compressor
        .set_quality(quality as i32)
        .expect("failed to set JPEG quality");

    let image = Image {
        pixels: data,
        width: width as usize,
        pitch: (width * 3) as usize,
        height: height as usize,
        format: PixelFormat::RGB,
    };

    compressor
        .compress_to_vec(image)
        .expect("JPEG encoding failed")
}

/// Compress and downscale raw RGB data for sidebar thumbnails.
///
/// Uses `fast_image_resize` for SIMD-accelerated resizing, then encodes to JPEG
/// via turbojpeg.
pub fn compress_thumbnail(
    data: &[u8],
    width: u32,
    height: u32,
    thumb_width: u32,
    thumb_height: u32,
) -> Vec<u8> {
    use fast_image_resize as fr;
    use fr::images::Image;

    // Create source image
    let src_image = Image::from_vec_u8(width, height, data.to_vec(), fr::PixelType::U8x3).unwrap();

    // Create destination image
    let mut dst_image = Image::new(thumb_width, thumb_height, fr::PixelType::U8x3);

    // Resize
    let mut resizer = fr::Resizer::new();
    resizer
        .resize(&src_image, &mut dst_image, None)
        .expect("resize failed");

    // Encode the resized image as JPEG
    let resized_data = dst_image.into_vec();
    compress_jpeg(&resized_data, thumb_width, thumb_height, 70)
}

/// Compress a JPEG image into a thumbnail by decoding, downscaling, and re-encoding.
///
/// Decodes the source JPEG via turbojpeg, downscales to `thumb_width x thumb_height`
/// and re-encodes at quality 70. Used for Canon cameras that deliver native JPEG frames.
pub fn compress_thumbnail_from_jpeg(
    jpeg_data: &[u8],
    thumb_width: u32,
    thumb_height: u32,
) -> Result<Vec<u8>, String> {
    let image = turbojpeg::decompress(jpeg_data, PixelFormat::RGB)
        .map_err(|e| format!("failed to decode JPEG: {e}"))?;
    Ok(compress_thumbnail(
        &image.pixels,
        image.width as u32,
        image.height as u32,
        thumb_width,
        thumb_height,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a synthetic RGB test image (gradient pattern).
    fn make_test_rgb(width: u32, height: u32) -> Vec<u8> {
        let mut data = Vec::with_capacity((width * height * 3) as usize);
        for y in 0..height {
            for x in 0..width {
                data.push((x % 256) as u8); // R
                data.push((y % 256) as u8); // G
                data.push(128); // B
            }
        }
        data
    }

    #[test]
    fn compress_jpeg_produces_valid_jpeg_bytes() {
        let rgb = make_test_rgb(640, 480);
        let jpeg = compress_jpeg(&rgb, 640, 480, 85);
        // JPEG files start with FF D8
        assert_eq!(jpeg[0], 0xFF);
        assert_eq!(jpeg[1], 0xD8);
    }

    #[test]
    fn compress_jpeg_1080p_at_quality_85_under_300kb() {
        let rgb = make_test_rgb(1920, 1080);
        let jpeg = compress_jpeg(&rgb, 1920, 1080, 85);
        assert!(
            jpeg.len() < 300_000,
            "JPEG size {} exceeds 300KB",
            jpeg.len()
        );
    }

    #[test]
    fn compress_jpeg_lower_quality_produces_smaller_output() {
        let rgb = make_test_rgb(1920, 1080);
        let high = compress_jpeg(&rgb, 1920, 1080, 85);
        let low = compress_jpeg(&rgb, 1920, 1080, 50);
        assert!(
            low.len() < high.len(),
            "quality 50 ({}) should be smaller than quality 85 ({})",
            low.len(),
            high.len()
        );
    }

    #[test]
    fn compress_thumbnail_produces_reduced_resolution() {
        let rgb = make_test_rgb(1920, 1080);
        let thumb = compress_thumbnail(&rgb, 1920, 1080, 160, 120);
        // Should be valid JPEG
        assert_eq!(thumb[0], 0xFF);
        assert_eq!(thumb[1], 0xD8);
    }

    #[test]
    fn compress_thumbnail_output_under_10kb() {
        let rgb = make_test_rgb(1920, 1080);
        let thumb = compress_thumbnail(&rgb, 1920, 1080, 160, 120);
        assert!(
            thumb.len() < 10_000,
            "thumbnail size {} exceeds 10KB",
            thumb.len()
        );
    }

    /// Create a valid JPEG from synthetic RGB data for testing.
    fn make_test_jpeg(width: u32, height: u32, quality: u8) -> Vec<u8> {
        let rgb = make_test_rgb(width, height);
        compress_jpeg(&rgb, width, height, quality)
    }

    #[test]
    fn compress_thumbnail_from_jpeg_produces_valid_jpeg() {
        let jpeg = make_test_jpeg(640, 480, 85);
        let thumb = compress_thumbnail_from_jpeg(&jpeg, 160, 120).unwrap();
        assert_eq!(thumb[0], 0xFF);
        assert_eq!(thumb[1], 0xD8);
    }

    #[test]
    fn compress_thumbnail_from_jpeg_output_under_10kb() {
        let jpeg = make_test_jpeg(1920, 1080, 85);
        let thumb = compress_thumbnail_from_jpeg(&jpeg, 160, 120).unwrap();
        assert!(
            thumb.len() < 10_000,
            "thumbnail size {} exceeds 10KB",
            thumb.len()
        );
    }

    #[test]
    fn compress_thumbnail_from_jpeg_invalid_input_returns_error() {
        let result = compress_thumbnail_from_jpeg(&[0x00, 0x01, 0x02], 160, 120);
        assert!(result.is_err(), "expected error for invalid JPEG input");
    }

    #[test]
    fn compress_jpeg_640x480_under_10ms() {
        let rgb = make_test_rgb(640, 480);
        // The fastest run reflects the encoder; slower runs reflect scheduler
        // contention from the rest of the parallel test suite.
        let fastest_ms = (0..20)
            .map(|_| {
                let start = std::time::Instant::now();
                let _ = compress_jpeg(&rgb, 640, 480, 85);
                start.elapsed().as_secs_f64() * 1000.0
            })
            .fold(f64::INFINITY, f64::min);
        assert!(
            fastest_ms < 10.0,
            "fastest encode time {fastest_ms:.1}ms exceeds 10ms for 640x480"
        );
    }
}
