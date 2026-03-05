//! IMFMediaSource + IMFMediaEventGenerator implementation.
//!
//! This is the heart of the virtual camera COM DLL. FrameServer creates an
//! instance of this via IClassFactory, then drives it through the standard
//! media source lifecycle: GetCharacteristics → CreatePresentationDescriptor →
//! Start → (frame delivery) → Stop → Shutdown.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use vcam_shared::SharedMemoryReader;

use windows::Win32::Foundation::{E_UNEXPECTED, S_OK};
use windows::Win32::Media::KernelStreaming::{
    IKsControl, IKsControl_Impl, IKsPropertySet, IKsPropertySet_Impl, KSCATEGORY_VIDEO_CAMERA,
    KSIDENTIFIER, PINNAME_VIDEO_CAPTURE,
};
use windows::Win32::Media::MediaFoundation::{
    IMFAsyncCallback, IMFAsyncResult, IMFAttributes, IMFGetService, IMFGetService_Impl,
    IMFMediaEvent, IMFMediaEventGenerator_Impl, IMFMediaEventQueue, IMFMediaSourceEx,
    IMFMediaSourceEx_Impl, IMFMediaSource_Impl, IMFMediaStream2, IMFMediaType,
    IMFPresentationDescriptor, IMFRealTimeClientEx, IMFRealTimeClientEx_Impl,
    IMFSampleAllocatorControl, IMFSampleAllocatorControl_Impl, IMFShutdown, IMFShutdown_Impl,
    IMFStreamDescriptor, MENewStream, MESourceStarted, MESourceStopped, MFCreateAttributes,
    MFCreateEventQueue, MFCreateMediaType, MFCreatePresentationDescriptor,
    MFCreateStreamDescriptor, MFFrameSourceTypes_Color, MFMediaType_Video, MFSampleAllocatorUsage,
    MFSampleAllocatorUsage_DoesNotAllocate, MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS,
    MFMEDIASOURCE_IS_LIVE, MFSHUTDOWN_COMPLETED, MFSHUTDOWN_STATUS,
    MF_DEVICESTREAM_ATTRIBUTE_FRAMESOURCE_TYPES, MF_DEVICESTREAM_FRAMESERVER_SHARED,
    MF_DEVICESTREAM_STREAM_CATEGORY, MF_DEVICESTREAM_STREAM_ID, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_CATEGORY,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID, MF_E_INVALIDREQUEST, MF_E_SHUTDOWN,
    MF_E_UNSUPPORTED_SERVICE, MF_MEDIASOURCE_SERVICE, MF_STREAM_STATE_RUNNING,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows_core::{implement, IUnknown, IUnknownImpl, Interface, Ref, BOOL, GUID, HRESULT};

use crate::media_stream::VCamMediaStream;
use crate::{
    decrement_object_count, increment_object_count, DEFAULT_HEIGHT, DEFAULT_WIDTH,
    SHARED_MEMORY_FILE_PATH, TARGET_FPS,
};

/// The MF_MT_SUBTYPE GUID for NV12.
const MF_MT_SUBTYPE_NV12: GUID = GUID::from_u128(0x3231_564E_0000_0010_8000_00AA_0038_9B71);

/// MF_MT_MAJOR_TYPE attribute GUID.
const MF_MT_MAJOR_TYPE: GUID = GUID::from_u128(0x48eba18e_f8c9_4687_bf11_0a74c9f96a8f);

/// MF_MT_SUBTYPE attribute GUID.
const MF_MT_SUBTYPE: GUID = GUID::from_u128(0xf7e34c9a_42e8_4714_b74b_cb29d72c35e5);

/// MF_MT_FRAME_SIZE attribute GUID.
const MF_MT_FRAME_SIZE: GUID = GUID::from_u128(0x1652c33d_d6b2_4012_b834_72030849a37d);

/// MF_MT_FRAME_RATE attribute GUID.
const MF_MT_FRAME_RATE: GUID = GUID::from_u128(0xc459a2e8_3d2c_4e44_b132_fee5156c7bb0);

/// MF_MT_INTERLACE_MODE attribute GUID.
const MF_MT_INTERLACE_MODE: GUID = GUID::from_u128(0xe2724bb8_e676_4806_b4b2_a8d6efb44ccd);

/// MF_MT_ALL_SAMPLES_INDEPENDENT attribute GUID.
const MF_MT_ALL_SAMPLES_INDEPENDENT: GUID = GUID::from_u128(0xc9173739_5e56_461c_b713_46fb995cb95f);

/// MFVideoInterlace_Progressive value.
const MF_VIDEO_INTERLACE_PROGRESSIVE: u32 = 2;

/// MF_DEVICESTREAM_MAX_FRAME_BUFFERS {1684CEBE-3175-4985-882C-0EFD3E8AC11E}
const MF_DEVICESTREAM_MAX_FRAME_BUFFERS: GUID =
    GUID::from_u128(0x1684CEBE_3175_4985_882C_0EFD3E8AC11E);

/// State of the media source lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceState {
    /// Created but not yet started.
    Stopped,
    /// Actively delivering frames.
    Started,
    /// Permanently shut down.
    Shutdown,
}

