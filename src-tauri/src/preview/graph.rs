// DirectShow filter graph for camera frame capture.
//
// Builds a Source -> SampleGrabber -> NullRenderer pipeline and delivers
// raw RGB24 frames via a callback into the shared FrameBuffer.

#[cfg(target_os = "windows")]
pub mod directshow {
    use parking_lot::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use tracing::{debug, error, info, trace, warn};
    use windows::core::{Interface, GUID, HRESULT};
    use windows::Win32::Media::DirectShow::{
        IAMStreamConfig, IBaseFilter, ICreateDevEnum, IFilterGraph2, IGraphBuilder, IMediaControl,
        IMediaFilter, IPin,
    };
    use windows::Win32::Media::MediaFoundation::VIDEOINFOHEADER;
    use windows::Win32::Media::MediaFoundation::{
        CLSID_SystemDeviceEnum, CLSID_VideoInputDeviceCategory,
    };
    use windows::Win32::System::Com::StructuredStorage::IPropertyBag;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_MULTITHREADED,
    };
    use windows::Win32::System::Variant::VARIANT;

    use crate::diagnostics::stats::DiagnosticStats;
    use crate::preview::capture::{Frame, FrameBuffer};
    use crate::preview::graph::{
        convert_bgr_bottom_up_to_rgb, convert_nv12_to_rgb, convert_yuy2_to_rgb,
        is_obs_virtual_camera,
    };

    // --- Manually defined types not in windows-rs metadata ---

    /// AM_MEDIA_TYPE — DirectShow media type descriptor.
    /// Layout matches the C struct from dshow.h / strmif.h.
    #[repr(C)]
    #[derive(Clone)]
    pub struct AmMediaType {
        pub major_type: GUID,
        pub sub_type: GUID,
        pub fixed_size_samples: i32,
        pub temporal_compression: i32,
        pub sample_size: u32,
        pub format_type: GUID,
        pub unk: usize, // IUnknown* (unused)
        pub cb_format: u32,
        pub pb_format: *mut u8,
    }

    impl Default for AmMediaType {
        fn default() -> Self {
            Self {
                major_type: GUID::zeroed(),
                sub_type: GUID::zeroed(),
                fixed_size_samples: 0,
                temporal_compression: 0,
                sample_size: 0,
                format_type: GUID::zeroed(),
                unk: 0,
                cb_format: 0,
                pb_format: std::ptr::null_mut(),
            }
        }
    }

    // --- COM GUIDs ---

    // IUnknown: {00000000-0000-0000-C000-000000000046}
    const IID_IUNKNOWN: GUID = GUID::from_u128(0x00000000_0000_0000_C000_000000000046);

    // ISampleGrabberCB: {0579154A-2B53-4994-B0D0-E773148EFF85}
    const IID_ISAMPLEGRABBER_CB: GUID = GUID::from_u128(0x0579154A_2B53_4994_B0D0_E773148EFF85);

    // ISampleGrabber: {6B652FFF-11FE-4FCE-92AD-0266B5D7C78F}
    const IID_ISAMPLEGRABBER: GUID = GUID::from_u128(0x6B652FFF_11FE_4FCE_92AD_0266B5D7C78F);

    // CLSID_SampleGrabber: {C1F400A0-3F08-11D3-9F0B-006008039E37}
    const CLSID_SAMPLE_GRABBER: GUID = GUID::from_u128(0xC1F400A0_3F08_11D3_9F0B_006008039E37);

    // CLSID_NullRenderer: {C1F400A4-3F08-11D3-9F0B-006008039E37}
    const CLSID_NULL_RENDERER: GUID = GUID::from_u128(0xC1F400A4_3F08_11D3_9F0B_006008039E37);

    // CLSID_FilterGraph: {E436EBB3-524F-11CE-9F53-0020AF0BA770}
    const CLSID_FILTER_GRAPH: GUID = GUID::from_u128(0xE436EBB3_524F_11CE_9F53_0020AF0BA770);

    // MEDIATYPE_Video: {73646976-0000-0010-8000-00AA00389B71}
    const MEDIATYPE_VIDEO: GUID = GUID::from_u128(0x73646976_0000_0010_8000_00AA00389B71);

    // MEDIASUBTYPE_RGB24: {e436eb7d-524f-11ce-9f53-0020af0ba770}
    const MEDIASUBTYPE_RGB24: GUID = GUID::from_u128(0xe436eb7d_524f_11ce_9f53_0020af0ba770);

    // MEDIASUBTYPE_YUY2: {32595559-0000-0010-8000-00AA00389B71}
    const MEDIASUBTYPE_YUY2: GUID = GUID::from_u128(0x32595559_0000_0010_8000_00AA00389B71);

    // MEDIASUBTYPE_NV12: {3231564E-0000-0010-8000-00AA00389B71}
    const MEDIASUBTYPE_NV12: GUID = GUID::from_u128(0x3231564E_0000_0010_8000_00AA00389B71);

    // FORMAT_VideoInfo: {05589F80-C356-11CE-BF01-00AA0055595A}
    const FORMAT_VIDEOINFO: GUID = GUID::from_u128(0x05589f80_c356_11ce_bf01_00aa0055595a);

    // --- ISampleGrabber raw COM interface ---

    /// ISampleGrabber vtable layout matching the C++ interface.
    #[repr(C)]
    struct ISampleGrabberVtbl {
        // IUnknown (3 methods)
        query_interface: unsafe extern "system" fn(
            *mut core::ffi::c_void,
            *const GUID,
            *mut *mut core::ffi::c_void,
        ) -> HRESULT,
        add_ref: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
        release: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
        // ISampleGrabber methods
        set_one_shot: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> HRESULT,
        set_media_type:
            unsafe extern "system" fn(*mut core::ffi::c_void, *const AmMediaType) -> HRESULT,
        get_connected_media_type:
            unsafe extern "system" fn(*mut core::ffi::c_void, *mut AmMediaType) -> HRESULT,
        set_buffer_samples: unsafe extern "system" fn(*mut core::ffi::c_void, i32) -> HRESULT,
        get_current_buffer:
            unsafe extern "system" fn(*mut core::ffi::c_void, *mut i32, *mut u8) -> HRESULT,
        get_current_sample: unsafe extern "system" fn(
            *mut core::ffi::c_void,
            *mut *mut core::ffi::c_void,
        ) -> HRESULT,
        set_callback: unsafe extern "system" fn(
            *mut core::ffi::c_void,
            *mut core::ffi::c_void,
            i32,
        ) -> HRESULT,
    }

    /// Wrapper for the raw ISampleGrabber COM pointer.
    struct SampleGrabber {
        ptr: *mut core::ffi::c_void,
    }

    impl SampleGrabber {
        /// Query ISampleGrabber from an IBaseFilter via raw QueryInterface.
        unsafe fn from_filter(filter: &IBaseFilter) -> Option<Self> {
            // Get the raw IUnknown pointer from the IBaseFilter
            let unk_ptr = std::mem::transmute_copy::<IBaseFilter, *mut core::ffi::c_void>(filter);
            if unk_ptr.is_null() {
                return None;
            }

            // Read the vtable pointer and call QueryInterface
            let vtbl = *(unk_ptr as *const *const usize);
            let qi: unsafe extern "system" fn(
                *mut core::ffi::c_void,
                *const GUID,
                *mut *mut core::ffi::c_void,
            ) -> HRESULT = std::mem::transmute(*vtbl);

            let mut result: *mut core::ffi::c_void = std::ptr::null_mut();
            let hr = qi(unk_ptr, &IID_ISAMPLEGRABBER, &mut result);

            if hr.is_ok() && !result.is_null() {
                Some(Self { ptr: result })
            } else {
                None
            }
        }

        unsafe fn vtbl(&self) -> &ISampleGrabberVtbl {
            &*(*(self.ptr as *const *const ISampleGrabberVtbl))
        }

        unsafe fn set_media_type(&self, mt: &AmMediaType) -> HRESULT {
            (self.vtbl().set_media_type)(self.ptr, mt)
        }

        unsafe fn set_one_shot(&self, one_shot: bool) -> HRESULT {
            (self.vtbl().set_one_shot)(self.ptr, i32::from(one_shot))
        }

        unsafe fn set_buffer_samples(&self, buffer: bool) -> HRESULT {
            (self.vtbl().set_buffer_samples)(self.ptr, i32::from(buffer))
        }

        unsafe fn set_callback(&self, callback: *mut core::ffi::c_void, which: i32) -> HRESULT {
            (self.vtbl().set_callback)(self.ptr, callback, which)
        }

        unsafe fn get_connected_media_type(&self, mt: &mut AmMediaType) -> HRESULT {
            (self.vtbl().get_connected_media_type)(self.ptr, mt)
        }
    }

    impl Drop for SampleGrabber {
        fn drop(&mut self) {
            unsafe {
                let vtbl = self.vtbl();
                (vtbl.release)(self.ptr);
            }
        }
    }

    // --- ISampleGrabberCB implementation ---

    /// ISampleGrabberCB vtable layout.
    #[repr(C)]
    struct ISampleGrabberCBVtbl {
        query_interface: unsafe extern "system" fn(
            *mut core::ffi::c_void,
            *const GUID,
            *mut *mut core::ffi::c_void,
        ) -> HRESULT,
        add_ref: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
        release: unsafe extern "system" fn(*mut core::ffi::c_void) -> u32,
        sample_cb: unsafe extern "system" fn(
            *mut core::ffi::c_void,
            f64,
            *mut core::ffi::c_void,
        ) -> HRESULT,
        buffer_cb: unsafe extern "system" fn(*mut core::ffi::c_void, f64, *mut u8, i32) -> HRESULT,
    }

    /// COM object data for our ISampleGrabberCB implementation.
    #[repr(C)]
    struct FrameCallbackData {
        vtbl: *const ISampleGrabberCBVtbl,
        ref_count: std::sync::atomic::AtomicU32,
        buffer: Arc<FrameBuffer>,
        width: u32,
        height: u32,
        sub_type: GUID,
        running: Arc<AtomicBool>,
        stats: Arc<Mutex<DiagnosticStats>>,
    }

    static FRAME_CALLBACK_VTBL: ISampleGrabberCBVtbl = ISampleGrabberCBVtbl {
        query_interface: frame_cb_query_interface,
        add_ref: frame_cb_add_ref,
        release: frame_cb_release,
        sample_cb: frame_cb_sample_cb,
        buffer_cb: frame_cb_buffer_cb,
    };

    unsafe extern "system" fn frame_cb_query_interface(
        this: *mut core::ffi::c_void,
        riid: *const GUID,
        ppv: *mut *mut core::ffi::c_void,
    ) -> HRESULT {
        let iid = &*riid;
        if *iid == IID_ISAMPLEGRABBER_CB || *iid == IID_IUNKNOWN {
            *ppv = this;
            frame_cb_add_ref(this);
            HRESULT(0) // S_OK
        } else {
            *ppv = std::ptr::null_mut();
            HRESULT(0x80004002u32 as i32) // E_NOINTERFACE
        }
    }

    unsafe extern "system" fn frame_cb_add_ref(this: *mut core::ffi::c_void) -> u32 {
        let data = &*(this as *const FrameCallbackData);
        data.ref_count
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1)
    }

    unsafe extern "system" fn frame_cb_release(this: *mut core::ffi::c_void) -> u32 {
        let data = &*(this as *const FrameCallbackData);
        let prev = data.ref_count.fetch_sub(1, Ordering::Relaxed);
        if prev == 1 {
            drop(Box::from_raw(this as *mut FrameCallbackData));
            return 0;
        }
        prev - 1
    }

    unsafe extern "system" fn frame_cb_sample_cb(
        _this: *mut core::ffi::c_void,
        _sample_time: f64,
        _sample: *mut core::ffi::c_void,
    ) -> HRESULT {
        // We use BufferCB (mode 1), not SampleCB
        HRESULT(0)
    }

    unsafe extern "system" fn frame_cb_buffer_cb(
        this: *mut core::ffi::c_void,
        sample_time: f64,
        buffer: *mut u8,
        buffer_len: i32,
    ) -> HRESULT {
        let data = &*(this as *const FrameCallbackData);

        if !data.running.load(Ordering::Relaxed) {
            return HRESULT(0);
        }

        if buffer.is_null() || buffer_len <= 0 {
            warn!("frame callback received null/empty buffer (len={buffer_len})");
            data.stats.lock().record_drop();
            return HRESULT(0);
        }

        let len = buffer_len as usize;
        let raw = std::slice::from_raw_parts(buffer, len);
        let timestamp_us = (sample_time * 1_000_000.0) as u64;

        let width = data.width as usize;
        let height = data.height as usize;

        let rgb = if data.sub_type == MEDIASUBTYPE_RGB24 {
            // DirectShow delivers BGR24 bottom-up; convert to RGB24 top-down
            let expected = width * 3 * height;
            if len < expected {
                warn!(
                    "frame size mismatch: got {len} bytes, expected {expected} ({width}x{height})"
                );
                data.stats.lock().record_drop();
                return HRESULT(0);
            }
            convert_bgr_bottom_up_to_rgb(raw, width, height)
        } else if data.sub_type == MEDIASUBTYPE_YUY2 {
            let expected = width * height * 2;
            if len < expected {
                warn!(
                    "YUY2 frame size mismatch: got {len} bytes, expected {expected} ({width}x{height})"
                );
                data.stats.lock().record_drop();
                return HRESULT(0);
            }
            convert_yuy2_to_rgb(raw, width, height)
        } else if data.sub_type == MEDIASUBTYPE_NV12 {
            let expected = width * height * 3 / 2;
            if len < expected {
                warn!(
                    "NV12 frame size mismatch: got {len} bytes, expected {expected} ({width}x{height})"
                );
                data.stats.lock().record_drop();
                return HRESULT(0);
            }
            trace!(target: "preview::graph", "Converting NV12 frame to RGB");
            convert_nv12_to_rgb(raw, width, height)
        } else {
            // Unsupported format — drop the frame to prevent panics in
            // compress_jpeg which expects RGB24 (width*height*3 bytes).
            warn!(
                "unsupported sub_type {:?}, dropping frame ({len} bytes)",
                data.sub_type
            );
            data.stats.lock().record_drop();
            return HRESULT(0);
        };

        let frame_bytes = rgb.len();
        data.buffer.push(Frame {
            data: rgb,
            width: data.width,
            height: data.height,
            timestamp_us,
        });
        data.stats.lock().record_frame(frame_bytes, timestamp_us);

        // Log early frames at debug level to confirm delivery
        let snapshot = data.stats.lock().snapshot();
        if snapshot.frame_count <= 3 {
            debug!(
                "frame #{} delivered: {width}x{height}, {frame_bytes} bytes, \
                 sub_type={:?}",
                snapshot.frame_count, data.sub_type
            );
        }

        HRESULT(0)
    }

    /// Create a new ISampleGrabberCB implementation that pushes frames
    /// into the buffer.
    fn create_frame_callback(
        buffer: Arc<FrameBuffer>,
        width: u32,
        height: u32,
        sub_type: GUID,
        running: Arc<AtomicBool>,
        stats: Arc<Mutex<DiagnosticStats>>,
    ) -> *mut core::ffi::c_void {
        let data = Box::new(FrameCallbackData {
            vtbl: &FRAME_CALLBACK_VTBL,
            ref_count: std::sync::atomic::AtomicU32::new(1),
            buffer,
            width,
            height,
            sub_type,
            running,
            stats,
        });
        Box::into_raw(data) as *mut core::ffi::c_void
    }

    /// COM guard for per-thread initialisation.
    struct ComGuard;

    impl ComGuard {
        fn init() -> Result<Self, String> {
            unsafe {
                let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
                if hr.is_err() {
                    return Err(format!("CoInitializeEx failed: {hr:?}"));
                }
            }
            Ok(Self)
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }

    /// Configure the source filter's output pin to request a specific resolution.
    ///
    /// Enumerates the pin's stream capabilities via IAMStreamConfig, picks the
    /// best match for the requested width/height, and calls SetFormat. If no
    /// suitable format is found or the pin doesn't support IAMStreamConfig, the
    /// function logs a warning and returns without error — the graph will fall
    /// back to the camera's default resolution.
    unsafe fn configure_source_resolution(source: &IBaseFilter, width: u32, height: u32) {
        use windows::Win32::Media::MediaFoundation::FORMAT_VideoInfo;

        let pin_enum = match source.EnumPins() {
            Ok(e) => e,
            Err(e) => {
                warn!("EnumPins failed when configuring resolution: {e}");
                return;
            }
        };

        let mut pin_array = [None; 1];

        // Find the first output pin with IAMStreamConfig
        loop {
            let hr = pin_enum.Next(&mut pin_array, None);
            if hr.is_err() {
                break;
            }

            let Some(pin) = pin_array[0].take() else {
                break;
            };

            let dir = match pin.QueryDirection() {
                Ok(d) => d,
                Err(_) => continue,
            };

            // PINDIR_OUTPUT = 1
            if dir.0 != 1 {
                continue;
            }

            let Ok(stream_config) = pin.cast::<IAMStreamConfig>() else {
                continue;
            };

            let mut count = 0i32;
            let mut size = 0i32;
            if stream_config
                .GetNumberOfCapabilities(&mut count, &mut size)
                .is_err()
            {
                continue;
            }

            let target_pixels = (width as u64) * (height as u64);
            let mut best_index: Option<i32> = None;
            let mut best_diff: u64 = u64::MAX;

            for i in 0..count {
                let mut scc = vec![0u8; size as usize];
                let mut mt_ptr = std::ptr::null_mut();
                if stream_config
                    .GetStreamCaps(i, &mut mt_ptr, scc.as_mut_ptr())
                    .is_err()
                {
                    continue;
                }

                if mt_ptr.is_null() {
                    continue;
                }

                let mt_ref = &*mt_ptr;
                let mut cap_w = 0u32;
                let mut cap_h = 0u32;

                if mt_ref.formattype == FORMAT_VideoInfo
                    && !mt_ref.pbFormat.is_null()
                    && mt_ref.cbFormat as usize >= std::mem::size_of::<VIDEOINFOHEADER>()
                {
                    let vih: &VIDEOINFOHEADER = &*(mt_ref.pbFormat as *const VIDEOINFOHEADER);
                    cap_w = vih.bmiHeader.biWidth as u32;
                    cap_h = vih.bmiHeader.biHeight.unsigned_abs();
                }

                // Free the AM_MEDIA_TYPE
                if !mt_ref.pbFormat.is_null() {
                    windows::Win32::System::Com::CoTaskMemFree(Some(mt_ref.pbFormat.cast()));
                }
                windows::Win32::System::Com::CoTaskMemFree(Some(
                    (mt_ptr as *mut core::ffi::c_void).cast(),
                ));

                if cap_w == 0 || cap_h == 0 {
                    continue;
                }

                let cap_pixels = (cap_w as u64) * (cap_h as u64);
                let diff = cap_pixels.abs_diff(target_pixels);
                if diff < best_diff {
                    best_diff = diff;
                    best_index = Some(i);
                }
            }

            if let Some(idx) = best_index {
                let mut scc = vec![0u8; size as usize];
                let mut mt_ptr = std::ptr::null_mut();
                if stream_config
                    .GetStreamCaps(idx, &mut mt_ptr, scc.as_mut_ptr())
                    .is_ok()
                    && !mt_ptr.is_null()
                {
                    let mt_ref = &*mt_ptr;
                    let mut fmt_w = 0u32;
                    let mut fmt_h = 0u32;
                    if mt_ref.formattype == FORMAT_VideoInfo
                        && !mt_ref.pbFormat.is_null()
                        && mt_ref.cbFormat as usize >= std::mem::size_of::<VIDEOINFOHEADER>()
                    {
                        let vih: &VIDEOINFOHEADER = &*(mt_ref.pbFormat as *const VIDEOINFOHEADER);
                        fmt_w = vih.bmiHeader.biWidth as u32;
                        fmt_h = vih.bmiHeader.biHeight.unsigned_abs();
                    }

                    match stream_config.SetFormat(mt_ptr) {
                        Ok(()) => {
                            info!(
                                "configured source resolution to {fmt_w}x{fmt_h} \
                                 (requested {width}x{height})"
                            );
                        }
                        Err(e) => {
                            warn!(
                                "SetFormat({fmt_w}x{fmt_h}) failed: {e}, \
                                 falling back to camera default"
                            );
                        }
                    }

                    // Free the AM_MEDIA_TYPE
                    if !mt_ref.pbFormat.is_null() {
                        windows::Win32::System::Com::CoTaskMemFree(Some(mt_ref.pbFormat.cast()));
                    }
                    windows::Win32::System::Com::CoTaskMemFree(Some(
                        (mt_ptr as *mut core::ffi::c_void).cast(),
                    ));
                }
            } else {
                warn!("no stream capabilities found, using camera default resolution");
            }

            return;
        }

        warn!("no output pin with IAMStreamConfig found, using camera default resolution");
    }

    /// Force NV12 on the source pin via IAMStreamConfig.
    ///
    /// Enumerates stream capabilities to find an NV12 format and calls
    /// SetFormat with the full media type (including proper format_type,
    /// width, height). This is how OpenCV handles OBS Virtual Camera —
    /// forcing the entire pipeline to NV12 from the source rather than
    /// relying on SampleGrabber hints.
    unsafe fn force_nv12_on_source_pin(source: &IBaseFilter) -> Result<(), String> {
        use windows::Win32::Media::MediaFoundation::FORMAT_VideoInfo;

        let pin_enum = source
            .EnumPins()
            .map_err(|e| format!("EnumPins failed: {e}"))?;

        let mut pin_array = [None; 1];

        loop {
            let hr = pin_enum.Next(&mut pin_array, None);
            if hr.is_err() {
                break;
            }

            let Some(pin) = pin_array[0].take() else {
                break;
            };

            let dir = match pin.QueryDirection() {
                Ok(d) => d,
                Err(_) => continue,
            };

            // PINDIR_OUTPUT = 1
            if dir.0 != 1 {
                continue;
            }

            let Ok(stream_config) = pin.cast::<IAMStreamConfig>() else {
                continue;
            };

            let mut count = 0i32;
            let mut size = 0i32;
            if stream_config
                .GetNumberOfCapabilities(&mut count, &mut size)
                .is_err()
            {
                continue;
            }

            // Find an NV12 capability
            for i in 0..count {
                let mut scc = vec![0u8; size as usize];
                let mut mt_ptr = std::ptr::null_mut();
                if stream_config
                    .GetStreamCaps(i, &mut mt_ptr, scc.as_mut_ptr())
                    .is_err()
                {
                    continue;
                }

                if mt_ptr.is_null() {
                    continue;
                }

                let mt_ref = &*mt_ptr;

                let is_nv12 =
                    mt_ref.subtype == MEDIASUBTYPE_NV12 && mt_ref.formattype == FORMAT_VideoInfo;

                if is_nv12 {
                    // Extract dimensions for logging
                    let mut w = 0u32;
                    let mut h = 0u32;
                    if !mt_ref.pbFormat.is_null()
                        && mt_ref.cbFormat as usize >= std::mem::size_of::<VIDEOINFOHEADER>()
                    {
                        let vih: &VIDEOINFOHEADER = &*(mt_ref.pbFormat as *const VIDEOINFOHEADER);
                        w = vih.bmiHeader.biWidth as u32;
                        h = vih.bmiHeader.biHeight.unsigned_abs();
                    }

                    match stream_config.SetFormat(mt_ptr) {
                        Ok(()) => {
                            info!("set source pin to NV12 successfully ({w}x{h})");

                            // Free the AM_MEDIA_TYPE
                            if !mt_ref.pbFormat.is_null() {
                                windows::Win32::System::Com::CoTaskMemFree(Some(
                                    mt_ref.pbFormat.cast(),
                                ));
                            }
                            windows::Win32::System::Com::CoTaskMemFree(Some(
                                (mt_ptr as *mut core::ffi::c_void).cast(),
                            ));

                            return Ok(());
                        }
                        Err(e) => {
                            warn!("SetFormat(NV12, cap {i}) failed: {e}, trying next capability");
                        }
                    }
                }

                // Free the AM_MEDIA_TYPE
                if !mt_ref.pbFormat.is_null() {
                    windows::Win32::System::Com::CoTaskMemFree(Some(mt_ref.pbFormat.cast()));
                }
                windows::Win32::System::Com::CoTaskMemFree(Some(
                    (mt_ptr as *mut core::ffi::c_void).cast(),
                ));
            }

            return Err("no NV12 capability found on source pin".to_string());
        }

        Err("no output pin with IAMStreamConfig found".to_string())
    }

    /// Find a DirectShow source filter by device path, falling back to
    /// friendly name for virtual cameras that lack a DevicePath property.
    unsafe fn find_source_filter(
        device_path: &str,
        friendly_name: &str,
    ) -> Result<IBaseFilter, String> {
        use windows::Win32::System::Com::IMoniker;

        let dev_enum: ICreateDevEnum =
            CoCreateInstance(&CLSID_SystemDeviceEnum, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| format!("CoCreateInstance(SystemDeviceEnum) failed: {e}"))?;

        let mut enum_moniker = None;
        dev_enum
            .CreateClassEnumerator(&CLSID_VideoInputDeviceCategory, &mut enum_moniker, 0)
            .map_err(|e| format!("CreateClassEnumerator failed: {e}"))?;

        let Some(enum_moniker) = enum_moniker else {
            return Err("no video devices found".to_string());
        };

        let mut moniker_array = [None; 1];

        loop {
            let hr = enum_moniker.Next(&mut moniker_array, None);
            if hr.is_err() {
                break;
            }

            let Some(moniker) = moniker_array[0].take() else {
                break;
            };

            let bag: IPropertyBag = match moniker.BindToStorage(
                None::<&windows::Win32::System::Com::IBindCtx>,
                None::<&IMoniker>,
            ) {
                Ok(b) => b,
                Err(_) => continue,
            };

            let path = read_property_string(&bag, "DevicePath").unwrap_or_default();
            let name = read_property_string(&bag, "FriendlyName").unwrap_or_default();

            // Match by device path first, fall back to friendly name for
            // virtual cameras that may not have a DevicePath property.
            let matched = if !device_path.is_empty() && path == device_path {
                true
            } else if !friendly_name.is_empty() && path.is_empty() && name == friendly_name {
                info!("matched virtual camera by FriendlyName: {name}");
                true
            } else {
                false
            };

            if !matched {
                continue;
            }

            let filter: IBaseFilter = moniker
                .BindToObject(
                    None::<&windows::Win32::System::Com::IBindCtx>,
                    None::<&IMoniker>,
                )
                .map_err(|e| format!("BindToObject failed: {e}"))?;

            return Ok(filter);
        }

        Err(format!(
            "device not found: path={device_path}, name={friendly_name}"
        ))
    }

    /// Read a string property from an IPropertyBag.
    unsafe fn read_property_string(bag: &IPropertyBag, name: &str) -> Option<String> {
        use windows::core::BSTR;

        let prop_name = BSTR::from(name);
        let mut variant = VARIANT::default();

        bag.Read(
            windows::core::PCWSTR(prop_name.as_ptr()),
            &mut variant,
            None,
        )
        .ok()?;

        let bstr_ptr: *const *const u16 = std::ptr::addr_of!(variant).cast::<u8>().add(8).cast();
        let raw_bstr = *bstr_ptr;
        if raw_bstr.is_null() {
            return None;
        }

        let len_ptr = (raw_bstr as *const u8).sub(4) as *const u32;
        let byte_len = *len_ptr;
        let char_len = byte_len as usize / 2;

        let slice = std::slice::from_raw_parts(raw_bstr, char_len);
        Some(String::from_utf16_lossy(slice))
    }

    /// Build and run the DirectShow capture graph.
    ///
    /// This function blocks the calling thread, running the filter graph
    /// until `running` is set to false. Should be called from a dedicated
    /// capture thread.
    pub fn run_capture_graph(
        device_path: &str,
        friendly_name: &str,
        width: u32,
        height: u32,
        buffer: Arc<FrameBuffer>,
        running: Arc<AtomicBool>,
        stats: Arc<Mutex<DiagnosticStats>>,
    ) -> Result<(), String> {
        unsafe {
            let _guard = ComGuard::init()?;

            // 1. Create filter graph
            info!("creating capture graph for {device_path}");
            let graph: IGraphBuilder =
                CoCreateInstance(&CLSID_FILTER_GRAPH, None, CLSCTX_INPROC_SERVER).map_err(|e| {
                    error!("failed to create filter graph: {e}");
                    format!("failed to create filter graph: {e}")
                })?;

            let graph2: IFilterGraph2 = graph.cast().map_err(|e| {
                error!("failed to get IFilterGraph2: {e}");
                format!("failed to get IFilterGraph2: {e}")
            })?;

            // 2. Find and add source filter
            let source = find_source_filter(device_path, friendly_name).map_err(|e| {
                error!("failed to find source filter for {device_path}: {e}");
                e
            })?;
            graph2
                .AddFilter(&source, windows::core::w!("Source"))
                .map_err(|e| {
                    error!("failed to add source filter: {e}");
                    format!("failed to add source filter: {e}")
                })?;

            // 2b. Configure the source output pin resolution via IAMStreamConfig.
            //     This requests the camera to output at the desired resolution
            //     rather than defaulting to its maximum (e.g. 1920x1080).
            //     Skip for OBS Virtual Camera — its SetFormat returns S_OK for
            //     anything but silently breaks the pipeline. NV12 is forced
            //     separately in step 6.
            info!("checking camera: friendly_name={friendly_name:?}");
            if width > 0 && height > 0 && !is_obs_virtual_camera(friendly_name) {
                configure_source_resolution(&source, width, height);
            }

            // 3. Create and add SampleGrabber filter
            let grabber_filter: IBaseFilter =
                CoCreateInstance(&CLSID_SAMPLE_GRABBER, None, CLSCTX_INPROC_SERVER).map_err(
                    |e| {
                        error!("failed to create SampleGrabber: {e}");
                        format!("failed to create SampleGrabber: {e}")
                    },
                )?;

            graph2
                .AddFilter(&grabber_filter, windows::core::w!("SampleGrabber"))
                .map_err(|e| {
                    error!("failed to add SampleGrabber filter: {e}");
                    format!("failed to add SampleGrabber filter: {e}")
                })?;

            // 4. Configure SampleGrabber
            let grabber = SampleGrabber::from_filter(&grabber_filter).ok_or_else(|| {
                error!("failed to query ISampleGrabber from SampleGrabber filter");
                "failed to query ISampleGrabber".to_string()
            })?;

            let hr = grabber.set_one_shot(false);
            if hr.is_err() {
                error!("SetOneShot failed: {hr:?}");
                return Err(format!("SetOneShot failed: {hr:?}"));
            }

            let hr = grabber.set_buffer_samples(false);
            if hr.is_err() {
                error!("SetBufferSamples failed: {hr:?}");
                return Err(format!("SetBufferSamples failed: {hr:?}"));
            }

            // 5. Create and add NullRenderer
            let null_renderer: IBaseFilter =
                CoCreateInstance(&CLSID_NULL_RENDERER, None, CLSCTX_INPROC_SERVER).map_err(
                    |e| {
                        error!("failed to create NullRenderer: {e}");
                        format!("failed to create NullRenderer: {e}")
                    },
                )?;

            graph2
                .AddFilter(&null_renderer, windows::core::w!("NullRenderer"))
                .map_err(|e| {
                    error!("failed to add NullRenderer: {e}");
                    format!("failed to add NullRenderer: {e}")
                })?;

            // 6. Connect: Source -> SampleGrabber -> NullRenderer
            //    OBS Virtual Camera lies about supporting RGB24 (SetFormat returns
            //    S_OK for anything) but only delivers NV12 frames reliably. When
            //    detected, request NV12 directly to avoid the 1-frame issue.
            //    For all other cameras, try RGB24 first then fall back to any subtype.
            let source_out = find_unconnected_pin(&source, 1)?;
            let grabber_in = find_unconnected_pin(&grabber_filter, 0)?;

            if is_obs_virtual_camera(friendly_name) {
                info!(
                    "OBS Virtual Camera detected — skipping RGB24 SetFormat, \
                     forcing NV12 on source pin"
                );

                // Force NV12 on the source pin via IAMStreamConfig so the
                // entire pipeline negotiates NV12 from the start. This is
                // how OpenCV handles OBS — setting the grabber alone is
                // just a hint that DirectShow may ignore.
                if let Err(e) = force_nv12_on_source_pin(&source) {
                    warn!(
                        "could not force NV12 on source pin: {e}, attempting graph-level connect"
                    );
                }

                // Set the SampleGrabber to accept NV12 with a proper
                // FORMAT_VideoInfo so DirectShow treats it as a real
                // constraint rather than a wildcard hint.
                let nv12_mt = AmMediaType {
                    major_type: MEDIATYPE_VIDEO,
                    sub_type: MEDIASUBTYPE_NV12,
                    format_type: FORMAT_VIDEOINFO,
                    ..AmMediaType::default()
                };

                let hr = grabber.set_media_type(&nv12_mt);
                if hr.is_err() {
                    error!("SetMediaType(NV12) failed: {hr:?}");
                    return Err(format!("SetMediaType(NV12) failed: {hr:?}"));
                }

                graph2.Connect(&source_out, &grabber_in).map_err(|e| {
                    error!("failed to connect OBS source -> grabber (NV12): {e}");
                    format!("failed to connect source -> grabber: {e}")
                })?;

                info!("connected OBS Virtual Camera with NV12");
            } else {
                let rgb24_mt = AmMediaType {
                    major_type: MEDIATYPE_VIDEO,
                    sub_type: MEDIASUBTYPE_RGB24,
                    format_type: GUID::zeroed(),
                    ..AmMediaType::default()
                };

                let hr = grabber.set_media_type(&rgb24_mt);
                if hr.is_err() {
                    error!("SetMediaType(RGB24) failed: {hr:?}");
                    return Err(format!("SetMediaType failed: {hr:?}"));
                }

                let connect_result = graph2.Connect(&source_out, &grabber_in);

                if let Err(ref e) = connect_result {
                    // 0x80040217 = VFW_E_CANNOT_CONNECT — no colour converter
                    // available. Fall back to accepting any subtype.
                    warn!("RGB24 connect failed ({e}), retrying with any subtype");

                    // Disconnect any partial connections before retrying
                    let _ = graph2.Disconnect(&source_out);
                    let _ = graph2.Disconnect(&grabber_in);

                    let any_mt = AmMediaType {
                        major_type: MEDIATYPE_VIDEO,
                        sub_type: GUID::zeroed(),
                        format_type: GUID::zeroed(),
                        ..AmMediaType::default()
                    };

                    let hr = grabber.set_media_type(&any_mt);
                    if hr.is_err() {
                        error!("SetMediaType(any) failed: {hr:?}");
                        return Err(format!("SetMediaType(any) failed: {hr:?}"));
                    }

                    graph2.Connect(&source_out, &grabber_in).map_err(|e| {
                        error!("failed to connect source -> grabber (fallback): {e}");
                        format!("failed to connect source -> grabber: {e}")
                    })?;

                    info!("connected with any-subtype fallback");
                }
            }

            // 7. Connect: SampleGrabber -> NullRenderer
            let grabber_out = find_unconnected_pin(&grabber_filter, 1)?;
            let null_in = find_unconnected_pin(&null_renderer, 0)?;
            graph2.Connect(&grabber_out, &null_in).map_err(|e| {
                error!("failed to connect grabber -> null renderer: {e}");
                format!("failed to connect grabber -> null renderer: {e}")
            })?;

            // 8. Query the actual negotiated resolution from the connected
            //    media type — the camera may have chosen a different size
            //    than what was requested.
            let mut connected_mt = AmMediaType::default();
            let hr = grabber.get_connected_media_type(&mut connected_mt);

            info!(
                "negotiated sub_type: {:?}, format_type: {:?}",
                connected_mt.sub_type, connected_mt.format_type
            );

            let (actual_width, actual_height, actual_sub_type) = if hr.is_ok()
                && !connected_mt.pb_format.is_null()
                && connected_mt.cb_format as usize >= std::mem::size_of::<VIDEOINFOHEADER>()
            {
                let vih = &*(connected_mt.pb_format as *const VIDEOINFOHEADER);
                let w = vih.bmiHeader.biWidth as u32;
                let h = vih.bmiHeader.biHeight.unsigned_abs();
                let sub = connected_mt.sub_type;
                info!("negotiated resolution: {w}x{h}");
                (w, h, sub)
            } else {
                warn!(
                    "could not query connected media type (hr={hr:?}), \
                     falling back to requested {width}x{height}"
                );
                (width, height, MEDIASUBTYPE_RGB24)
            };

            // Free the format block if allocated
            if !connected_mt.pb_format.is_null() {
                windows::Win32::System::Com::CoTaskMemFree(Some(
                    connected_mt.pb_format as *mut core::ffi::c_void,
                ));
            }

            // 9. Set up callback (mode 1 = BufferCB) with actual resolution
            let callback = create_frame_callback(
                buffer,
                actual_width,
                actual_height,
                actual_sub_type,
                Arc::clone(&running),
                stats,
            );

            let hr = grabber.set_callback(callback, 1);
            if hr.is_err() {
                error!("SetCallback failed: {hr:?}");
                return Err(format!("SetCallback failed: {hr:?}"));
            }

            // 10. Accept frames BEFORE running the graph to avoid dropping
            //     the first few frames
            running.store(true, Ordering::Relaxed);

            let media_control: IMediaControl = graph.cast().map_err(|e| {
                error!("failed to get IMediaControl: {e}");
                format!("failed to get IMediaControl: {e}")
            })?;

            // OBS Virtual Camera doesn't handle reference clock timing correctly
            // (OBS issue #4929, #8057). Remove the clock so the NullRenderer
            // delivers every sample immediately instead of scheduling by timestamp.
            if is_obs_virtual_camera(friendly_name) {
                let media_filter: IMediaFilter = graph.cast().map_err(|e| {
                    error!("failed to get IMediaFilter: {e}");
                    format!("failed to get IMediaFilter: {e}")
                })?;
                media_filter
                    .SetSyncSource(None)
                    .map_err(|e| format!("SetSyncSource(NULL) failed: {e}"))?;
                info!("disabled reference clock for OBS Virtual Camera");
            }

            media_control.Run().map_err(|e| {
                error!("failed to run graph: {e}");
                running.store(false, Ordering::Relaxed);
                format!("failed to run graph: {e}")
            })?;

            info!("capture graph running for {device_path} at {actual_width}x{actual_height}");

            // 11. Block until stopped
            while running.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }

            // 12. Cleanup
            debug!("stopping capture graph for {device_path}");
            if let Err(e) = media_control.Stop() {
                warn!("IMediaControl::Stop failed: {e}");
            }

            Ok(())
        }
    }

    /// Find an unconnected pin on a filter by direction.
    /// direction: 0 = PINDIR_INPUT, 1 = PINDIR_OUTPUT
    unsafe fn find_unconnected_pin(filter: &IBaseFilter, direction: i32) -> Result<IPin, String> {
        let pin_enum = filter
            .EnumPins()
            .map_err(|e| format!("EnumPins failed: {e}"))?;

        let mut pin_array = [None; 1];

        loop {
            let hr = pin_enum.Next(&mut pin_array, None);
            if hr.is_err() {
                break;
            }

            let Some(pin) = pin_array[0].take() else {
                break;
            };

            let dir = match pin.QueryDirection() {
                Ok(d) => d,
                Err(_) => continue,
            };

            if dir.0 != direction {
                continue;
            }

            // Check if pin is unconnected
            match pin.ConnectedTo() {
                Ok(_) => continue,
                Err(_) => return Ok(pin),
            }
        }

        Err(format!("no unconnected pin with direction {direction}"))
    }
}

