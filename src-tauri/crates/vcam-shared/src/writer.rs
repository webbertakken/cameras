#[cfg(windows)]
mod platform {
    use std::ptr;
    use std::sync::atomic::Ordering;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows::Win32::System::Memory::{
        CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_WRITE, PAGE_READWRITE,
    };
    use windows::Win32::System::Threading::{CreateEventW, SetEvent};

    use crate::error::Error;
    use crate::ring_buffer::{PixelFormat, SharedFrameHeader, MAGIC, VERSION};

    /// Writes NV12 frames into a named shared memory ring buffer.
    ///
    /// Created by the main application. The COM DLL reads frames via
    /// [`SharedMemoryReader`](crate::reader::SharedMemoryReader).
    pub struct SharedMemoryWriter {
        mapping_handle: HANDLE,
        event_handle: HANDLE,
        base_ptr: *mut u8,
        frame_size: u32,
        slot_count: u32,
    }

    // SAFETY: The shared memory region is process-shared and synchronised via
    // atomics + event signalling. The writer is the sole producer.
    unsafe impl Send for SharedMemoryWriter {}

    impl SharedMemoryWriter {
        /// Create a new named shared memory region for frame transport.
        ///
        /// `name` is used as the shared memory object name (e.g. `"Local\\CamerasVCam"`).
        pub fn new(name: &str, width: u32, height: u32, slot_count: u32) -> Result<Self, Error> {
            let total_size = SharedFrameHeader::total_size(width, height, slot_count);
            let frame_size = SharedFrameHeader::nv12_frame_size(width, height);

            let wide_name = to_wide(name);
            let event_name_str = format!("{name}_event");
            let wide_event_name = to_wide(&event_name_str);

            // SAFETY: Creating a named file mapping backed by the page file.
            let mapping_handle = unsafe {
                CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    None,
                    PAGE_READWRITE,
                    (total_size >> 32) as u32,
                    total_size as u32,
                    PCWSTR(wide_name.as_ptr()),
                )?
            };

            // SAFETY: Mapping the entire region as read-write.
            let base_ptr = unsafe {
                MapViewOfFile(mapping_handle, FILE_MAP_WRITE, 0, 0, total_size).Value as *mut u8
            };

            if base_ptr.is_null() {
                let err = windows::core::Error::from_thread();
                unsafe {
                    let _ = CloseHandle(mapping_handle);
                }
                return Err(err.into());
            }

            // SAFETY: Creating a named auto-reset event for frame signalling.
            let event_handle =
                unsafe { CreateEventW(None, false, false, PCWSTR(wide_event_name.as_ptr()))? };

            // Initialise the header.
            // SAFETY: base_ptr is valid, aligned to 64, and we own the mapping.
            let header = unsafe { &mut *(base_ptr as *mut SharedFrameHeader) };

            // Zero the entire region first.
            // SAFETY: base_ptr is valid for total_size bytes.
            unsafe { ptr::write_bytes(base_ptr, 0, total_size) };

            header.magic = MAGIC;
            header.version = VERSION;
            header.width = width;
            header.height = height;
            header.format = PixelFormat::Nv12 as u32;
            header.frame_size = frame_size;
            header.slot_count = slot_count;
            // write_index and sequence are already zeroed.

            Ok(Self {
                mapping_handle,
                event_handle,
                base_ptr,
                frame_size,
                slot_count,
            })
        }

        /// Write a single NV12 frame into the next ring buffer slot.
        ///
        /// # Panics
        /// Panics if `nv12_data.len()` does not match the expected frame size.
        pub fn write_frame(&self, nv12_data: &[u8]) {
            assert_eq!(
                nv12_data.len(),
                self.frame_size as usize,
                "frame data size mismatch"
            );

            let header = self.header();

            // Determine slot to write into.
            let slot = header.write_index.load(Ordering::Acquire) % self.slot_count;
            let offset = SharedFrameHeader::slot_offset(slot, self.frame_size);

            // SAFETY: offset + frame_size <= total_size by construction.
            unsafe {
                let dst = self.base_ptr.add(offset);
                ptr::copy_nonoverlapping(nv12_data.as_ptr(), dst, nv12_data.len());
            }

            // Advance write_index and bump sequence.
            header
                .write_index
                .store(slot.wrapping_add(1), Ordering::Release);
            header.sequence.fetch_add(1, Ordering::Release);

            // Signal the reader.
            // SAFETY: event_handle is valid.
            unsafe {
                let _ = SetEvent(self.event_handle);
            }
        }

        fn header(&self) -> &SharedFrameHeader {
            // SAFETY: base_ptr is valid and points to a SharedFrameHeader.
            unsafe { &*(self.base_ptr as *const SharedFrameHeader) }
        }

        /// Current frame sequence number.
        pub fn sequence(&self) -> u64 {
            self.header().sequence.load(Ordering::Acquire)
        }

        /// Current write index (raw, not wrapped).
        pub fn write_index(&self) -> u32 {
            self.header().write_index.load(Ordering::Acquire)
        }
    }

    impl Drop for SharedMemoryWriter {
        fn drop(&mut self) {
            // SAFETY: handles and pointer are valid from construction.
            unsafe {
                let view = windows::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.base_ptr as *mut _,
                };
                let _ = UnmapViewOfFile(view);
                let _ = CloseHandle(self.mapping_handle);
                let _ = CloseHandle(self.event_handle);
            }
        }
    }

    /// Encode a Rust `&str` as a null-terminated UTF-16 wide string.
    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

#[cfg(windows)]
pub use platform::SharedMemoryWriter;