/// Custom media source for the virtual camera.
///
/// Implements the full set of interfaces required by Windows FrameServer when
/// activating an `IMFVirtualCamera`:
///
/// - `IMFMediaSourceEx` (+ inherited `IMFMediaSource` + `IMFMediaEventGenerator`)
/// - `IKsControl` — FrameServer queries KS property support; stubs return
///   `ERROR_SET_NOT_FOUND` (no properties supported).
/// - `IKsPropertySet` — legacy KS property set; stubs return
///   `ERROR_SET_NOT_FOUND`.
/// - `IMFGetService` — auxiliary service queries; stubs return
///   `MF_E_UNSUPPORTED_SERVICE`.
/// - `IMFRealTimeClientEx` — real-time thread registration; stubs accept
///   calls silently.
/// - `IMFShutdown` — shutdown status reporting; forwards to internal state.
/// - `IMFSampleAllocatorControl` — sample allocator hint; stubs report
///   `MFSampleAllocatorUsage_DoesNotAllocate`.
#[implement(
    IMFMediaSourceEx,
    IKsControl,
    IKsPropertySet,
    IMFGetService,
    IMFRealTimeClientEx,
    IMFShutdown,
    IMFSampleAllocatorControl
)]
pub(crate) struct VCamMediaSource {
    /// Attribute store forwarded from the `IMFActivate` activation object.
    /// Contains device attributes set by FrameServer (symbolic link, etc.).
    source_attributes: IMFAttributes,
    event_queue: IMFMediaEventQueue,
    state: Mutex<SourceState>,
    stream: Mutex<Option<IMFMediaStream2>>,
    shutdown: AtomicBool,
}

impl VCamMediaSource {
    /// Create a new media source with an empty attribute store.
    ///
    /// Used by `IClassFactory::CreateInstance` when creating a source directly
    /// (not via `IMFActivate`), e.g. in tests.
    pub(crate) fn new() -> windows_core::Result<Self> {
        let mut attrs: Option<IMFAttributes> = None;
        unsafe { MFCreateAttributes(&mut attrs, 0)? };
        let attrs = attrs.ok_or(windows_core::Error::from(MF_E_SHUTDOWN))?;
        Self::new_with_attributes(attrs)
    }

    /// Create a new media source with attributes forwarded from `VCamActivate`.
    ///
    /// The provided attribute store is the same one that FrameServer populates
    /// on the `IMFActivate` object (symbolic link, friendly name, etc.).
    pub(crate) fn new_with_attributes(
        source_attributes: IMFAttributes,
    ) -> windows_core::Result<Self> {
        let event_queue = unsafe { MFCreateEventQueue()? };
        increment_object_count();
        Ok(Self {
            source_attributes,
            event_queue,
            state: Mutex::new(SourceState::Stopped),
            stream: Mutex::new(None),
            shutdown: AtomicBool::new(false),
        })
    }

    fn check_shutdown(&self) -> windows_core::Result<()> {
        if self.shutdown.load(Ordering::Acquire) {
            Err(windows_core::Error::from(MF_E_SHUTDOWN))
        } else {
            Ok(())
        }
    }

    /// Full shutdown logic shared by `IMFMediaSource::Shutdown` and `IMFShutdown::Shutdown`.
    fn do_shutdown(&self) -> windows_core::Result<()> {
        self.shutdown.store(true, Ordering::Release);

        {
            let mut state = self.state.lock().unwrap();
            *state = SourceState::Shutdown;
        }

        // Drop the stream (and its SharedMemoryReader handle).
        if let Some(stream) = self.stream.lock().unwrap().take() {
            drop(stream);
        }

        // Shut down the event queue.
        unsafe { self.event_queue.Shutdown()? };

        Ok(())
    }
}

impl Drop for VCamMediaSource {
    fn drop(&mut self) {
        decrement_object_count();
    }
}

impl IMFMediaSource_Impl for VCamMediaSource_Impl {
    fn GetCharacteristics(&self) -> windows_core::Result<u32> {
        crate::trace::trace_method("IMFMediaSource::GetCharacteristics");
        self.check_shutdown()?;
        Ok(MFMEDIASOURCE_IS_LIVE.0 as u32)
    }

