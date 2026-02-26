use image::codecs::jpeg::JpegEncoder;
use image::{ImageBuffer, Rgb};

/// Compress raw RGB pixel data to JPEG at the given quality (1-100).
pub fn compress_jpeg(data: &[u8], width: u32, height: u32, quality: u8) -> Vec<u8> {
    let img: ImageBuffer<Rgb<u8>, _> =
        ImageBuffer::from_raw(width, height, data).expect("invalid buffer dimensions");

    let mut buf = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    img.write_with_encoder(encoder)
        .expect("JPEG encoding failed");
    buf
}

/// Compress and downscale raw RGB data for sidebar thumbnails.
///
/// Uses `fast_image_resize` for SIMD-accelerated resizing, then encodes to JPEG.
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
}
