//! IMFMediaSource + IMFMediaEventGenerator implementation.
//!
//! This is the heart of the virtual camera COM DLL. FrameServer creates an
//! instance of this via IClassFactory, then drives it through the standard
//! media source lifecycle: GetCharacteristics → CreatePresentationDescriptor →
//! Start → (frame delivery) → Stop → Shutdown.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use windows::Win32::Foundation::{E_UNEXPECTED, S_OK};
use windows::Win32::Media::MediaFoundation::{
    IMFAsyncCallback, IMFAsyncResult, IMFMediaEvent, IMFMediaEventGenerator,
    IMFMediaEventGenerator_Impl, IMFMediaEventQueue, IMFMediaSource, IMFMediaSource_Impl,
    IMFMediaStream, IMFMediaType, IMFPresentationDescriptor, IMFStreamDescriptor, MENewStream,
    MESourceStarted, MESourceStopped, MFCreateEventQueue, MFCreateMediaType,
    MFCreatePresentationDescriptor, MFCreateStreamDescriptor, MFMediaType_Video,
    MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS, MFMEDIASOURCE_IS_LIVE, MF_E_SHUTDOWN,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows_core::{implement, IUnknown, Interface, Ref, BOOL, GUID, HRESULT};

use crate::media_stream::VCamMediaStream;
use crate::{
    decrement_object_count, increment_object_count, DEFAULT_HEIGHT, DEFAULT_WIDTH,
    SHARED_MEMORY_NAME, TARGET_FPS,
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
/// Implements IMFMediaSource + IMFMediaEventGenerator. IKsControl is omitted
/// for now — FrameServer can still use the source without property negotiation.
#[implement(IMFMediaSource, IMFMediaEventGenerator)]
pub(crate) struct VCamMediaSource {
    event_queue: IMFMediaEventQueue,
    state: Mutex<SourceState>,
    stream: Mutex<Option<IMFMediaStream>>,
    shutdown: AtomicBool,
    shared_mem_name: Mutex<String>,
}

impl VCamMediaSource {
    /// Create a new media source instance.
    pub(crate) fn new() -> windows_core::Result<Self> {
        let event_queue = unsafe { MFCreateEventQueue()? };
        increment_object_count();
        Ok(Self {
            event_queue,
            state: Mutex::new(SourceState::Stopped),
            stream: Mutex::new(None),
            shutdown: AtomicBool::new(false),
            shared_mem_name: Mutex::new(SHARED_MEMORY_NAME.to_owned()),
        })
    }

    fn check_shutdown(&self) -> windows_core::Result<()> {
        if self.shutdown.load(Ordering::Acquire) {
            Err(windows_core::Error::from(MF_E_SHUTDOWN))
        } else {
            Ok(())
        }
    }
}

impl Drop for VCamMediaSource {
    fn drop(&mut self) {
        decrement_object_count();
    }
}

impl IMFMediaSource_Impl for VCamMediaSource_Impl {
    fn GetCharacteristics(&self) -> windows_core::Result<u32> {
        self.check_shutdown()?;
        Ok(MFMEDIASOURCE_IS_LIVE.0 as u32)
    }

    fn CreatePresentationDescriptor(&self) -> windows_core::Result<IMFPresentationDescriptor> {
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

        let shared_mem_name = self.shared_mem_name.lock().unwrap().clone();

        // Create and store the media stream.
        let stream = VCamMediaStream::new(&stream_desc, &shared_mem_name)?;
        let stream_iface: IMFMediaStream = stream.into();

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
        self.shutdown.store(true, Ordering::Release);

        {
            let mut state = self.state.lock().unwrap();
            *state = SourceState::Shutdown;
        }

        // Drop the stream.
        if let Some(stream) = self.stream.lock().unwrap().take() {
            drop(stream);
        }

        // Shut down the event queue.
        unsafe { self.event_queue.Shutdown()? };

        Ok(())
    }

    fn Pause(&self) -> windows_core::Result<()> {
        Err(windows_core::Error::from(E_UNEXPECTED))
    }
}

impl IMFMediaEventGenerator_Impl for VCamMediaSource_Impl {
    fn GetEvent(
        &self,
        dwflags: MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS,
    ) -> windows_core::Result<IMFMediaEvent> {
        self.check_shutdown()?;
        unsafe { self.event_queue.GetEvent(dwflags.0) }
    }

    fn BeginGetEvent(
        &self,
        pcallback: Ref<IMFAsyncCallback>,
        punkstate: Ref<IUnknown>,
    ) -> windows_core::Result<()> {
        self.check_shutdown()?;
        unsafe {
            self.event_queue
                .BeginGetEvent(pcallback.as_ref(), punkstate.as_ref())
        }
    }

    fn EndGetEvent(&self, presult: Ref<IMFAsyncResult>) -> windows_core::Result<IMFMediaEvent> {
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
        self.check_shutdown()?;
        unsafe {
            self.event_queue
                .QueueEventParamVar(met, guidextendedtype, hrstatus, pvvalue)
        }
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
        let iface: IMFMediaSource = source.into();
        let chars = unsafe { iface.GetCharacteristics().unwrap() };
        assert_eq!(chars, MFMEDIASOURCE_IS_LIVE.0 as u32);
    }

    #[test]
    fn source_creates_presentation_descriptor() {
        let source = VCamMediaSource::new().unwrap();
        let iface: IMFMediaSource = source.into();
        let pd = unsafe { iface.CreatePresentationDescriptor().unwrap() };

        let count = unsafe { pd.GetStreamDescriptorCount().unwrap() };
        assert_eq!(count, 1);
    }

    #[test]
    fn source_shutdown_blocks_further_calls() {
        let source = VCamMediaSource::new().unwrap();
        let iface: IMFMediaSource = source.into();

        unsafe { iface.Shutdown().unwrap() };

        let result = unsafe { iface.GetCharacteristics() };
        assert!(result.is_err());
    }
}