    fn CreatePresentationDescriptor(&self) -> windows_core::Result<IMFPresentationDescriptor> {
        crate::trace::trace_method("IMFMediaSource::CreatePresentationDescriptor");
        self.check_shutdown()?;

        let media_type = create_nv12_media_type(DEFAULT_WIDTH, DEFAULT_HEIGHT, TARGET_FPS)?;

        // Create stream descriptor with a single media type.
        let type_array = [Some(media_type)];
        let stream_desc: IMFStreamDescriptor = unsafe { MFCreateStreamDescriptor(0, &type_array)? };

        // Set the current media type on the stream descriptor's media type handler.
        unsafe {
            let handler = stream_desc.GetMediaTypeHandler()?;
            handler.SetCurrentMediaType(&type_array[0].clone().unwrap())?;
        }

        // Set mandatory stream attributes required by the FrameServer Custom Media Source spec.
        unsafe {
            let attrs = stream_desc.cast::<IMFAttributes>()?;
            attrs.SetGUID(&MF_DEVICESTREAM_STREAM_CATEGORY, &PINNAME_VIDEO_CAPTURE)?;
            attrs.SetUINT32(&MF_DEVICESTREAM_STREAM_ID, 0)?;
            // Mark this stream as shared so multiple apps can access it.
            attrs.SetUINT32(&MF_DEVICESTREAM_FRAMESERVER_SHARED, 1)?;
            // Indicate this stream produces colour video frames.
            attrs.SetUINT32(
                &MF_DEVICESTREAM_ATTRIBUTE_FRAMESOURCE_TYPES,
                MFFrameSourceTypes_Color.0 as u32,
            )?;
            // Maximum frame buffers FrameServer may allocate for this stream.
            attrs.SetUINT32(&MF_DEVICESTREAM_MAX_FRAME_BUFFERS, 2)?;

            // Media type hints — FrameServer cross-checks these against the
            // media type handler. Keep in sync with GetStreamAttributes().
            attrs.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            attrs.SetGUID(&MF_MT_SUBTYPE, &MF_MT_SUBTYPE_NV12)?;
            let frame_size = ((DEFAULT_WIDTH as u64) << 32) | (DEFAULT_HEIGHT as u64);
            attrs.SetUINT64(&MF_MT_FRAME_SIZE, frame_size)?;
            let frame_rate = ((TARGET_FPS as u64) << 32) | 1u64;
            attrs.SetUINT64(&MF_MT_FRAME_RATE, frame_rate)?;
        }

        // Create presentation descriptor with a single stream.
        let stream_descs: &[Option<IMFStreamDescriptor>] = &[Some(stream_desc)];
        let pd: IMFPresentationDescriptor =
            unsafe { MFCreatePresentationDescriptor(Some(stream_descs))? };

        // Select the first (and only) stream.
        unsafe { pd.SelectStream(0)? };

        Ok(pd)
    }

    fn Start(
        &self,
        pdescriptor: Ref<IMFPresentationDescriptor>,
        _pguidtimeformat: *const GUID,
        _pvarstartposition: *const PROPVARIANT,
    ) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFMediaSource::Start");
        self.check_shutdown()?;

        let pd: &IMFPresentationDescriptor = pdescriptor.ok()?;

        let mut state = self.state.lock().unwrap();
        if *state == SourceState::Shutdown {
            return Err(windows_core::Error::from(MF_E_SHUTDOWN));
        }

        // Get the stream descriptor from the presentation descriptor.
        let mut selected = BOOL(0);
        let mut stream_desc: Option<IMFStreamDescriptor> = None;
        unsafe {
            pd.GetStreamDescriptorByIndex(0, &mut selected, &mut stream_desc)?;
        }
        let stream_desc = stream_desc.ok_or(windows_core::Error::from(E_UNEXPECTED))?;

        // Open the file-backed shared memory created by the app. If the file
        // doesn't exist yet, deliver black frames until it appears.
        let reader = SharedMemoryReader::open_file(Path::new(SHARED_MEMORY_FILE_PATH))
            .map_err(|e| {
                crate::trace::trace(&format!("SHM file not ready: {e}"));
                // Not fatal — deliver black frames until file appears.
            })
            .ok()
            .map(Arc::new);

        if reader.is_some() {
            crate::trace::trace(&format!(
                "Opened shared memory file '{SHARED_MEMORY_FILE_PATH}'"
            ));
        }

        // Create and store the media stream. FrameServer requires IMFMediaStream2.
        let stream = VCamMediaStream::new(&stream_desc, reader)?;
        let stream_iface: IMFMediaStream2 = stream.into();

        // Transition to running state per MSDN FrameServer sample.
        unsafe { stream_iface.SetStreamState(MF_STREAM_STATE_RUNNING)? };

        {
            let mut stream_lock = self.stream.lock().unwrap();
            *stream_lock = Some(stream_iface.clone());
        }

