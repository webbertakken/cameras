//! IMFMediaStream + IMFMediaEventGenerator implementation.
//!
//! Delivers `IMFSample` objects to FrameServer by reading NV12 frames from
//! shared memory. When `RequestSample` is called, we read the latest frame
//! from the `SharedMemoryReader` and queue an `MEMediaSample` event.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use windows::Win32::Foundation::{E_NOTIMPL, S_OK};
use windows::Win32::Media::MediaFoundation::{
    IMFAsyncCallback, IMFAsyncResult, IMFMediaEvent, IMFMediaEventGenerator,
    IMFMediaEventGenerator_Impl, IMFMediaEventQueue, IMFMediaSource, IMFMediaStream,
    IMFMediaStream_Impl, IMFStreamDescriptor, MEMediaSample, MEStreamStarted, MFCreateEventQueue,
    MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS, MF_E_SHUTDOWN,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows_core::{implement, IUnknown, Interface, Ref, GUID, HRESULT};

use crate::sample_factory::create_nv12_sample;
use crate::{decrement_object_count, increment_object_count, DEFAULT_HEIGHT, DEFAULT_WIDTH};

/// Media stream that delivers NV12 samples from shared memory.
#[implement(IMFMediaStream, IMFMediaEventGenerator)]
pub(crate) struct VCamMediaStream {
    event_queue: IMFMediaEventQueue,
    stream_descriptor: IMFStreamDescriptor,
    shutdown: AtomicBool,
    shared_mem_name: String,
    /// Last sequence number we delivered, to avoid re-delivering the same frame.
    last_sequence: Mutex<u64>,
}

impl VCamMediaStream {
    /// Create a new media stream.
    ///
    /// `shared_mem_name` is the named shared memory region to read frames from.
    pub(crate) fn new(
        stream_descriptor: &IMFStreamDescriptor,
        shared_mem_name: &str,
    ) -> windows_core::Result<Self> {
        let event_queue = unsafe { MFCreateEventQueue()? };
        increment_object_count();

        // Queue MEStreamStarted immediately so FrameServer knows we're ready.
        unsafe {
            event_queue.QueueEventParamVar(
                MEStreamStarted.0 as u32,
                &GUID::zeroed(),
                S_OK,
                std::ptr::null(),
            )?;
        }

        Ok(Self {
            event_queue,
            stream_descriptor: stream_descriptor.clone(),
            shutdown: AtomicBool::new(false),
            shared_mem_name: shared_mem_name.to_owned(),
            last_sequence: Mutex::new(0),
        })
    }

    fn check_shutdown(&self) -> windows_core::Result<()> {
        if self.shutdown.load(Ordering::Acquire) {
            Err(windows_core::Error::from(MF_E_SHUTDOWN))
        } else {
            Ok(())
        }
    }

    /// Try to read a frame from shared memory and deliver it as an IMFSample.
    fn deliver_sample(&self) -> windows_core::Result<()> {
        let (frame_data, width, height) = self.read_frame_or_black();

        let sample = create_nv12_sample(&frame_data, width, height)?;

        // Wrap the sample in a PROPVARIANT (VT_UNKNOWN).
        let unknown: IUnknown = sample.cast()?;
        let propvar = PROPVARIANT::from(unknown);

        unsafe {
            self.event_queue.QueueEventParamVar(
                MEMediaSample.0 as u32,
                &GUID::zeroed(),
                S_OK,
                &propvar,
            )?;
        }

        Ok(())
    }

    /// Read a frame from shared memory, or generate a black NV12 frame if
    /// shared memory is not available.
    fn read_frame_or_black(&self) -> (Vec<u8>, u32, u32) {
        #[cfg(windows)]
        if let Ok(reader) = vcam_shared::SharedMemoryReader::open(&self.shared_mem_name) {
            let header = reader.header();
            let width = header.width;
            let height = header.height;
            let seq = header.sequence.load(Ordering::Acquire);

            let mut last_seq = self.last_sequence.lock().unwrap();
            if seq > *last_seq {
                *last_seq = seq;
                if let Some(data) = reader.read_frame() {
                    return (data.to_vec(), width, height);
                }
            }
        }

        // Fall back to a black NV12 frame.
        generate_black_nv12(DEFAULT_WIDTH, DEFAULT_HEIGHT)
    }
}

impl Drop for VCamMediaStream {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        let _ = unsafe { self.event_queue.Shutdown() };
        decrement_object_count();
    }
}

impl IMFMediaStream_Impl for VCamMediaStream_Impl {
    fn GetMediaSource(&self) -> windows_core::Result<IMFMediaSource> {
        // FrameServer manages the source reference; we don't keep a back-pointer.
        Err(windows_core::Error::from(E_NOTIMPL))
    }

    fn GetStreamDescriptor(&self) -> windows_core::Result<IMFStreamDescriptor> {
        self.check_shutdown()?;
        Ok(self.stream_descriptor.clone())
    }

    fn RequestSample(&self, _ptoken: Ref<IUnknown>) -> windows_core::Result<()> {
        self.check_shutdown()?;
        self.deliver_sample()
    }
}

impl IMFMediaEventGenerator_Impl for VCamMediaStream_Impl {
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

/// Generate a black NV12 frame: Y=0x10 (16), UV=0x80 (128) for proper black
/// in BT.601 limited range.
pub(crate) fn generate_black_nv12(width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    let y_size = (width * height) as usize;
    let uv_size = (width * height / 2) as usize;
    let mut data = vec![0u8; y_size + uv_size];

    // Y plane: 16 (studio black in limited range).
    data[..y_size].fill(0x10);
    // UV plane: 128 (neutral chroma).
    data[y_size..].fill(0x80);

    (data, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_nv12_has_correct_size() {
        let (data, w, h) = generate_black_nv12(1920, 1080);
        let expected = (1920 * 1080 * 3 / 2) as usize;
        assert_eq!(data.len(), expected);
        assert_eq!(w, 1920);
        assert_eq!(h, 1080);
    }

    #[test]
    fn black_nv12_has_correct_values() {
        let (data, _, _) = generate_black_nv12(4, 2);
        // Y plane: 4*2 = 8 bytes, all 0x10.
        assert!(data[..8].iter().all(|&b| b == 0x10));
        // UV plane: 4*2/2 = 4 bytes, all 0x80.
        assert!(data[8..].iter().all(|&b| b == 0x80));
    }

    #[test]
    fn black_nv12_small_dimensions() {
        let (data, w, h) = generate_black_nv12(2, 2);
        assert_eq!(data.len(), 6); // 2*2 + 2*2/2 = 4 + 2
        assert_eq!(w, 2);
        assert_eq!(h, 2);
    }
}
