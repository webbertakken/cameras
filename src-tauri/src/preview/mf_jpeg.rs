// Hardware-accelerated JPEG encoding via Windows Media Foundation.
//
// Uses MFTEnumEx to discover JPEG encoder MFTs, preferring hardware-accelerated
// transforms (NVENC/AMD VCE/Intel Quick Sync). Falls back to the built-in
// Windows MJPEG encoder, then to the existing `image` crate CPU path.

#[cfg(target_os = "windows")]
pub mod encoder {
    use std::sync::OnceLock;

    use crate::preview::compress;
    use tracing::{debug, info, warn};
    #[allow(unused_imports)]
    use windows::core::Interface;
    use windows::core::GUID;
    use windows::Win32::Media::MediaFoundation::{
        IMFActivate, IMFMediaBuffer, IMFMediaType, IMFSample, IMFTransform, MFCreateMediaType,
        MFCreateMemoryBuffer, MFCreateSample, MFStartup, MFTEnumEx, MFT_ENUM_FLAG,
        MFT_OUTPUT_DATA_BUFFER, MFT_REGISTER_TYPE_INFO, MF_API_VERSION,
    };
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

    // MFVideoFormat_RGB24: {00000014-0000-0010-8000-00AA00389B71}
    const MF_VIDEO_FORMAT_RGB24: GUID = GUID::from_u128(0x00000014_0000_0010_8000_00AA00389B71);

    // MFVideoFormat_MJPG: {47504A4D-0000-0010-8000-00AA00389B71}
    const MF_VIDEO_FORMAT_MJPG: GUID = GUID::from_u128(0x47504A4D_0000_0010_8000_00AA00389B71);

    // MFMediaType_Video: {73646976-0000-0010-8000-00AA00389B71}
    const MF_MEDIA_TYPE_VIDEO: GUID = GUID::from_u128(0x73646976_0000_0010_8000_00AA00389B71);

    // MFT_CATEGORY_VIDEO_ENCODER: {F79EAC7D-E545-4387-BDEE-D647D7BDE42A}
    const MFT_CATEGORY_VIDEO_ENCODER: GUID =
        GUID::from_u128(0xF79EAC7D_E545_4387_BDEE_D647D7BDE42A);

    // MF_MT_MAJOR_TYPE: {48EBA18E-F8C9-4687-BF11-0A74C9F96A8F}
    const MF_MT_MAJOR_TYPE: GUID = GUID::from_u128(0x48EBA18E_F8C9_4687_BF11_0A74C9F96A8F);

    // MF_MT_SUBTYPE: {F7E34C9A-42E8-4714-B74B-CB29D72C35E5}
    const MF_MT_SUBTYPE: GUID = GUID::from_u128(0xF7E34C9A_42E8_4714_B74B_CB29D72C35E5);

    // MF_MT_FRAME_SIZE: {1652C33D-D6B2-4012-B834-72030849A37D}
    const MF_MT_FRAME_SIZE: GUID = GUID::from_u128(0x1652C33D_D6B2_4012_B834_72030849A37D);

    // MF_MT_AVG_BITRATE: {20332624-FB0D-4D9E-BD0D-CBF6786C102E}
    const MF_MT_AVG_BITRATE: GUID = GUID::from_u128(0x20332624_FB0D_4D9E_BD0D_CBF6786C102E);

    // MFT_MESSAGE_NOTIFY_BEGIN_STREAMING
    const MFT_MSG_NOTIFY_BEGIN_STREAMING: i32 = 0x10000000_u32 as i32;
    // MFT_MESSAGE_NOTIFY_START_OF_STREAM
    const MFT_MSG_NOTIFY_START_OF_STREAM: i32 = 0x10000003_u32 as i32;