        // Queue MENewStream event with the stream as event value.
        let unknown: IUnknown = stream_iface.cast()?;
        let propvar = PROPVARIANT::from(unknown);
        unsafe {
            self.event_queue.QueueEventParamVar(
                MENewStream.0 as u32,
                &GUID::zeroed(),
                S_OK,
                &propvar,
            )?;
        }

        // Queue MESourceStarted.
        unsafe {
            self.event_queue.QueueEventParamVar(
                MESourceStarted.0 as u32,
                &GUID::zeroed(),
                S_OK,
                std::ptr::null(),
            )?;
        }

        *state = SourceState::Started;
        Ok(())
    }

    fn Stop(&self) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFMediaSource::Stop");
        self.check_shutdown()?;

        let mut state = self.state.lock().unwrap();
        if *state != SourceState::Started {
            return Ok(());
        }

        // Signal the stream to stop delivering frames.
        if let Some(stream) = self.stream.lock().unwrap().take() {
            drop(stream);
        }

        // Queue MESourceStopped.
        unsafe {
            self.event_queue.QueueEventParamVar(
                MESourceStopped.0 as u32,
                &GUID::zeroed(),
                S_OK,
                std::ptr::null(),
            )?;
        }

        *state = SourceState::Stopped;
        Ok(())
    }

    fn Shutdown(&self) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFMediaSource::Shutdown");
        self.do_shutdown()
    }

    fn Pause(&self) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFMediaSource::Pause");
        Err(windows_core::Error::from(E_UNEXPECTED))
    }
}

impl IMFMediaEventGenerator_Impl for VCamMediaSource_Impl {
    fn GetEvent(
        &self,
        dwflags: MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS,
    ) -> windows_core::Result<IMFMediaEvent> {
        crate::trace::trace_method("IMFMediaEventGenerator::GetEvent");
        self.check_shutdown()?;
        unsafe { self.event_queue.GetEvent(dwflags.0) }
    }

    fn BeginGetEvent(
        &self,
        pcallback: Ref<IMFAsyncCallback>,
        punkstate: Ref<IUnknown>,
    ) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFMediaEventGenerator::BeginGetEvent");
        self.check_shutdown()?;
        unsafe {
            self.event_queue
                .BeginGetEvent(pcallback.as_ref(), punkstate.as_ref())
        }
    }

    fn EndGetEvent(&self, presult: Ref<IMFAsyncResult>) -> windows_core::Result<IMFMediaEvent> {
        crate::trace::trace_method("IMFMediaEventGenerator::EndGetEvent");
        self.check_shutdown()?;
        unsafe { self.event_queue.EndGetEvent(presult.as_ref()) }
    }

    fn QueueEvent(
        &self,
        met: u32,
        guidextendedtype: *const GUID,
        hrstatus: HRESULT,
        pvvalue: *const PROPVARIANT,
    ) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFMediaEventGenerator::QueueEvent");
        self.check_shutdown()?;
        unsafe {
            self.event_queue
                .QueueEventParamVar(met, guidextendedtype, hrstatus, pvvalue)
        }
    }
}

