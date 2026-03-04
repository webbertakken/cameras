/// Convert RGB pixel data to NV12 format.
///
/// NV12 layout:
/// - Y plane: width * height bytes (full resolution)
/// - UV plane: width * (height/2) bytes (interleaved Cb/Cr, 2x2 subsampled)
///
/// Total output size: width * height * 3/2
///
/// Uses BT.601 coefficients for the conversion.
///
/// # Panics
///
/// Panics if width or height is odd (NV12 requires even dimensions) or if
/// `rgb.len()` does not equal `width * height * 3`.
pub fn rgb_to_nv12(rgb: &[u8], width: u32, height: u32) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;

    assert!(w % 2 == 0, "NV12 requires even width, got {w}");
    assert!(h % 2 == 0, "NV12 requires even height, got {h}");
    assert_eq!(
        rgb.len(),
        w * h * 3,
        "RGB buffer length mismatch: expected {}, got {}",
        w * h * 3,
        rgb.len()
    );

    let mut nv12 = vec![0u8; w * h * 3 / 2];

    // Y plane
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 3;
            let r = rgb[i] as f32;
            let g = rgb[i + 1] as f32;
            let b = rgb[i + 2] as f32;
            nv12[y * w + x] = (0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 255.0) as u8;
        }
    }

    // UV plane (interleaved Cb, Cr, 2x2 subsampled)
    let uv_offset = w * h;
    for y in (0..h).step_by(2) {
        for x in (0..w).step_by(2) {
            // Average the 2x2 block
            let (mut r, mut g, mut b) = (0f32, 0f32, 0f32);
            for dy in 0..2 {
                for dx in 0..2 {
                    let i = ((y + dy) * w + (x + dx)) * 3;
                    r += rgb[i] as f32;
                    g += rgb[i + 1] as f32;
                    b += rgb[i + 2] as f32;
                }
            }
            r /= 4.0;
            g /= 4.0;
            b /= 4.0;

            let cb = (-0.169 * r - 0.331 * g + 0.500 * b + 128.0).clamp(0.0, 255.0) as u8;
            let cr = (0.500 * r - 0.419 * g - 0.081 * b + 128.0).clamp(0.0, 255.0) as u8;
            let uv_idx = uv_offset + (y / 2) * w + x;
            nv12[uv_idx] = cb;
            nv12[uv_idx + 1] = cr;
        }
    }

    nv12
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a flat RGB buffer filled with a single colour.
    fn solid_rgb(r: u8, g: u8, b: u8, width: u32, height: u32) -> Vec<u8> {
        let pixel_count = (width * height) as usize;
        let mut buf = Vec::with_capacity(pixel_count * 3);
        for _ in 0..pixel_count {
            buf.push(r);
            buf.push(g);
            buf.push(b);
        }
        buf
    }

    #[test]
    fn output_size_is_correct() {
        for (w, h) in [(640, 480), (1920, 1080), (160, 120)] {
            let rgb = solid_rgb(0, 0, 0, w, h);
            let nv12 = rgb_to_nv12(&rgb, w, h);
            let expected = (w as usize) * (h as usize) * 3 / 2;
            assert_eq!(nv12.len(), expected, "wrong size for {w}x{h}");
        }
    }

    #[test]
    fn pure_red_produces_expected_y() {
        let rgb = solid_rgb(255, 0, 0, 4, 4);
        let nv12 = rgb_to_nv12(&rgb, 4, 4);
        // BT.601: Y = 0.299 * 255 ≈ 76.245 → 76
        assert_eq!(nv12[0], 76);
    }

    #[test]
    fn pure_green_produces_expected_y() {
        let rgb = solid_rgb(0, 255, 0, 4, 4);
        let nv12 = rgb_to_nv12(&rgb, 4, 4);
        // BT.601: Y = 0.587 * 255 ≈ 149.685 → 149
        assert_eq!(nv12[0], 149);
    }

    #[test]
    fn pure_white_produces_neutral_uv() {
        let rgb = solid_rgb(255, 255, 255, 4, 4);
        let nv12 = rgb_to_nv12(&rgb, 4, 4);

        // UV plane starts at w*h = 16
        let uv_offset = 4 * 4;
        // For white, chroma should be neutral (128)
        // Cb = -0.169*255 - 0.331*255 + 0.500*255 + 128 = 128.0
        // Cr =  0.500*255 - 0.419*255 - 0.081*255 + 128 = 128.0
        assert_eq!(nv12[uv_offset], 128, "Cb should be 128 for white");
        assert_eq!(nv12[uv_offset + 1], 128, "Cr should be 128 for white");
    }

    #[test]
    fn pure_black_produces_zero_y_and_neutral_uv() {
        let rgb = solid_rgb(0, 0, 0, 4, 4);
        let nv12 = rgb_to_nv12(&rgb, 4, 4);

        // Y should be 0
        assert_eq!(nv12[0], 0);

        // UV should be 128 (neutral)
        let uv_offset = 4 * 4;
        assert_eq!(nv12[uv_offset], 128, "Cb should be 128 for black");
        assert_eq!(nv12[uv_offset + 1], 128, "Cr should be 128 for black");
    }

    #[test]
    #[should_panic(expected = "NV12 requires even width")]
    fn panics_on_odd_width() {
        let rgb = solid_rgb(0, 0, 0, 3, 4);
        rgb_to_nv12(&rgb, 3, 4);
    }

    #[test]
    #[should_panic(expected = "NV12 requires even height")]
    fn panics_on_odd_height() {
        let rgb = solid_rgb(0, 0, 0, 4, 3);
        rgb_to_nv12(&rgb, 4, 3);
    }

    #[test]
    fn conversion_performance_under_5ms_at_640x480() {
        let rgb = solid_rgb(128, 64, 200, 640, 480);

        // Warm up to avoid cold-cache effects
        let _ = rgb_to_nv12(&rgb, 640, 480);

        let start = std::time::Instant::now();
        let _ = rgb_to_nv12(&rgb, 640, 480);
        let elapsed = start.elapsed();

        // Debug builds are ~2-3x slower; use relaxed threshold
        let limit_ms = if cfg!(debug_assertions) { 15 } else { 5 };
        assert!(
            elapsed.as_millis() < limit_ms,
            "640x480 conversion took {}ms, expected < {limit_ms}ms",
            elapsed.as_millis()
        );
    }
}