/// Returns `true` if the friendly name looks like an OBS Virtual Camera.
///
/// OBS Virtual Camera lies about supporting RGB24 format — `SetFormat()`
/// returns `S_OK` for anything but it only delivers NV12 frames reliably.
/// When detected, the capture graph should request NV12 directly instead
/// of relying on DirectShow's colour converter + RGB24.
pub fn is_obs_virtual_camera(friendly_name: &str) -> bool {
    let lower = friendly_name.to_ascii_lowercase();
    lower.contains("obs") && lower.contains("virtual")
}

/// Convert BGR24 bottom-up data to RGB24 top-down.
///
/// DirectShow delivers frames in BGR colour order with rows stored
/// bottom-to-top. This function flips rows vertically and swaps
/// blue/red channels.
pub fn convert_bgr_bottom_up_to_rgb(bgr: &[u8], width: usize, height: usize) -> Vec<u8> {
    let stride = width * 3;
    let expected = stride * height;
    if bgr.len() < expected {
        return Vec::new();
    }

    let mut rgb = vec![0u8; expected];
    for y in 0..height {
        let src_row = &bgr[(height - 1 - y) * stride..(height - y) * stride];
        let dst_row = &mut rgb[y * stride..(y + 1) * stride];
        for x in 0..width {
            dst_row[x * 3] = src_row[x * 3 + 2]; // R
            dst_row[x * 3 + 1] = src_row[x * 3 + 1]; // G
            dst_row[x * 3 + 2] = src_row[x * 3]; // B
        }
    }
    rgb
}