/// `IMFMediaSourceEx` stub.
///
/// `IMFVirtualCamera::Start` requires this interface to be present on the media
/// source object — it fails with `E_NOINTERFACE` (0x80004002) if QI returns
/// nothing. The methods return empty attribute stores or silently ignore D3D
/// manager assignment; no source- or stream-level attributes are needed for a
/// basic virtual camera.
impl IMFMediaSourceEx_Impl for VCamMediaSource_Impl {
    fn GetSourceAttributes(&self) -> windows_core::Result<IMFAttributes> {
        crate::trace::trace_method("IMFMediaSourceEx::GetSourceAttributes");

        // Ensure required attributes are present (set defaults if missing).
        unsafe {
            if self
                .source_attributes
                .GetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE)
                .is_err()
            {
                self.source_attributes.SetGUID(
                    &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                    &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
                )?;
            }
            if self
                .source_attributes
                .GetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_CATEGORY)
                .is_err()
            {
                self.source_attributes.SetGUID(
                    &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_CATEGORY,
                    &KSCATEGORY_VIDEO_CAMERA,
                )?;
            }
        }

        Ok(self.source_attributes.clone())
    }

    fn GetStreamAttributes(&self, dwstreamidentifier: u32) -> windows_core::Result<IMFAttributes> {
        crate::trace::trace(&format!(
            "IMFMediaSourceEx::GetStreamAttributes called (stream={dwstreamidentifier})"
        ));

        // We only expose a single stream (id 0). Reject anything else.
        if dwstreamidentifier != 0 {
            return Err(windows_core::Error::from(MF_E_INVALIDREQUEST));
        }

        let mut attrs: Option<IMFAttributes> = None;
        unsafe { MFCreateAttributes(&mut attrs, 8)? };
        let attrs = attrs.ok_or(windows_core::Error::from(MF_E_SHUTDOWN))?;

        // FrameServer queries stream attributes independently from the
        // presentation descriptor. Populate the same mandatory attributes
        // that CreatePresentationDescriptor sets on the stream descriptor.
        unsafe {
            attrs.SetUINT32(&MF_DEVICESTREAM_STREAM_ID, dwstreamidentifier)?;
            attrs.SetGUID(&MF_DEVICESTREAM_STREAM_CATEGORY, &PINNAME_VIDEO_CAPTURE)?;
            attrs.SetUINT32(&MF_DEVICESTREAM_FRAMESERVER_SHARED, 1)?;
            attrs.SetUINT32(
                &MF_DEVICESTREAM_ATTRIBUTE_FRAMESOURCE_TYPES,
                MFFrameSourceTypes_Color.0 as u32,
            )?;

            // Media type attributes — FrameServer requires these to validate the stream.
            attrs.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            attrs.SetGUID(&MF_MT_SUBTYPE, &MF_MT_SUBTYPE_NV12)?;
            let frame_size = ((DEFAULT_WIDTH as u64) << 32) | (DEFAULT_HEIGHT as u64);
            attrs.SetUINT64(&MF_MT_FRAME_SIZE, frame_size)?;
            let frame_rate = ((TARGET_FPS as u64) << 32) | 1u64;
            attrs.SetUINT64(&MF_MT_FRAME_RATE, frame_rate)?;

            // Maximum frame buffers FrameServer may allocate for this stream.
            attrs.SetUINT32(&MF_DEVICESTREAM_MAX_FRAME_BUFFERS, 2)?;
        }

        Ok(attrs)
    }

    fn SetD3DManager(
        &self,
        _pmanager: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFMediaSourceEx::SetD3DManager");
        Ok(())
    }
}

/// `IMFGetService` implementation.
///
/// FrameServer calls `GetService(MF_MEDIASOURCE_SERVICE, IKsControl)` to
/// obtain KS property support. We return our own `IKsControl` implementation
/// for that request and `MF_E_UNSUPPORTED_SERVICE` for anything else.
impl IMFGetService_Impl for VCamMediaSource_Impl {
    fn GetService(
        &self,
        guidservice: *const GUID,
        riid: *const GUID,
        ppvobject: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        let service = unsafe { *guidservice };
        let iid = unsafe { *riid };

        if service == MF_MEDIASOURCE_SERVICE && iid == IKsControl::IID {
            crate::trace::trace_get_service(&service, &iid, "OK (IKsControl)");
            let hr = unsafe { self.QueryInterface(&iid, ppvobject) };
            return hr.ok();
        }

        crate::trace::trace_get_service(&service, &iid, "MF_E_UNSUPPORTED_SERVICE");
        Err(windows_core::Error::from(MF_E_UNSUPPORTED_SERVICE))
    }
}

/// `IKsPropertySet` stub.
///
/// Legacy KS property set interface. FrameServer may query this; we expose it
/// and return `ERROR_SET_NOT_FOUND` for every call to signal no properties.
impl IKsPropertySet_Impl for VCamMediaSource_Impl {
    fn Set(
        &self,
        _guidpropset: *const GUID,
        _dwpropid: u32,
        _pinstancedata: *const core::ffi::c_void,
        _cbinstancedata: u32,
        _ppropdata: *const core::ffi::c_void,
        _cbpropdata: u32,
    ) -> windows_core::Result<()> {
        crate::trace::trace_method("IKsPropertySet::Set");
        Err(windows_core::Error::from(HRESULT(0x80070490u32 as i32)))
    }

    fn Get(
        &self,
        _guidpropset: *const GUID,
        _dwpropid: u32,
        _pinstancedata: *const core::ffi::c_void,
        _cbinstancedata: u32,
        _ppropdata: *mut core::ffi::c_void,
        _cbpropdata: u32,
        _pcbreturned: *mut u32,
    ) -> windows_core::Result<()> {
        crate::trace::trace_method("IKsPropertySet::Get");
        Err(windows_core::Error::from(HRESULT(0x80070490u32 as i32)))
    }

    fn QuerySupported(
        &self,
        _guidpropset: *const GUID,
        _dwpropid: u32,
    ) -> windows_core::Result<u32> {
        crate::trace::trace_method("IKsPropertySet::QuerySupported");
        Err(windows_core::Error::from(HRESULT(0x80070490u32 as i32)))
    }
}