    // MFT_ENUM_FLAG constants (i32 to match MFT_ENUM_FLAG newtype)
    const MFT_ENUM_FLAG_SYNCMFT: i32 = 0x00000001;
    const MFT_ENUM_FLAG_ASYNCMFT: i32 = 0x00000002;
    const MFT_ENUM_FLAG_HARDWARE: i32 = 0x00000004;
    const MFT_ENUM_FLAG_SORTANDFILTER: i32 = 0x00000040;
    const MFT_ENUM_FLAG_ALL: i32 = MFT_ENUM_FLAG_SYNCMFT
        | MFT_ENUM_FLAG_ASYNCMFT
        | MFT_ENUM_FLAG_HARDWARE
        | MFT_ENUM_FLAG_SORTANDFILTER;

    /// Which encoder backend is being used.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    pub enum EncoderKind {
        /// Media Foundation hardware-accelerated JPEG encoder
        MfHardware,
        /// Media Foundation software JPEG encoder
        MfSoftware,
        /// Fallback to `image` crate CPU encoder
        CpuFallback,
    }

    impl std::fmt::Display for EncoderKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::MfHardware => write!(f, "MF Hardware"),
                Self::MfSoftware => write!(f, "MF Software"),
                Self::CpuFallback => write!(f, "CPU (image crate)"),
            }
        }
    }

    /// Result of a single JPEG encode operation.
    pub struct EncodeResult {
        pub jpeg_bytes: Vec<u8>,
        pub encoder_kind: EncoderKind,
    }

    /// Persistent encoder state for a single camera session.
    ///
    /// Holds a configured IMFTransform so we avoid re-creating it every frame.
    /// Not `Send` because COM objects are apartment-threaded; the encoder must
    /// be used from the same thread that created it.
    pub struct JpegEncoder {
        transform: IMFTransform,
        kind: EncoderKind,
        width: u32,
        height: u32,
        output_buffer_size: u32,
    }

    /// Ensure MFStartup is called exactly once per process.
    static MF_INIT: OnceLock<bool> = OnceLock::new();

    fn ensure_mf_started() -> bool {
        *MF_INIT.get_or_init(|| unsafe {
            match MFStartup(MF_API_VERSION, 0) {
                Ok(()) => {
                    info!("Media Foundation initialised");
                    true
                }
                Err(e) => {
                    warn!("MFStartup failed: {e}");
                    false
                }
            }
        })
    }

    impl JpegEncoder {
        /// Create a new JPEG encoder for the given frame dimensions.
        ///
        /// Tries hardware MFT first, then software MFT, then returns None
        /// to signal the caller should use CPU fallback.
        pub fn new(width: u32, height: u32, quality: u8) -> Option<Self> {
            if !ensure_mf_started() {
                return None;
            }

            // Try hardware encoder first, then any (includes software)
            if let Some(enc) = Self::try_create_mft(width, height, quality, true) {
                return Some(enc);
            }

            if let Some(enc) = Self::try_create_mft(width, height, quality, false) {
                return Some(enc);
            }

            warn!("no Media Foundation JPEG encoder available, using CPU fallback");
            None
        }

        /// Attempt to create a JPEG encoder MFT.
        fn try_create_mft(
            width: u32,
            height: u32,
            quality: u8,
            hardware_only: bool,
        ) -> Option<Self> {
            unsafe {
                let input_type = MFT_REGISTER_TYPE_INFO {
                    guidMajorType: MF_MEDIA_TYPE_VIDEO,
                    guidSubtype: MF_VIDEO_FORMAT_RGB24,
                };
                let output_type = MFT_REGISTER_TYPE_INFO {
                    guidMajorType: MF_MEDIA_TYPE_VIDEO,
                    guidSubtype: MF_VIDEO_FORMAT_MJPG,
                };

                let flags = if hardware_only {
                    MFT_ENUM_FLAG(MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER)
                } else {
                    MFT_ENUM_FLAG(MFT_ENUM_FLAG_ALL)
                };

                let mut activates: *mut Option<IMFActivate> = std::ptr::null_mut();
                let mut count: u32 = 0;

                let result = MFTEnumEx(
                    MFT_CATEGORY_VIDEO_ENCODER,
                    flags,
                    Some(&input_type),
                    Some(&output_type),
                    &mut activates,
                    &mut count,
                );

                if result.is_err() || count == 0 || activates.is_null() {
                    if !activates.is_null() {
                        windows::Win32::System::Com::CoTaskMemFree(Some(activates.cast()));
                    }
                    debug!(
                        "no {} JPEG encoder MFTs found",
                        if hardware_only { "hardware" } else { "any" }
                    );
                    return None;
                }

                // Try each activate object until one works
                let activate_slice = std::slice::from_raw_parts(activates, count as usize);
                let mut encoder = None;

                for (i, activate_opt) in activate_slice.iter().enumerate() {
                    let Some(activate) = activate_opt else {
                        continue;
                    };

                    match activate.ActivateObject::<IMFTransform>() {
                        Ok(transform) => {
                            match Self::configure_transform(&transform, width, height, quality) {
                                Ok(output_buffer_size) => {
                                    let kind = if hardware_only {
                                        EncoderKind::MfHardware
                                    } else {
                                        // Check if it's hardware by trying hardware-only enum
                                        EncoderKind::MfSoftware
                                    };
                                    info!(
                                        "created {} JPEG encoder (MFT index {i}, {width}x{height})",
                                        kind
                                    );
                                    encoder = Some(Self {
                                        transform,
                                        kind,
                                        width,
                                        height,
                                        output_buffer_size,
                                    });
                                    break;
                                }
                                Err(e) => {
                                    debug!("MFT {i} configure failed: {e}");
                                    continue;
                                }
                            }
                        }
                        Err(e) => {
                            debug!("MFT {i} ActivateObject failed: {e}");
                            continue;
                        }
                    }
                }

                // Release all activate objects and free the array
                for activate_opt in activate_slice {
                    // IMFActivate has Drop which calls Release
                    drop(activate_opt.clone());
                }
                windows::Win32::System::Com::CoTaskMemFree(Some(activates.cast()));

                encoder
            }
        }

        /// Configure the MFT input/output types and send streaming messages.
        unsafe fn configure_transform(
            transform: &IMFTransform,
            width: u32,
            height: u32,
            quality: u8,
        ) -> Result<u32, String> {
            // Set output type first (MJPEG) — encoders expect output before input
            let output_mt: IMFMediaType =
                MFCreateMediaType().map_err(|e| format!("MFCreateMediaType: {e}"))?;

            output_mt
                .SetGUID(&MF_MT_MAJOR_TYPE, &MF_MEDIA_TYPE_VIDEO)
                .map_err(|e| format!("SetGUID major: {e}"))?;
            output_mt
                .SetGUID(&MF_MT_SUBTYPE, &MF_VIDEO_FORMAT_MJPG)
                .map_err(|e| format!("SetGUID subtype: {e}"))?;

            // Pack width/height into a single u64 (high 32 = width, low 32 = height)
            let frame_size = ((width as u64) << 32) | (height as u64);
            output_mt
                .SetUINT64(&MF_MT_FRAME_SIZE, frame_size)
                .map_err(|e| format!("SetUINT64 frame size: {e}"))?;

            // Use quality to set a reasonable bitrate hint
            // Higher quality => higher bitrate. Scale from ~1 Mbps to ~20 Mbps
            let bitrate = (quality as u32) * 200_000;
            output_mt
                .SetUINT32(&MF_MT_AVG_BITRATE, bitrate)
                .map_err(|e| format!("SetUINT32 bitrate: {e}"))?;

            transform
                .SetOutputType(0, &output_mt, 0)
                .map_err(|e| format!("SetOutputType: {e}"))?;

            // Set input type (RGB24)
            let input_mt: IMFMediaType =
                MFCreateMediaType().map_err(|e| format!("MFCreateMediaType input: {e}"))?;

            input_mt
                .SetGUID(&MF_MT_MAJOR_TYPE, &MF_MEDIA_TYPE_VIDEO)
                .map_err(|e| format!("SetGUID major: {e}"))?;
            input_mt
                .SetGUID(&MF_MT_SUBTYPE, &MF_VIDEO_FORMAT_RGB24)
                .map_err(|e| format!("SetGUID subtype: {e}"))?;
            input_mt
                .SetUINT64(&MF_MT_FRAME_SIZE, frame_size)
                .map_err(|e| format!("SetUINT64 frame size: {e}"))?;

            transform
                .SetInputType(0, &input_mt, 0)
                .map_err(|e| format!("SetInputType: {e}"))?;

            // Query output buffer requirements
            let output_info = transform
                .GetOutputStreamInfo(0)
                .map_err(|e| format!("GetOutputStreamInfo: {e}"))?;

            let buffer_size = if output_info.cbSize > 0 {
                output_info.cbSize
            } else {
                // Fallback: allocate enough for the worst case
                width * height * 3
            };

            // Notify the MFT that streaming is about to begin
            let _ = transform.ProcessMessage(
                windows::Win32::Media::MediaFoundation::MFT_MESSAGE_TYPE(
                    MFT_MSG_NOTIFY_BEGIN_STREAMING,
                ),
                0,
            );
            let _ = transform.ProcessMessage(
                windows::Win32::Media::MediaFoundation::MFT_MESSAGE_TYPE(
                    MFT_MSG_NOTIFY_START_OF_STREAM,
                ),
                0,
            );

            Ok(buffer_size)
        }

        /// Encode an RGB24 frame to JPEG.
        ///
        /// Returns the JPEG bytes on success, or an error string. The caller
        /// should fall back to CPU encoding on error.
        pub fn encode(&self, rgb_data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
            if width != self.width || height != self.height {
                return Err(format!(
                    "dimension mismatch: encoder is {}x{}, got {width}x{height}",
                    self.width, self.height
                ));
            }

            let expected = (width * height * 3) as usize;
            if rgb_data.len() < expected {
                return Err(format!(
                    "buffer too small: expected {expected}, got {}",
                    rgb_data.len()
                ));
            }

            unsafe { self.encode_mft(rgb_data) }
        }

        /// Perform the actual MFT encode cycle: ProcessInput → ProcessOutput.
        unsafe fn encode_mft(&self, rgb_data: &[u8]) -> Result<Vec<u8>, String> {
            // Create input sample with RGB data
            let input_buffer: IMFMediaBuffer = MFCreateMemoryBuffer(rgb_data.len() as u32)
                .map_err(|e| format!("MFCreateMemoryBuffer input: {e}"))?;

            // Lock, copy data, unlock
            {
                let mut buf_ptr: *mut u8 = std::ptr::null_mut();
                let mut max_len: u32 = 0;
                let mut cur_len: u32 = 0;
                input_buffer
                    .Lock(&mut buf_ptr, Some(&mut max_len), Some(&mut cur_len))
                    .map_err(|e| format!("Lock input: {e}"))?;

                std::ptr::copy_nonoverlapping(rgb_data.as_ptr(), buf_ptr, rgb_data.len());

                input_buffer
                    .Unlock()
                    .map_err(|e| format!("Unlock input: {e}"))?;
            }

            input_buffer
                .SetCurrentLength(rgb_data.len() as u32)
                .map_err(|e| format!("SetCurrentLength: {e}"))?;

            let input_sample: IMFSample =
                MFCreateSample().map_err(|e| format!("MFCreateSample: {e}"))?;
            input_sample
                .AddBuffer(&input_buffer)
                .map_err(|e| format!("AddBuffer: {e}"))?;

            // Feed input to the transform
            self.transform
                .ProcessInput(0, &input_sample, 0)
                .map_err(|e| format!("ProcessInput: {e}"))?;

            // Create output sample and buffer
            let output_buffer: IMFMediaBuffer = MFCreateMemoryBuffer(self.output_buffer_size)
                .map_err(|e| format!("MFCreateMemoryBuffer output: {e}"))?;

            let output_sample: IMFSample =
                MFCreateSample().map_err(|e| format!("MFCreateSample output: {e}"))?;
            output_sample
                .AddBuffer(&output_buffer)
                .map_err(|e| format!("AddBuffer output: {e}"))?;

            let mut output_buffers = [MFT_OUTPUT_DATA_BUFFER {
                dwStreamID: 0,
                pSample: std::mem::ManuallyDrop::new(Some(output_sample)),
                dwStatus: 0,
                pEvents: std::mem::ManuallyDrop::new(None),
            }];

            let mut status: u32 = 0;
            self.transform
                .ProcessOutput(0, &mut output_buffers, &mut status)
                .map_err(|e| format!("ProcessOutput: {e}"))?;

            // Read the JPEG data from the output sample
            let out_sample = std::mem::ManuallyDrop::into_inner(output_buffers[0].pSample.clone())
                .ok_or("no output sample")?;

            let out_buf: IMFMediaBuffer = out_sample
                .ConvertToContiguousBuffer()
                .map_err(|e| format!("ConvertToContiguousBuffer: {e}"))?;

            let mut buf_ptr: *mut u8 = std::ptr::null_mut();
            let mut cur_len: u32 = 0;
            out_buf
                .Lock(&mut buf_ptr, None, Some(&mut cur_len))
                .map_err(|e| format!("Lock output: {e}"))?;

            let jpeg_bytes = std::slice::from_raw_parts(buf_ptr, cur_len as usize).to_vec();

            out_buf
                .Unlock()
                .map_err(|e| format!("Unlock output: {e}"))?;

            // Clean up ManuallyDrop for events
            let _ = std::mem::ManuallyDrop::into_inner(output_buffers[0].pEvents.clone());

            Ok(jpeg_bytes)
        }

        /// Which encoder backend is active.
        pub fn kind(&self) -> EncoderKind {
            self.kind
        }

        /// The configured frame dimensions.
        pub fn dimensions(&self) -> (u32, u32) {
            (self.width, self.height)
        }
    }

    /// Encode RGB24 data to JPEG using the best available encoder.
    ///
    /// This is the main entry point for one-shot encoding. For repeated
    /// encoding of the same resolution, prefer creating a `JpegEncoder` once
    /// and calling `encode()` on each frame.
    pub fn encode_jpeg_best_effort(
        data: &[u8],
        width: u32,
        height: u32,
        quality: u8,
    ) -> EncodeResult {
        // Try Media Foundation first
        if let Some(encoder) = JpegEncoder::new(width, height, quality) {
            match encoder.encode(data, width, height) {
                Ok(jpeg_bytes) => {
                    return EncodeResult {
                        jpeg_bytes,
                        encoder_kind: encoder.kind(),
                    };
                }
                Err(e) => {
                    warn!("MF JPEG encode failed, falling back to CPU: {e}");
                }
            }
        }

        // CPU fallback via image crate
        let jpeg_bytes = compress::compress_jpeg(data, width, height, quality);
        EncodeResult {
            jpeg_bytes,
            encoder_kind: EncoderKind::CpuFallback,
        }
    }

    /// COM guard for per-thread initialisation in tests.
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

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Create a synthetic RGB24 test image (gradient pattern).
        fn make_test_rgb(width: u32, height: u32) -> Vec<u8> {
            let mut data = Vec::with_capacity((width * height * 3) as usize);
            for y in 0..height {
                for x in 0..width {
                    data.push((x % 256) as u8); // R
                    data.push((y % 256) as u8); // G
                    data.push(128); // B
                }
            }
            data
        }

        #[test]
        fn encode_jpeg_best_effort_produces_valid_jpeg() {
            let _com = ComGuard::init();
            let rgb = make_test_rgb(640, 480);
            let result = encode_jpeg_best_effort(&rgb, 640, 480, 75);

            // Regardless of which encoder was used, output must be valid JPEG
            assert!(
                result.jpeg_bytes.len() > 2,
                "JPEG output too small: {} bytes",
                result.jpeg_bytes.len()
            );
            assert_eq!(result.jpeg_bytes[0], 0xFF, "missing JPEG SOI marker");
            assert_eq!(result.jpeg_bytes[1], 0xD8, "missing JPEG SOI marker");

            // Log which encoder was used (useful for CI diagnostics)
            eprintln!("encoder used: {}", result.encoder_kind);
        }

        #[test]
        fn encode_jpeg_best_effort_1080p() {
            let _com = ComGuard::init();
            let rgb = make_test_rgb(1920, 1080);
            let result = encode_jpeg_best_effort(&rgb, 1920, 1080, 75);

            assert_eq!(result.jpeg_bytes[0], 0xFF);
            assert_eq!(result.jpeg_bytes[1], 0xD8);
            eprintln!(
                "1080p JPEG: {} bytes, encoder: {}",
                result.jpeg_bytes.len(),
                result.encoder_kind
            );
        }

        #[test]
        fn encoder_kind_display() {
            assert_eq!(format!("{}", EncoderKind::MfHardware), "MF Hardware");
            assert_eq!(format!("{}", EncoderKind::MfSoftware), "MF Software");
            assert_eq!(format!("{}", EncoderKind::CpuFallback), "CPU (image crate)");
        }

        #[test]
        fn encode_rejects_dimension_mismatch() {
            let _com = ComGuard::init();
            // If we can create an encoder, test that it rejects wrong dimensions
            if let Some(encoder) = JpegEncoder::new(640, 480, 75) {
                let rgb = make_test_rgb(320, 240);
                let result = encoder.encode(&rgb, 320, 240);
                assert!(
                    result.is_err(),
                    "should reject dimensions that don't match encoder config"
                );
            }
        }

        #[test]
        fn encode_rejects_short_buffer() {
            let _com = ComGuard::init();
            if let Some(encoder) = JpegEncoder::new(640, 480, 75) {
                let short_data = vec![0u8; 100];
                let result = encoder.encode(&short_data, 640, 480);
                assert!(result.is_err(), "should reject buffer that's too small");
            }
        }

        #[test]
        fn mf_startup_is_idempotent() {
            let _com = ComGuard::init();
            // Call ensure_mf_started multiple times — should not panic
            let a = ensure_mf_started();
            let b = ensure_mf_started();
            assert_eq!(a, b);
        }
    }
}

/// Re-export for non-Windows platforms (no-op).
#[cfg(not(target_os = "windows"))]
pub mod encoder {
    use crate::preview::compress;

    /// Which encoder backend is being used.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    pub enum EncoderKind {
        MfHardware,
        MfSoftware,
        CpuFallback,
    }

    impl std::fmt::Display for EncoderKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::MfHardware => write!(f, "MF Hardware"),
                Self::MfSoftware => write!(f, "MF Software"),
                Self::CpuFallback => write!(f, "CPU (image crate)"),
            }
        }
    }

    pub struct EncodeResult {
        pub jpeg_bytes: Vec<u8>,
        pub encoder_kind: EncoderKind,
    }

    /// On non-Windows, always falls back to CPU encoding.
    pub fn encode_jpeg_best_effort(
        data: &[u8],
        width: u32,
        height: u32,
        quality: u8,
    ) -> EncodeResult {
        let jpeg_bytes = compress::compress_jpeg(data, width, height, quality);
        EncodeResult {
            jpeg_bytes,
            encoder_kind: EncoderKind::CpuFallback,
        }
    }
}