/// Convert YUY2 (YUYV) packed data to RGB24.
///
/// YUY2 stores two pixels per 4-byte macro-pixel: [Y0, U, Y1, V].
/// Uses BT.601 conversion with fixed-point integer arithmetic (<<8)
/// for performance on the DirectShow capture thread. Width must be even.
pub fn convert_yuy2_to_rgb(yuy2: &[u8], width: usize, height: usize) -> Vec<u8> {
    let expected = width * height * 2;
    if yuy2.len() < expected || width == 0 || height == 0 {
        return Vec::new();
    }

    let mut rgb = vec![0u8; width * height * 3];
    for i in 0..(width * height / 2) {
        let y0 = yuy2[i * 4] as i32;
        let u = yuy2[i * 4 + 1] as i32 - 128;
        let y1 = yuy2[i * 4 + 2] as i32;
        let v = yuy2[i * 4 + 3] as i32 - 128;

        let base = i * 6;
        rgb[base] = ((y0 * 256 + 359 * v) >> 8).clamp(0, 255) as u8;
        rgb[base + 1] = ((y0 * 256 - 88 * u - 183 * v) >> 8).clamp(0, 255) as u8;
        rgb[base + 2] = ((y0 * 256 + 454 * u) >> 8).clamp(0, 255) as u8;
        rgb[base + 3] = ((y1 * 256 + 359 * v) >> 8).clamp(0, 255) as u8;
        rgb[base + 4] = ((y1 * 256 - 88 * u - 183 * v) >> 8).clamp(0, 255) as u8;
        rgb[base + 5] = ((y1 * 256 + 454 * u) >> 8).clamp(0, 255) as u8;
    }
    rgb
}

