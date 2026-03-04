#[cfg(windows)]
mod platform {
    use std::sync::atomic::Ordering;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
    use windows::Win32::System::Memory::{
        MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ,
    };
    use windows::Win32::System::Threading::{
        OpenEventW, WaitForSingleObject, SYNCHRONIZATION_SYNCHRONIZE,
    };

    use crate::error::Error;
    use crate::ring_buffer::{SharedFrameHeader, MAGIC, VERSION};

    /// Reads NV12 frames from a named shared memory ring buffer.
    ///
    /// Created by the COM DLL (virtual camera filter). The main application
    /// writes frames via [`SharedMemoryWriter`](crate::writer::SharedMemoryWriter).
    pub struct SharedMemoryReader {
        mapping_handle: HANDLE,
        event_handle: HANDLE,
        base_ptr: *const u8,
        _total_size: usize,
    }

    // SAFETY: same rationale as SharedMemoryWriter — synchronised via atomics.
    unsafe impl Send for SharedMemoryReader {}

    impl SharedMemoryReader {
        /// Open an existing named shared memory region.
        pub fn open(name: &str) -> Result<Self, Error> {
            let wide_name = to_wide(name);
            let event_name_str = format!("{name}_event");
            let wide_event_name = to_wide(&event_name_str);

            // SAFETY: Opening an existing named file mapping.
            let mapping_handle =
                unsafe { OpenFileMappingW(FILE_MAP_READ.0, false, PCWSTR(wide_name.as_ptr()))? };

            // Map the header first to read dimensions, then we know the full size.
            // SAFETY: mapping at least 64 bytes to read the header.
            let base_ptr =
                unsafe { MapViewOfFile(mapping_handle, FILE_MAP_READ, 0, 0, 0).Value as *const u8 };

            if base_ptr.is_null() {
                let err = windows::core::Error::from_thread();
                unsafe {
                    let _ = CloseHandle(mapping_handle);
                }
                return Err(err.into());
            }

            // Validate the header.
            // SAFETY: base_ptr points to a valid SharedFrameHeader.
            let header = unsafe { &*(base_ptr as *const SharedFrameHeader) };

            if header.magic != MAGIC {
                let magic = header.magic;
                unsafe {
                    let view = windows::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS {
                        Value: base_ptr as *mut _,
                    };
                    let _ = UnmapViewOfFile(view);
                    let _ = CloseHandle(mapping_handle);
                }
                return Err(Error::InvalidMagic(magic));
            }

            if header.version != VERSION {
                let version = header.version;
                unsafe {
                    let view = windows::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS {
                        Value: base_ptr as *mut _,
                    };
                    let _ = UnmapViewOfFile(view);
                    let _ = CloseHandle(mapping_handle);
                }
                return Err(Error::VersionMismatch {
                    expected: VERSION,
                    actual: version,
                });
            }

            let total_size =
                SharedFrameHeader::total_size(header.width, header.height, header.slot_count);

            // SAFETY: Opening an existing named event.
            let event_handle = unsafe {
                OpenEventW(
                    SYNCHRONIZATION_SYNCHRONIZE,
                    false,
                    PCWSTR(wide_event_name.as_ptr()),
                )?
            };

            Ok(Self {
                mapping_handle,
                event_handle,
                base_ptr,
                _total_size: total_size,
            })
        }

        /// Access the shared frame header.
        pub fn header(&self) -> &SharedFrameHeader {
            // SAFETY: base_ptr is valid and points to a SharedFrameHeader.
            unsafe { &*(self.base_ptr as *const SharedFrameHeader) }
        }

        /// Read the latest frame data from the ring buffer.
        ///
        /// Returns `None` if no frames have been written yet (sequence == 0).
        /// The returned slice borrows the shared memory directly.
        pub fn read_frame(&self) -> Option<&[u8]> {
            let header = self.header();
            let seq = header.sequence.load(Ordering::Acquire);
            if seq == 0 {
                return None;
            }

            // The writer increments write_index AFTER writing, so the most
            // recent completed frame is at (write_index - 1) mod slot_count.
            let write_idx = header.write_index.load(Ordering::Acquire);
            let slot = write_idx.wrapping_sub(1) % header.slot_count;
            let offset = SharedFrameHeader::slot_offset(slot, header.frame_size);

            // SAFETY: offset and frame_size are within bounds of the mapped region.
            Some(unsafe {
                std::slice::from_raw_parts(self.base_ptr.add(offset), header.frame_size as usize)
            })
        }

        /// Wait for a new frame signal with timeout.
        ///
        /// Returns `true` if a frame signal was received, `false` on timeout.
        pub fn wait_frame(&self, timeout_ms: u32) -> bool {
            // SAFETY: event_handle is valid.
            let result = unsafe { WaitForSingleObject(self.event_handle, timeout_ms) };
            result == WAIT_OBJECT_0
        }
    }

    impl Drop for SharedMemoryReader {
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
pub use platform::SharedMemoryReader;