/// `IMFRealTimeClientEx` stub.
///
/// FrameServer uses this to assign work queues and real-time priorities to
/// the media source's threads. We accept all calls silently — no custom
/// work-queue management is needed for a virtual camera that delivers frames
/// from shared memory.
impl IMFRealTimeClientEx_Impl for VCamMediaSource_Impl {
    fn RegisterThreadsEx(
        &self,
        _pdwtaskindex: *mut u32,
        _wszclassname: &windows_core::PCWSTR,
        _lbasepriority: i32,
    ) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFRealTimeClientEx::RegisterThreadsEx");
        Ok(())
    }

    fn UnregisterThreads(&self) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFRealTimeClientEx::UnregisterThreads");
        Ok(())
    }

    fn SetWorkQueueEx(
        &self,
        _dwmultithreadedworkqueueid: u32,
        _lworkitembasepriority: i32,
    ) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFRealTimeClientEx::SetWorkQueueEx");
        Ok(())
    }
}

/// `IMFShutdown` implementation.
///
/// Reports the shutdown status based on the internal `shutdown` flag so that
/// FrameServer can poll whether the source has been cleanly torn down.
impl IMFShutdown_Impl for VCamMediaSource_Impl {
    fn Shutdown(&self) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFShutdown::Shutdown");
        self.do_shutdown()
    }

    fn GetShutdownStatus(&self) -> windows_core::Result<MFSHUTDOWN_STATUS> {
        crate::trace::trace_method("IMFShutdown::GetShutdownStatus");
        if self.shutdown.load(Ordering::Acquire) {
            Ok(MFSHUTDOWN_COMPLETED)
        } else {
            // Per MSDN: return MF_E_INVALIDREQUEST when not in shutdown state.
            Err(windows_core::Error::from(MF_E_INVALIDREQUEST))
        }
    }
}

/// `IMFSampleAllocatorControl` stub.
///
/// FrameServer may use this to hint that it wants to provide a sample
/// allocator. We report that we do not use an external allocator and
/// manage our own samples directly.
impl IMFSampleAllocatorControl_Impl for VCamMediaSource_Impl {
    fn SetDefaultAllocator(
        &self,
        _dwoutputstreamid: u32,
        _pallocator: windows_core::Ref<windows_core::IUnknown>,
    ) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFSampleAllocatorControl::SetDefaultAllocator");
        Ok(())
    }

    fn GetAllocatorUsage(
        &self,
        _dwoutputstreamid: u32,
        pdwinputstreamid: *mut u32,
        peusage: *mut MFSampleAllocatorUsage,
    ) -> windows_core::Result<()> {
        crate::trace::trace_method("IMFSampleAllocatorControl::GetAllocatorUsage");
        // Indicate that we do not use FrameServer-provided allocators.
        unsafe {
            if !pdwinputstreamid.is_null() {
                *pdwinputstreamid = 0;
            }
            if !peusage.is_null() {
                *peusage = MFSampleAllocatorUsage_DoesNotAllocate;
            }
        }
        Ok(())
    }
}

/// Stub IKsControl implementation.
///
/// FrameServer queries IKsControl on the media source; we expose the interface
/// but return ERROR_SET_NOT_FOUND for every call to signal no properties are
/// supported.
impl IKsControl_Impl for VCamMediaSource_Impl {
    fn KsProperty(
        &self,
        _property: *const KSIDENTIFIER,
        _propertylength: u32,
        _propertydata: *mut core::ffi::c_void,
        _datalength: u32,
        _bytesreturned: *mut u32,
    ) -> windows_core::Result<()> {
        crate::trace::trace_method("IKsControl::KsProperty");
        Err(windows_core::Error::from(HRESULT(0x80070490u32 as i32)))
    }

    fn KsMethod(
        &self,
        _method: *const KSIDENTIFIER,
        _methodlength: u32,
        _methoddata: *mut core::ffi::c_void,
        _datalength: u32,
        _bytesreturned: *mut u32,
    ) -> windows_core::Result<()> {
        crate::trace::trace_method("IKsControl::KsMethod");
        Err(windows_core::Error::from(HRESULT(0x80070490u32 as i32)))
    }

    fn KsEvent(
        &self,
        _event: *const KSIDENTIFIER,
        _eventlength: u32,
        _eventdata: *mut core::ffi::c_void,
        _datalength: u32,
        _bytesreturned: *mut u32,
    ) -> windows_core::Result<()> {
        crate::trace::trace_method("IKsControl::KsEvent");
        Err(windows_core::Error::from(HRESULT(0x80070490u32 as i32)))
    }
}