/// Convert NV12 planar data to RGB24.
///
/// NV12 stores a full-resolution Y plane followed by an interleaved UV plane
/// at half resolution in both dimensions (4:2:0 subsampling). Each 2x2 block
/// of pixels shares one U,V pair. Uses BT.601 conversion with fixed-point
/// integer arithmetic (<<8) for performance on the DirectShow capture thread.
pub fn convert_nv12_to_rgb(nv12: &[u8], width: usize, height: usize) -> Vec<u8> {
    let expected = width * height * 3 / 2;
    if nv12.len() < expected || width == 0 || height == 0 {
        return Vec::new();
    }

    let y_plane = &nv12[..width * height];
    let uv_plane = &nv12[width * height..];

    let mut rgb = vec![0u8; width * height * 3];

    for row in 0..height {
        for col in 0..width {
            let y = y_plane[row * width + col] as i32;
            let uv_index = (row / 2) * width + (col / 2) * 2;
            let u = uv_plane[uv_index] as i32 - 128;
            let v = uv_plane[uv_index + 1] as i32 - 128;

            let base = (row * width + col) * 3;
            rgb[base] = ((y * 256 + 359 * v) >> 8).clamp(0, 255) as u8;
            rgb[base + 1] = ((y * 256 - 88 * u - 183 * v) >> 8).clamp(0, 255) as u8;
            rgb[base + 2] = ((y * 256 + 454 * u) >> 8).clamp(0, 255) as u8;
        }
    }

    rgb
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::capture::{Frame, FrameBuffer};
    use std::sync::Arc;

    #[test]
    fn converts_bgr_bottom_up_to_rgb_top_down() {
        // 2x2 BGR24 bottom-up image
        // Row 0 (bottom of image): blue pixels (B=255, G=0, R=0)
        // Row 1 (top of image): red pixels (B=0, G=0, R=255)
        let width = 2usize;
        let height = 2usize;
        let stride = width * 3;

        let mut bgr = vec![0u8; stride * height];
        // Row 0 (bottom): B=255, G=0, R=0
        bgr[0] = 255;
        bgr[3] = 255;
        // Row 1 (top): B=0, G=0, R=255
        bgr[8] = 255;
        bgr[11] = 255;

        let rgb = convert_bgr_bottom_up_to_rgb(&bgr, width, height);

        // After flip: output row 0 = input row 1 (top of image)
        // BGR(0,0,255) -> RGB(255,0,0) = red
        assert_eq!(rgb[0], 255); // R
        assert_eq!(rgb[1], 0); // G
        assert_eq!(rgb[2], 0); // B

        // Output row 1 = input row 0 (bottom of image)
        // BGR(255,0,0) -> RGB(0,0,255) = blue
        assert_eq!(rgb[6], 0); // R
        assert_eq!(rgb[7], 0); // G
        assert_eq!(rgb[8], 255); // B
    }

    #[test]
    fn handles_undersized_buffer_gracefully() {
        let result = convert_bgr_bottom_up_to_rgb(&[0u8; 5], 2, 2);
        assert!(result.is_empty());
    }

    #[test]
    fn handles_1x1_pixel() {
        let bgr = vec![100u8, 150, 200]; // B=100, G=150, R=200
        let rgb = convert_bgr_bottom_up_to_rgb(&bgr, 1, 1);
        assert_eq!(rgb, vec![200, 150, 100]); // R=200, G=150, B=100
    }

    #[test]
    fn converted_frame_pushes_to_buffer() {
        let buffer = Arc::new(FrameBuffer::new(3));

        let bgr = vec![50u8, 100, 150]; // B=50, G=100, R=150
        let rgb = convert_bgr_bottom_up_to_rgb(&bgr, 1, 1);

        buffer.push(Frame {
            data: rgb,
            width: 1,
            height: 1,
            timestamp_us: 42,
        });

        let frame = buffer.latest().unwrap();
        assert_eq!(frame.data, vec![150, 100, 50]); // R=150, G=100, B=50
        assert_eq!(frame.timestamp_us, 42);
    }

    #[test]
    fn empty_input_returns_empty() {
        let result = convert_bgr_bottom_up_to_rgb(&[], 0, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn converts_yuy2_white_pixel_pair() {
        // White in YUY2: Y=235, U=128, V=128 (no chroma)
        let yuy2 = vec![235, 128, 235, 128];
        let rgb = convert_yuy2_to_rgb(&yuy2, 2, 1);
        assert_eq!(rgb.len(), 6);
        // Y=235, U=0, V=0 => R=235, G=235, B=235
        assert_eq!(rgb[0], 235);
        assert_eq!(rgb[1], 235);
        assert_eq!(rgb[2], 235);
        assert_eq!(rgb[3], 235);
        assert_eq!(rgb[4], 235);
        assert_eq!(rgb[5], 235);
    }

    #[test]
    fn converts_yuy2_black_pixel_pair() {
        // Black in YUY2: Y=0, U=128, V=128
        let yuy2 = vec![0, 128, 0, 128];
        let rgb = convert_yuy2_to_rgb(&yuy2, 2, 1);
        assert_eq!(rgb, vec![0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn yuy2_undersized_buffer_returns_empty() {
        let result = convert_yuy2_to_rgb(&[0u8; 3], 2, 1);
        assert!(result.is_empty());
    }

    #[test]
    fn yuy2_zero_dimensions_returns_empty() {
        let result = convert_yuy2_to_rgb(&[], 0, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn converts_yuy2_2x2_produces_correct_size() {
        // 2x2 image = 2 macro-pixels (one per row)
        let yuy2 = vec![
            128, 128, 128, 128, // row 0: grey pair
            128, 128, 128, 128, // row 1: grey pair
        ];
        let rgb = convert_yuy2_to_rgb(&yuy2, 2, 2);
        assert_eq!(rgb.len(), 2 * 2 * 3); // 12 bytes
    }

    #[test]
    fn converts_nv12_to_rgb_grey() {
        // 2x2 NV12 grey image: Y=128 for all pixels, U=128, V=128 (no chroma)
        // Y plane: 4 bytes, UV plane: 2 bytes (one U, one V interleaved)
        let nv12 = vec![
            128, 128, 128, 128, // Y plane (2x2)
            128, 128, // UV plane (1 pair for the 2x2 block)
        ];
        let rgb = convert_nv12_to_rgb(&nv12, 2, 2);
        assert_eq!(rgb.len(), 2 * 2 * 3);
        // Y=128, U=0, V=0 => R=128, G=128, B=128
        for pixel in rgb.chunks(3) {
            assert_eq!(pixel, [128, 128, 128]);
        }
    }

    #[test]
    fn nv12_undersized_buffer_returns_empty() {
        // NV12 for 2x2 should be 6 bytes (4 Y + 2 UV), pass only 5
        let result = convert_nv12_to_rgb(&[0u8; 5], 2, 2);
        assert!(result.is_empty());
    }

    #[test]
    fn converts_nv12_to_rgb_4x2() {
        // 4x2 NV12: Y plane = 8 bytes, UV plane = 4 bytes (2 U/V pairs)
        // All grey: Y=200, U=128, V=128
        let mut nv12 = vec![200u8; 8]; // Y plane
        nv12.extend_from_slice(&[128, 128, 128, 128]); // UV plane
        let rgb = convert_nv12_to_rgb(&nv12, 4, 2);
        assert_eq!(rgb.len(), 4 * 2 * 3);
        // Y=200, U=0, V=0 => R=200, G=200, B=200
        for pixel in rgb.chunks(3) {
            assert_eq!(pixel, [200, 200, 200]);
        }
    }

    #[test]
    fn nv12_zero_dimensions_returns_empty() {
        let result = convert_nv12_to_rgb(&[], 0, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn converts_nv12_to_rgb_black() {
        // Black: Y=0, U=128, V=128
        let nv12 = vec![
            0, 0, 0, 0, // Y plane (2x2)
            128, 128, // UV plane
        ];
        let rgb = convert_nv12_to_rgb(&nv12, 2, 2);
        assert_eq!(rgb.len(), 12);
        for pixel in rgb.chunks(3) {
            assert_eq!(pixel, [0, 0, 0]);
        }
    }

    #[test]
    fn converts_nv12_to_rgb_white() {
        // White: Y=235, U=128, V=128
        let nv12 = vec![
            235, 235, 235, 235, // Y plane (2x2)
            128, 128, // UV plane
        ];
        let rgb = convert_nv12_to_rgb(&nv12, 2, 2);
        assert_eq!(rgb.len(), 12);
        for pixel in rgb.chunks(3) {
            assert_eq!(pixel, [235, 235, 235]);
        }
    }

    #[test]
    fn detects_obs_virtual_camera() {
        assert!(is_obs_virtual_camera("OBS Virtual Camera"));
        assert!(is_obs_virtual_camera("OBS-Virtual-Camera"));
        assert!(is_obs_virtual_camera("obs virtual cam"));
    }

    #[test]
    fn does_not_detect_real_cameras_as_obs() {
        assert!(!is_obs_virtual_camera("Logitech C920"));
        assert!(!is_obs_virtual_camera("HD Webcam"));
        assert!(!is_obs_virtual_camera("Virtual Camera"));
        assert!(!is_obs_virtual_camera("OBS Studio"));
    }
}
