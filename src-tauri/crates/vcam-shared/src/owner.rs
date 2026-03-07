#[cfg(windows)]
mod platform {
    use std::path::{Path, PathBuf};
    use std::ptr;
    use std::sync::atomic::Ordering;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, DeleteFileW, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ,
    };
    use windows::Win32::System::Memory::{
        CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_ALL_ACCESS, PAGE_READWRITE,
    };

    use crate::error::Error;
    use crate::ring_buffer::{PixelFormat, SharedFrameHeader, MAGIC, VERSION};

    /// Creates and owns a file-backed shared memory region.
    ///
    /// Used by the main app to create a shared memory file that the COM DLL
    /// (loaded by FrameServer as LOCAL SERVICE) opens for reading. The file
    /// path is universal — it works across all sessions and kernel object
    /// namespaces.
    pub struct SharedMemoryOwner {
        file_handle: HANDLE,
        mapping_handle: HANDLE,
        base_ptr: *mut u8,
        file_path: PathBuf,
        frame_size: u32,
        slot_count: u32,
    }

    // SAFETY: The shared memory region is process-shared and synchronised via
    // atomics. The owner is the sole creator and writer.
    unsafe impl Send for SharedMemoryOwner {}
    unsafe impl Sync for SharedMemoryOwner {}

    impl SharedMemoryOwner {
        /// Create a file-backed shared memory region.
        ///
        /// Creates (or overwrites) the file at `file_path`, sets its size,
        /// creates a file mapping, and initialises the ring buffer header.
        pub fn new(
            file_path: &Path,
            width: u32,
            height: u32,
            slot_count: u32,
        ) -> Result<Self, Error> {
            let total_size = SharedFrameHeader::total_size(width, height, slot_count);
            let frame_size = SharedFrameHeader::nv12_frame_size(width, height);

            let wide_path = path_to_wide(file_path);

            // Always create a fresh file to avoid stale data from a previous
            // crash. CREATE_ALWAYS truncates any existing file.
            let file_handle = unsafe {
                CreateFileW(
                    PCWSTR(wide_path.as_ptr()),
                    (GENERIC_READ | GENERIC_WRITE).0,
                    FILE_SHARE_READ,
                    None,
                    CREATE_ALWAYS,
                    FILE_ATTRIBUTE_NORMAL,
                    None,
                )?
            };

            // Set the file size by seeking to the desired end and truncating.
            if let Err(e) = set_file_size(file_handle, total_size as u64) {
                unsafe {
                    let _ = CloseHandle(file_handle);
                }
                return Err(e);
            }

            // Create a file mapping over the backing file.
            let mapping_handle = match unsafe {
                CreateFileMappingW(
                    file_handle,
                    None,
                    PAGE_READWRITE,
                    (total_size >> 32) as u32,
                    total_size as u32,
                    PCWSTR::null(),
                )
            } {
                Ok(h) => h,
                Err(e) => {
                    unsafe {
                        let _ = CloseHandle(file_handle);
                    }
                    return Err(e.into());
                }
            };

            // Map the entire file into memory.
            let base_ptr = unsafe {
                MapViewOfFile(mapping_handle, FILE_MAP_ALL_ACCESS, 0, 0, total_size).Value
                    as *mut u8
            };

            if base_ptr.is_null() {
                let err = windows::core::Error::from_thread();
                unsafe {
                    let _ = CloseHandle(mapping_handle);
                    let _ = CloseHandle(file_handle);
                }
                return Err(err.into());
            }

            // Zero the entire region.
            // SAFETY: base_ptr is valid for total_size bytes.
            unsafe { ptr::write_bytes(base_ptr, 0, total_size) };

            // Initialise the header.
            // SAFETY: base_ptr is valid and aligned to 64.
            let header = unsafe { &mut *(base_ptr as *mut SharedFrameHeader) };
            header.magic = MAGIC;
            header.version = VERSION;
            header.width = width;
            header.height = height;
            header.format = PixelFormat::Nv12 as u32;
            header.frame_size = frame_size;
            header.slot_count = slot_count;

            Ok(Self {
                file_handle,
                mapping_handle,
                base_ptr,
                file_path: file_path.to_owned(),
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
        }

        /// Access the shared frame header.
        pub fn header(&self) -> &SharedFrameHeader {
            // SAFETY: base_ptr is valid and points to a SharedFrameHeader.
            unsafe { &*(self.base_ptr as *const SharedFrameHeader) }
        }

        /// Read the latest frame from the ring buffer.
        ///
        /// Returns `None` if no frames have been written (sequence == 0).
        pub fn read_frame(&self) -> Option<&[u8]> {
            let header = self.header();
            let seq = header.sequence.load(Ordering::Acquire);
            if seq == 0 {
                return None;
            }

            let write_idx = header.write_index.load(Ordering::Acquire);
            let slot = write_idx.wrapping_sub(1) % header.slot_count;
            let offset = SharedFrameHeader::slot_offset(slot, header.frame_size);

            // SAFETY: offset and frame_size are within bounds of the mapped region.
            Some(unsafe {
                std::slice::from_raw_parts(self.base_ptr.add(offset), header.frame_size as usize)
            })
        }

        /// Current sequence number.
        pub fn sequence(&self) -> u64 {
            self.header().sequence.load(Ordering::Acquire)
        }

        /// Current write index (raw, not wrapped).
        pub fn write_index(&self) -> u32 {
            self.header().write_index.load(Ordering::Acquire)
        }
    }

    impl Drop for SharedMemoryOwner {
        fn drop(&mut self) {
            unsafe {
                let view = windows::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.base_ptr as *mut _,
                };
                let _ = UnmapViewOfFile(view);
                let _ = CloseHandle(self.mapping_handle);
                let _ = CloseHandle(self.file_handle);

                // Delete the backing file.
                let wide_path = path_to_wide(&self.file_path);
                let _ = DeleteFileW(PCWSTR(wide_path.as_ptr()));
            }
        }
    }

    /// Set the file size using `SetFilePointerEx` + `SetEndOfFile`.
    fn set_file_size(handle: HANDLE, size: u64) -> Result<(), Error> {
        use windows::Win32::Storage::FileSystem::{SetEndOfFile, SetFilePointerEx, FILE_BEGIN};
        let mut new_pos = 0i64;
        unsafe {
            SetFilePointerEx(handle, size as i64, Some(&mut new_pos), FILE_BEGIN)?;
            SetEndOfFile(handle)?;
            // Seek back to the beginning.
            SetFilePointerEx(handle, 0, None, FILE_BEGIN)?;
        }
        Ok(())
    }

    /// Convert a `Path` to a null-terminated UTF-16 wide string.
    fn path_to_wide(path: &Path) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
}

#[cfg(windows)]
pub use platform::SharedMemoryOwner;