/// Create an NV12 `IMFMediaType` with the given dimensions and frame rate.
pub(crate) fn create_nv12_media_type(
    width: u32,
    height: u32,
    fps: u32,
) -> windows_core::Result<IMFMediaType> {
    let media_type: IMFMediaType = unsafe { MFCreateMediaType()? };

    unsafe {
        // Major type: Video.
        media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;

        // Subtype: NV12.
        media_type.SetGUID(&MF_MT_SUBTYPE, &MF_MT_SUBTYPE_NV12)?;

        // Frame size: pack width (high 32) and height (low 32) into u64.
        let frame_size = ((width as u64) << 32) | (height as u64);
        media_type.SetUINT64(&MF_MT_FRAME_SIZE, frame_size)?;

        // Frame rate: fps/1 packed as u64.
        let frame_rate = ((fps as u64) << 32) | 1u64;
        media_type.SetUINT64(&MF_MT_FRAME_RATE, frame_rate)?;

        // Progressive scan.
        media_type.SetUINT32(&MF_MT_INTERLACE_MODE, MF_VIDEO_INTERLACE_PROGRESSIVE)?;

        // All samples are independent (no B-frames).
        media_type.SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)?;
    }

    Ok(media_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nv12_media_type_has_correct_attributes() {
        let mt = create_nv12_media_type(1920, 1080, 30).unwrap();

        let major = unsafe { mt.GetGUID(&MF_MT_MAJOR_TYPE).unwrap() };
        assert_eq!(major, MFMediaType_Video);

        let sub = unsafe { mt.GetGUID(&MF_MT_SUBTYPE).unwrap() };
        assert_eq!(sub, MF_MT_SUBTYPE_NV12);

        let size = unsafe { mt.GetUINT64(&MF_MT_FRAME_SIZE).unwrap() };
        assert_eq!(size >> 32, 1920);
        assert_eq!(size & 0xFFFF_FFFF, 1080);

        let rate = unsafe { mt.GetUINT64(&MF_MT_FRAME_RATE).unwrap() };
        assert_eq!(rate >> 32, 30);
        assert_eq!(rate & 0xFFFF_FFFF, 1);

        let interlace = unsafe { mt.GetUINT32(&MF_MT_INTERLACE_MODE).unwrap() };
        assert_eq!(interlace, MF_VIDEO_INTERLACE_PROGRESSIVE);

        let independent = unsafe { mt.GetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT).unwrap() };
        assert_eq!(independent, 1);
    }

    #[test]
    fn media_type_640x480_15fps() {
        let mt = create_nv12_media_type(640, 480, 15).unwrap();

        let size = unsafe { mt.GetUINT64(&MF_MT_FRAME_SIZE).unwrap() };
        assert_eq!(size >> 32, 640);
        assert_eq!(size & 0xFFFF_FFFF, 480);

        let rate = unsafe { mt.GetUINT64(&MF_MT_FRAME_RATE).unwrap() };
        assert_eq!(rate >> 32, 15);
    }

    #[test]
    fn source_characteristics_is_live() {
        let source = VCamMediaSource::new().unwrap();
        let iface: IMFMediaSourceEx = source.into();
        let chars = unsafe { iface.GetCharacteristics().unwrap() };
        assert_eq!(chars, MFMEDIASOURCE_IS_LIVE.0 as u32);
    }

    #[test]
    fn source_creates_presentation_descriptor() {
        let source = VCamMediaSource::new().unwrap();
        let iface: IMFMediaSourceEx = source.into();
        let pd = unsafe { iface.CreatePresentationDescriptor().unwrap() };

        let count = unsafe { pd.GetStreamDescriptorCount().unwrap() };
        assert_eq!(count, 1);
    }

    #[test]
    fn source_shutdown_blocks_further_calls() {
        let source = VCamMediaSource::new().unwrap();
        let iface: IMFMediaSourceEx = source.into();

        unsafe { iface.Shutdown().unwrap() };

        let result = unsafe { iface.GetCharacteristics() };
        assert!(result.is_err());
    }

    #[test]
    fn source_attributes_default_to_vidcap() {
        let source = VCamMediaSource::new().unwrap();
        let iface: IMFMediaSourceEx = source.into();

        let attrs = unsafe { iface.GetSourceAttributes().unwrap() };

        // Defaults are set even with an empty attribute store.
        let source_type = unsafe { attrs.GetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE).unwrap() };
        assert_eq!(source_type, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID);

        let category = unsafe {
            attrs
                .GetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_CATEGORY)
                .unwrap()
        };
        assert_eq!(category, KSCATEGORY_VIDEO_CAMERA);
    }

    #[test]
    fn forwarded_attributes_visible_in_source() {
        // Simulate FrameServer setting attributes on the activate's store.
        let mut attrs: Option<IMFAttributes> = None;
        unsafe { MFCreateAttributes(&mut attrs, 4).unwrap() };
        let attrs = attrs.unwrap();

        // Set a custom u32 attribute to verify forwarding.
        let test_guid = GUID::from_u128(0xDEADBEEF_CAFE_BABE_0123_456789ABCDEF);
        unsafe { attrs.SetUINT32(&test_guid, 42).unwrap() };

        // Pre-set the required attributes (as FrameServer might).
        unsafe {
            attrs
                .SetGUID(
                    &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                    &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
                )
                .unwrap();
        }

        let source = VCamMediaSource::new_with_attributes(attrs).unwrap();
        let iface: IMFMediaSourceEx = source.into();

        let source_attrs = unsafe { iface.GetSourceAttributes().unwrap() };

        // Custom attribute should be forwarded.
        let val = unsafe { source_attrs.GetUINT32(&test_guid).unwrap() };
        assert_eq!(val, 42);

        // Required attributes should still be present.
        let source_type = unsafe {
            source_attrs
                .GetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE)
                .unwrap()
        };
        assert_eq!(source_type, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID);

        // Category should be filled in as default.
        let category = unsafe {
            source_attrs
                .GetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_CATEGORY)
                .unwrap()
        };
        assert_eq!(category, KSCATEGORY_VIDEO_CAMERA);
    }

    #[test]
    fn stream_attributes_has_required_fields() {
        let source = VCamMediaSource::new().unwrap();
        let iface: IMFMediaSourceEx = source.into();

        let attrs = unsafe { iface.GetStreamAttributes(0).unwrap() };

        let stream_id = unsafe { attrs.GetUINT32(&MF_DEVICESTREAM_STREAM_ID).unwrap() };
        assert_eq!(stream_id, 0);

        let category = unsafe { attrs.GetGUID(&MF_DEVICESTREAM_STREAM_CATEGORY).unwrap() };
        assert_eq!(category, PINNAME_VIDEO_CAPTURE);

        let shared = unsafe {
            attrs
                .GetUINT32(&MF_DEVICESTREAM_FRAMESERVER_SHARED)
                .unwrap()
        };
        assert_eq!(shared, 1);

        let frame_source = unsafe {
            attrs
                .GetUINT32(&MF_DEVICESTREAM_ATTRIBUTE_FRAMESOURCE_TYPES)
                .unwrap()
        };
        assert_eq!(frame_source, MFFrameSourceTypes_Color.0 as u32);
    }

    #[test]
    fn stream_attributes_rejects_invalid_stream_id() {
        let source = VCamMediaSource::new().unwrap();
        let iface: IMFMediaSourceEx = source.into();

        let result = unsafe { iface.GetStreamAttributes(1) };
        assert!(result.is_err());

        let result = unsafe { iface.GetStreamAttributes(5) };
        assert!(result.is_err());
    }

    #[test]
    fn stream_attributes_includes_max_frame_buffers() {
        let source = VCamMediaSource::new().unwrap();
        let iface: IMFMediaSourceEx = source.into();

        let attrs = unsafe { iface.GetStreamAttributes(0).unwrap() };

        let max_buffers = unsafe { attrs.GetUINT32(&MF_DEVICESTREAM_MAX_FRAME_BUFFERS).unwrap() };
        assert_eq!(max_buffers, 2);
    }

    #[test]
    fn presentation_descriptor_stream_has_media_type_hints() {
        let source = VCamMediaSource::new().unwrap();
        let iface: IMFMediaSourceEx = source.into();
        let pd = unsafe { iface.CreatePresentationDescriptor().unwrap() };

        let mut selected = BOOL(0);
        let mut stream_desc: Option<IMFStreamDescriptor> = None;
        unsafe {
            pd.GetStreamDescriptorByIndex(0, &mut selected, &mut stream_desc)
                .unwrap();
        }
        let stream_desc = stream_desc.unwrap();
        let attrs = stream_desc.cast::<IMFAttributes>().unwrap();

        // Media type hints must be present on the stream descriptor.
        let major = unsafe { attrs.GetGUID(&MF_MT_MAJOR_TYPE).unwrap() };
        assert_eq!(major, MFMediaType_Video);

        let sub = unsafe { attrs.GetGUID(&MF_MT_SUBTYPE).unwrap() };
        assert_eq!(sub, MF_MT_SUBTYPE_NV12);

        let frame_size = unsafe { attrs.GetUINT64(&MF_MT_FRAME_SIZE).unwrap() };
        assert_eq!(frame_size >> 32, DEFAULT_WIDTH as u64);
        assert_eq!(frame_size & 0xFFFF_FFFF, DEFAULT_HEIGHT as u64);

        let frame_rate = unsafe { attrs.GetUINT64(&MF_MT_FRAME_RATE).unwrap() };
        assert_eq!(frame_rate >> 32, TARGET_FPS as u64);
        assert_eq!(frame_rate & 0xFFFF_FFFF, 1);

        // MAX_FRAME_BUFFERS must also be set on the stream descriptor.
        let max_buffers = unsafe { attrs.GetUINT32(&MF_DEVICESTREAM_MAX_FRAME_BUFFERS).unwrap() };
        assert_eq!(max_buffers, 2);
    }
}
