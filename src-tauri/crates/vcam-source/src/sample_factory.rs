//! Creates `IMFSample` objects from NV12 frame data.
//!
//! Each sample wraps an `IMFMediaBuffer` containing the raw NV12 pixel data,
//! with timestamps set for the target frame rate.

use std::sync::atomic::{AtomicI64, Ordering};

use windows::Win32::Media::MediaFoundation::{IMFSample, MFCreateMemoryBuffer, MFCreateSample};

use crate::TARGET_FPS;

/// Monotonically increasing sample timestamp counter.
/// Each sample advances by `FRAME_DURATION_100NS`.
static SAMPLE_TIME: AtomicI64 = AtomicI64::new(0);

/// Duration of one frame in 100-nanosecond units (MF's time base).
const FRAME_DURATION_100NS: i64 = 10_000_000 / TARGET_FPS as i64;

/// Create an `IMFSample` containing the given NV12 frame data.
///
/// The sample includes a memory buffer with the raw pixel data and
/// appropriate timestamps for the target frame rate.
pub(crate) fn create_nv12_sample(
    nv12_data: &[u8],
    _width: u32,
    _height: u32,
) -> windows_core::Result<IMFSample> {
    let buffer_size = nv12_data.len() as u32;

    // Create a memory buffer.
    let buffer = unsafe { MFCreateMemoryBuffer(buffer_size)? };

    // Lock the buffer and copy the NV12 data.
    unsafe {
        let mut raw_ptr: *mut u8 = std::ptr::null_mut();
        let mut max_len: u32 = 0;
        let mut cur_len: u32 = 0;
        buffer.Lock(&mut raw_ptr, Some(&mut max_len), Some(&mut cur_len))?;

        std::ptr::copy_nonoverlapping(nv12_data.as_ptr(), raw_ptr, nv12_data.len());

        buffer.Unlock()?;
        buffer.SetCurrentLength(buffer_size)?;
    }

    // Create the sample and add the buffer.
    let sample = unsafe { MFCreateSample()? };
    unsafe {
        sample.AddBuffer(&buffer)?;
    }

    // Set sample time and duration.
    let time = SAMPLE_TIME.fetch_add(FRAME_DURATION_100NS, Ordering::Relaxed);
    unsafe {
        sample.SetSampleTime(time)?;
        sample.SetSampleDuration(FRAME_DURATION_100NS)?;
    }

    Ok(sample)
}

/// Reset the sample timestamp counter (useful for testing).
#[cfg(test)]
pub(crate) fn reset_sample_time() {
    SAMPLE_TIME.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media_stream::generate_black_nv12;

    #[test]
    fn sample_has_buffer_with_correct_length() {
        reset_sample_time();

        let (data, w, h) = generate_black_nv12(4, 2);
        let sample = create_nv12_sample(&data, w, h).unwrap();

        let count = unsafe { sample.GetBufferCount().unwrap() };
        assert_eq!(count, 1);

        let buffer = unsafe { sample.GetBufferByIndex(0).unwrap() };
        let len = unsafe { buffer.GetCurrentLength().unwrap() };
        assert_eq!(len, data.len() as u32);
    }

    #[test]
    fn sample_timestamps_increment_by_frame_duration() {
        let (data, w, h) = generate_black_nv12(4, 2);

        let sample1 = create_nv12_sample(&data, w, h).unwrap();
        let sample2 = create_nv12_sample(&data, w, h).unwrap();

        let time1 = unsafe { sample1.GetSampleTime().unwrap() };
        let time2 = unsafe { sample2.GetSampleTime().unwrap() };

        // Consecutive samples should be exactly one frame duration apart.
        assert_eq!(time2 - time1, FRAME_DURATION_100NS);

        let dur = unsafe { sample1.GetSampleDuration().unwrap() };
        assert_eq!(dur, FRAME_DURATION_100NS);
    }

    #[test]
    fn sample_buffer_contains_correct_data() {
        reset_sample_time();

        let nv12_data: Vec<u8> = (0..12).collect(); // 4x2 NV12 = 12 bytes
        let sample = create_nv12_sample(&nv12_data, 4, 2).unwrap();

        let buffer = unsafe { sample.GetBufferByIndex(0).unwrap() };

        unsafe {
            let mut raw_ptr: *mut u8 = std::ptr::null_mut();
            let mut max_len: u32 = 0;
            let mut cur_len: u32 = 0;
            buffer
                .Lock(&mut raw_ptr, Some(&mut max_len), Some(&mut cur_len))
                .unwrap();

            let slice = std::slice::from_raw_parts(raw_ptr, cur_len as usize);
            assert_eq!(slice, &nv12_data[..]);

            buffer.Unlock().unwrap();
        }
    }

    #[test]
    fn frame_duration_is_correct_for_30fps() {
        // 30 fps → 333,333.3... 100ns ticks per frame.
        // Integer division: 10_000_000 / 30 = 333_333.
        assert_eq!(FRAME_DURATION_100NS, 333_333);
    }
}
