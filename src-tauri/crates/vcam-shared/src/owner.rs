#[cfg(windows)]
mod platform {
    use std::ptr;
    use std::sync::atomic::Ordering;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{
        CloseHandle, GENERIC_ALL, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows::Win32::Security::{
        AddAccessAllowedAce, AllocateAndInitializeSid, GetLengthSid, InitializeAcl,
        InitializeSecurityDescriptor, SetSecurityDescriptorDacl, ACL, ACL_REVISION,
        PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR,
        SID_IDENTIFIER_AUTHORITY,
    };
    use windows::Win32::System::Memory::{
        CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_ALL_ACCESS, PAGE_READWRITE,
    };
    use windows::Win32::System::Threading::CreateEventW;

    use crate::error::Error;
    use crate::ring_buffer::{PixelFormat, SharedFrameHeader, MAGIC, VERSION};

    /// SECURITY_NT_AUTHORITY = {0,0,0,0,0,5}
    const SECURITY_NT_AUTHORITY: SID_IDENTIFIER_AUTHORITY = SID_IDENTIFIER_AUTHORITY {
        Value: [0, 0, 0, 0, 0, 5],
    };

    /// SECURITY_INTERACTIVE_RID (S-1-5-4) — all interactive logon users.
    const SECURITY_INTERACTIVE_RID: u32 = 4;

    /// SECURITY_LOCAL_SERVICE_RID (S-1-5-19) — LOCAL SERVICE account.
    const SECURITY_LOCAL_SERVICE_RID: u32 = 19;

    /// SECURITY_DESCRIPTOR_REVISION — version 1.
    const SECURITY_DESCRIPTOR_REVISION: u32 = 1;

    /// Creates and owns a `Global\` shared memory mapping with an explicit DACL.
    ///
    /// Used by the COM DLL (loaded by FrameServer as LOCAL SERVICE) to create the
    /// shared memory region. The DACL grants:
    /// - LOCAL SERVICE: `GENERIC_ALL`
    /// - Interactive users: `GENERIC_READ | GENERIC_WRITE`
    pub struct SharedMemoryOwner {
        mapping_handle: HANDLE,
        event_handle: HANDLE,
        base_ptr: *mut u8,
        _frame_size: u32,
        _slot_count: u32,
    }

    // SAFETY: The shared memory region is process-shared and synchronised via
    // atomics + event signalling. The owner is the sole creator.
    unsafe impl Send for SharedMemoryOwner {}
    unsafe impl Sync for SharedMemoryOwner {}

    impl SharedMemoryOwner {
        /// Create a new named shared memory region in the Global namespace.
        ///
        /// Sets a DACL granting:
        /// - LOCAL SERVICE: `GENERIC_ALL`
        /// - Interactive users: `GENERIC_READ | GENERIC_WRITE`
        pub fn new(name: &str, width: u32, height: u32, slot_count: u32) -> Result<Self, Error> {
            let total_size = SharedFrameHeader::total_size(width, height, slot_count);
            let frame_size = SharedFrameHeader::nv12_frame_size(width, height);

            let wide_name = to_wide(name);
            let event_name_str = format!("{name}_event");
            let wide_event_name = to_wide(&event_name_str);

            // Build security descriptor with DACL.
            let (sd, _acl_buf, _sid_ls, _sid_iu) = build_security_descriptor()?;

            let sa = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: &sd as *const SECURITY_DESCRIPTOR as *mut _,
                bInheritHandle: false.into(),
            };

            // SAFETY: Creating a named file mapping with explicit security.
            let mapping_handle = unsafe {
                CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    Some(&sa),
                    PAGE_READWRITE,
                    (total_size >> 32) as u32,
                    total_size as u32,
                    PCWSTR(wide_name.as_ptr()),
                )?
            };

            // SAFETY: Mapping the entire region as read-write.
            let base_ptr = unsafe {
                MapViewOfFile(mapping_handle, FILE_MAP_ALL_ACCESS, 0, 0, total_size).Value
                    as *mut u8
            };

            if base_ptr.is_null() {
                let err = windows::core::Error::from_thread();
                unsafe {
                    let _ = CloseHandle(mapping_handle);
                }
                return Err(err.into());
            }

            // SAFETY: Creating a named auto-reset event with the same DACL.
            let event_handle =
                unsafe { CreateEventW(Some(&sa), false, false, PCWSTR(wide_event_name.as_ptr()))? };

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
                mapping_handle,
                event_handle,
                base_ptr,
                _frame_size: frame_size,
                _slot_count: slot_count,
            })
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

        /// Access the shared frame header.
        pub fn header(&self) -> &SharedFrameHeader {
            // SAFETY: base_ptr is valid and points to a SharedFrameHeader.
            unsafe { &*(self.base_ptr as *const SharedFrameHeader) }
        }

        /// Wait for new frame signal with timeout (millis).
        ///
        /// Returns `true` if signalled, `false` on timeout.
        pub fn wait_frame(&self, timeout_ms: u32) -> bool {
            use windows::Win32::Foundation::WAIT_OBJECT_0;
            use windows::Win32::System::Threading::WaitForSingleObject;

            let result = unsafe { WaitForSingleObject(self.event_handle, timeout_ms) };
            result == WAIT_OBJECT_0
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
                let _ = CloseHandle(self.event_handle);
            }
        }
    }

    /// Build a SECURITY_DESCRIPTOR with a DACL granting LOCAL SERVICE full access
    /// and interactive users read+write access.
    ///
    /// Returns the SD, the ACL buffer (must outlive the SD), and the two SIDs
    /// (must outlive the ACL).
    fn build_security_descriptor() -> Result<(SECURITY_DESCRIPTOR, Vec<u8>, PSID, PSID), Error> {
        // Allocate SIDs.
        let sid_local_service = allocate_sid(SECURITY_LOCAL_SERVICE_RID)?;
        let sid_interactive = allocate_sid(SECURITY_INTERACTIVE_RID)?;

        // Calculate ACL size: base ACL header + 2 ACEs.
        let ace_size = |sid: PSID| -> usize {
            let sid_len = unsafe { GetLengthSid(sid) } as usize;
            // ACE header (4 bytes) + Mask (4 bytes) + SID length
            4 + 4 + sid_len
        };

        let acl_header_size = std::mem::size_of::<ACL>();
        let acl_size = acl_header_size + ace_size(sid_local_service) + ace_size(sid_interactive);

        let mut acl_buf = vec![0u8; acl_size];

        // SAFETY: Initialising ACL in our buffer.
        unsafe {
            InitializeAcl(
                acl_buf.as_mut_ptr() as *mut _,
                acl_size as u32,
                ACL_REVISION,
            )?;
        }

        // Add ACE: LOCAL SERVICE gets GENERIC_ALL.
        unsafe {
            AddAccessAllowedAce(
                acl_buf.as_mut_ptr() as *mut _,
                ACL_REVISION,
                GENERIC_ALL.0,
                sid_local_service,
            )?;
        }

        // Add ACE: Interactive users get GENERIC_READ | GENERIC_WRITE.
        unsafe {
            AddAccessAllowedAce(
                acl_buf.as_mut_ptr() as *mut _,
                ACL_REVISION,
                GENERIC_READ.0 | GENERIC_WRITE.0,
                sid_interactive,
            )?;
        }

        // Build the security descriptor.
        let mut sd = SECURITY_DESCRIPTOR::default();
        unsafe {
            InitializeSecurityDescriptor(
                PSECURITY_DESCRIPTOR(&mut sd as *mut _ as *mut _),
                SECURITY_DESCRIPTOR_REVISION,
            )?;
        }

        // Set the DACL on the descriptor.
        unsafe {
            SetSecurityDescriptorDacl(
                PSECURITY_DESCRIPTOR(&mut sd as *mut _ as *mut _),
                true,
                Some(acl_buf.as_ptr() as *const _),
                false,
            )?;
        }

        Ok((sd, acl_buf, sid_local_service, sid_interactive))
    }

    /// Allocate a well-known SID from NT AUTHORITY with a single sub-authority.
    fn allocate_sid(rid: u32) -> Result<PSID, Error> {
        let mut sid = PSID::default();
        // SAFETY: Allocating a well-known SID.
        unsafe {
            AllocateAndInitializeSid(
                &SECURITY_NT_AUTHORITY,
                1,
                rid,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                &mut sid,
            )?;
        }
        Ok(sid)
    }

    /// Encode a Rust `&str` as a null-terminated UTF-16 wide string.
    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

#[cfg(windows)]
pub use platform::SharedMemoryOwner;
