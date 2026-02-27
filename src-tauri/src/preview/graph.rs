// DirectShow filter graph for camera frame capture.
//
// Builds a Source -> SampleGrabber -> NullRenderer pipeline and delivers
// raw RGB24 frames via a callback into the shared FrameBuffer.

#[cfg(target_os = "windows")]
pub mod directshow {
    use parking_lot::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use tracing::{debug, info};
    use windows::core::{Interface, GUID, HRESULT};
    use windows::Win32::Media::DirectShow::{
        IBaseFilter, ICreateDevEnum, IFilterGraph2, IGraphBuilder, IMediaControl, IPin,
    };
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
    use crate::preview::graph::convert_bgr_bottom_up_to_rgb;

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
            return HRESULT(0);
        }

        let len = buffer_len as usize;
        let raw = std::slice::from_raw_parts(buffer, len);

        // DirectShow delivers BGR24 bottom-up; convert to RGB24 top-down
        let width = data.width as usize;
        let height = data.height as usize;
        let stride = width * 3;
        let expected = stride * height;

        if len < expected {
            return HRESULT(0);
        }

        let rgb = convert_bgr_bottom_up_to_rgb(raw, width, height);

        let timestamp_us = (sample_time * 1_000_000.0) as u64;
        let frame_bytes = rgb.len();

        data.buffer.push(Frame {
            data: rgb,
            width: data.width,
            height: data.height,
            timestamp_us,
        });

        data.stats.lock().record_frame(frame_bytes, timestamp_us);

        HRESULT(0)
    }

    /// Create a new ISampleGrabberCB implementation that pushes frames
    /// into the buffer.
    fn create_frame_callback(
        buffer: Arc<FrameBuffer>,
        width: u32,
        height: u32,
        running: Arc<AtomicBool>,
        stats: Arc<Mutex<DiagnosticStats>>,
    ) -> *mut core::ffi::c_void {
        let data = Box::new(FrameCallbackData {
            vtbl: &FRAME_CALLBACK_VTBL,
            ref_count: std::sync::atomic::AtomicU32::new(1),
            buffer,
            width,
            height,
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

    /// Find a DirectShow source filter by device path.
    unsafe fn find_source_filter(device_path: &str) -> Result<IBaseFilter, String> {
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
            if path != device_path {
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

        Err(format!("device not found: {device_path}"))
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
        width: u32,
        height: u32,
        buffer: Arc<FrameBuffer>,
        running: Arc<AtomicBool>,
        stats: Arc<Mutex<DiagnosticStats>>,
    ) -> Result<(), String> {
        unsafe {
            let _guard = ComGuard::init()?;

            // 1. Create filter graph
            let graph: IGraphBuilder =
                CoCreateInstance(&CLSID_FILTER_GRAPH, None, CLSCTX_INPROC_SERVER)
                    .map_err(|e| format!("failed to create filter graph: {e}"))?;

            let graph2: IFilterGraph2 = graph
                .cast()
                .map_err(|e| format!("failed to get IFilterGraph2: {e}"))?;

            // 2. Find and add source filter
            let source = find_source_filter(device_path)?;
            graph2
                .AddFilter(&source, windows::core::w!("Source"))
                .map_err(|e| format!("failed to add source filter: {e}"))?;

            // 3. Create and add SampleGrabber filter
            let grabber_filter: IBaseFilter =
                CoCreateInstance(&CLSID_SAMPLE_GRABBER, None, CLSCTX_INPROC_SERVER)
                    .map_err(|e| format!("failed to create SampleGrabber: {e}"))?;

            graph2
                .AddFilter(&grabber_filter, windows::core::w!("SampleGrabber"))
                .map_err(|e| format!("failed to add SampleGrabber filter: {e}"))?;

            // 4. Configure SampleGrabber to accept RGB24
            let grabber = SampleGrabber::from_filter(&grabber_filter)
                .ok_or("failed to query ISampleGrabber")?;

            let mt = AmMediaType {
                major_type: MEDIATYPE_VIDEO,
                sub_type: MEDIASUBTYPE_RGB24,
                format_type: FORMAT_VIDEOINFO,
                ..AmMediaType::default()
            };

            let hr = grabber.set_media_type(&mt);
            if hr.is_err() {
                return Err(format!("SetMediaType failed: {hr:?}"));
            }

            let hr = grabber.set_one_shot(false);
            if hr.is_err() {
                return Err(format!("SetOneShot failed: {hr:?}"));
            }

            let hr = grabber.set_buffer_samples(false);
            if hr.is_err() {
                return Err(format!("SetBufferSamples failed: {hr:?}"));
            }

            // 5. Create and add NullRenderer
            let null_renderer: IBaseFilter =
                CoCreateInstance(&CLSID_NULL_RENDERER, None, CLSCTX_INPROC_SERVER)
                    .map_err(|e| format!("failed to create NullRenderer: {e}"))?;

            graph2
                .AddFilter(&null_renderer, windows::core::w!("NullRenderer"))
                .map_err(|e| format!("failed to add NullRenderer: {e}"))?;

            // 6. Connect: Source output -> SampleGrabber input
            let source_out = find_unconnected_pin(&source, 1)?;
            let grabber_in = find_unconnected_pin(&grabber_filter, 0)?;
            graph2
                .ConnectDirect(&source_out, &grabber_in, None)
                .map_err(|e| format!("failed to connect source -> grabber: {e}"))?;

            // 7. Connect: SampleGrabber output -> NullRenderer input
            let grabber_out = find_unconnected_pin(&grabber_filter, 1)?;
            let null_in = find_unconnected_pin(&null_renderer, 0)?;
            graph2
                .ConnectDirect(&grabber_out, &null_in, None)
                .map_err(|e| format!("failed to connect grabber -> null renderer: {e}"))?;

            // 8. Set up callback (mode 1 = BufferCB)
            let callback =
                create_frame_callback(buffer, width, height, Arc::clone(&running), stats);

            let hr = grabber.set_callback(callback, 1);
            if hr.is_err() {
                return Err(format!("SetCallback failed: {hr:?}"));
            }

            // 9. Run the graph
            let media_control: IMediaControl = graph
                .cast()
                .map_err(|e| format!("failed to get IMediaControl: {e}"))?;

            media_control
                .Run()
                .map_err(|e| format!("failed to run graph: {e}"))?;

            info!("capture graph running for {device_path}");
            running.store(true, Ordering::Relaxed);

            // 10. Block until stopped
            while running.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }

            // 11. Cleanup
            debug!("stopping capture graph");
            let _ = media_control.Stop();

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
}
