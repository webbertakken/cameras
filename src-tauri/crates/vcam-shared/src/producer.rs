#[cfg(windows)]
mod platform {
    use std::ptr;
    use std::sync::atomic::Ordering;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Memory::{
        MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_WRITE,
    };
    use windows::Win32::System::Threading::{OpenEventW, SetEvent, SYNCHRONIZATION_SYNCHRONIZE};

    use crate::error::Error;
    use crate::ring_buffer::{SharedFrameHeader, MAGIC, VERSION};

    /// Opens an existing shared memory mapping for writing NV12 frames.
    ///
    /// Used by the main app to write frames into a mapping created by
    /// [`SharedMemoryOwner`](crate::owner::SharedMemoryOwner) in the COM DLL.
    pub struct SharedMemoryProducer {
        mapping_handle: HANDLE,
        event_handle: HANDLE,
        base_ptr: *mut u8,
        frame_size: u32,
        slot_count: u32,
    }

    // SAFETY: The shared memory region is process-shared and synchronised via
    // atomics + event signalling. The producer is the sole writer.
    unsafe impl Send for SharedMemoryProducer {}

    impl SharedMemoryProducer {
        /// Open an existing named shared memory region for writing.
        ///
        /// The mapping must already exist (created by `SharedMemoryOwner` in the
        /// COM DLL). Validates the header (magic + version) before returning.
        pub fn open(name: &str) -> Result<Self, Error> {
            let wide_name = to_wide(name);
            let event_name_str = format!("{name}_event");
            let wide_event_name = to_wide(&event_name_str);

            // SAFETY: Opening an existing named file mapping for writing.
            let mapping_handle =
                unsafe { OpenFileMappingW(FILE_MAP_WRITE.0, false, PCWSTR(wide_name.as_ptr()))? };

            // SAFETY: Mapping the region with write access. Size 0 = map entire region.
            let base_ptr =
                unsafe { MapViewOfFile(mapping_handle, FILE_MAP_WRITE, 0, 0, 0).Value as *mut u8 };

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

            let frame_size = header.frame_size;
            let slot_count = header.slot_count;

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

    impl Drop for SharedMemoryProducer {
        fn drop(&mut self) {
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
pub use platform::SharedMemoryProducer;
