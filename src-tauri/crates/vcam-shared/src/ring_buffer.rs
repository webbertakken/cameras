use std::sync::atomic::{AtomicU32, AtomicU64};

/// Magic value identifying valid shared memory: "VCAM" as little-endian u32.
pub const MAGIC: u32 = 0x5643_414D;

/// Protocol version.
pub const VERSION: u32 = 1;

/// Header size in bytes, cache-line aligned.
pub const HEADER_SIZE: usize = 64;

/// Pixel format identifiers.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Nv12 = 0,
}

/// Shared memory header placed at the start of the mapped region.
///
/// Layout is 64 bytes (one cache line), followed by `slot_count * frame_size`
/// bytes of NV12 frame data.
#[repr(C, align(64))]
pub struct SharedFrameHeader {
    pub magic: u32,
    pub version: u32,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub frame_size: u32,
    pub slot_count: u32,
    /// Current write slot index (mod `slot_count`).
    pub write_index: AtomicU32,
    /// Monotonically increasing frame counter.
    pub sequence: AtomicU64,
    // Remaining bytes pad to 64.
    _pad: [u8; 20],
}

// Compile-time guarantee that header is exactly 64 bytes.
const _: () = assert!(std::mem::size_of::<SharedFrameHeader>() == HEADER_SIZE);

impl SharedFrameHeader {
    /// Calculate NV12 frame size for the given dimensions.
    pub fn nv12_frame_size(width: u32, height: u32) -> u32 {
        // NV12: Y plane (width * height) + UV interleaved (width * height / 2)
        width * height * 3 / 2
    }

    /// Total shared memory size required for the given parameters.
    pub fn total_size(width: u32, height: u32, slot_count: u32) -> usize {
        let frame_size = Self::nv12_frame_size(width, height) as usize;
        HEADER_SIZE + slot_count as usize * frame_size
    }

    /// Byte offset from the start of shared memory to the given slot.
    pub fn slot_offset(slot_index: u32, frame_size: u32) -> usize {
        HEADER_SIZE + slot_index as usize * frame_size as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_layout_size_calculation() {
        // 1920x1080 NV12 = 1920 * 1080 * 1.5 = 3_110_400 bytes per frame
        let width = 1920;
        let height = 1080;
        let slots = 3;
        let frame_size = SharedFrameHeader::nv12_frame_size(width, height);
        assert_eq!(frame_size, 3_110_400);

        let total = SharedFrameHeader::total_size(width, height, slots);
        assert_eq!(total, 64 + 3 * 3_110_400);
    }

    #[test]
    fn header_size_is_64_bytes() {
        assert_eq!(std::mem::size_of::<SharedFrameHeader>(), 64);
    }

    #[test]
    fn header_alignment_is_64() {
        assert_eq!(std::mem::align_of::<SharedFrameHeader>(), 64);
    }

    #[test]
    fn slot_offset_calculation() {
        let frame_size = 3_110_400u32;
        assert_eq!(SharedFrameHeader::slot_offset(0, frame_size), 64);
        assert_eq!(
            SharedFrameHeader::slot_offset(1, frame_size),
            64 + 3_110_400
        );
        assert_eq!(
            SharedFrameHeader::slot_offset(2, frame_size),
            64 + 2 * 3_110_400
        );
    }
}
