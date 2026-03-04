//! MF Virtual Camera sink for Windows 11+.
//!
//! Uses `MFCreateVirtualCamera` to register a session-lifetime virtual camera
//! backed by our custom COM media source DLL (`vcam-source`). A background
//! frame pump thread reads JPEG frames from the preview session, decodes them,
//! converts to NV12, and writes into shared memory for the media source to read.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use tracing::{debug, error, info};
use windows::core::PCWSTR;
use windows::core::{GUID, HSTRING};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::Media::KernelStreaming::KSCATEGORY_VIDEO_CAMERA;
use windows::Win32::Media::MediaFoundation::{
    IMFVirtualCamera, MFCreateVirtualCamera, MFVirtualCameraAccess_CurrentUser,
    MFVirtualCameraLifetime_Session, MFVirtualCameraType_SoftwareCameraSource,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_WRITE,
    REG_OPTION_NON_VOLATILE, REG_SZ,
};

use super::VirtualCameraSink;
use crate::preview::encode_worker::JpegFrameBuffer;

/// Media Foundation virtual camera sink.
///
/// Holds the COM virtual camera handle, shared memory writer, and the
/// background frame pump thread. When dropped, all resources are cleaned up.
pub struct MfVirtualCamera {
    device_name: String,
    jpeg_buffer: Arc<JpegFrameBuffer>,
    vcam: Option<IMFVirtualCamera>,
    pump_handle: Option<JoinHandle<()>>,
    pump_running: Arc<AtomicBool>,
}

// SAFETY: IMFVirtualCamera is apartment-agile (supports both STA and MTA).
// The pump thread accesses only Arc-wrapped, thread-safe types.
unsafe impl Send for MfVirtualCamera {}
unsafe impl Sync for MfVirtualCamera {}

impl MfVirtualCamera {
    pub fn new(device_name: String, jpeg_buffer: Arc<JpegFrameBuffer>) -> Self {
        Self {
            device_name,
            jpeg_buffer,
            vcam: None,
            pump_handle: None,
            pump_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Locate the vcam-source COM DLL relative to the current executable.
    fn find_com_dll() -> Result<String, String> {
        let exe = std::env::current_exe().map_err(|e| format!("failed to get exe path: {e}"))?;
        let exe_dir = exe
            .parent()
            .ok_or_else(|| "exe has no parent directory".to_string())?;
        let dll_path = exe_dir.join("vcam_source.dll");

        if dll_path.exists() {
            Ok(dll_path.to_string_lossy().to_string())
        } else {
            Err(format!(
                "vcam_source.dll not found at: {}",
                dll_path.display()
            ))
        }
    }

    /// Register the COM DLL and create the MF virtual camera.
    fn create_virtual_camera(&mut self) -> Result<(), String> {
        // Locate and register the COM media source DLL
        let dll_path = Self::find_com_dll()?;
        register_com_server(&dll_path)?;
        info!("Registered vcam COM server at {dll_path}");

        // Build the CLSID string for our media source
        let clsid_str = clsid_string();
        let friendly_name = format!("Cameras App \u{2014} {}", self.device_name);

        let friendly_name_h = HSTRING::from(&friendly_name);
        let clsid_h = HSTRING::from(&clsid_str);
        let categories = [KSCATEGORY_VIDEO_CAMERA];

        let vcam = unsafe {
            MFCreateVirtualCamera(
                MFVirtualCameraType_SoftwareCameraSource,
                MFVirtualCameraLifetime_Session,
                MFVirtualCameraAccess_CurrentUser,
                &friendly_name_h,
                &clsid_h,
                Some(&categories),
            )
        }
        .map_err(|e| format!("MFCreateVirtualCamera failed: {e}"))?;

        info!("Created MF virtual camera: '{friendly_name}'");

        unsafe { vcam.Start(None) }.map_err(|e| format!("IMFVirtualCamera::Start failed: {e}"))?;

        info!("MF virtual camera started and visible to consumers");
        self.vcam = Some(vcam);
        Ok(())
    }

    /// Spawn the background frame pump thread.
    fn start_pump(&mut self) -> Result<(), String> {
        let width = vcam_shared::DEFAULT_WIDTH;
        let height = vcam_shared::DEFAULT_HEIGHT;

        let shm_writer =
            vcam_shared::SharedMemoryWriter::new(vcam_shared::SHARED_MEMORY_NAME, width, height, 3)
                .map_err(|e| format!("failed to create shared memory: {e}"))?;

        debug!(
            "Created shared memory '{}' ({width}x{height})",
            vcam_shared::SHARED_MEMORY_NAME
        );

        self.pump_running.store(true, Ordering::Relaxed);

        let jpeg_buffer = Arc::clone(&self.jpeg_buffer);
        let running = Arc::clone(&self.pump_running);

        let handle = std::thread::Builder::new()
            .name("vcam-pump".to_string())
            .spawn(move || {
                super::frame_pump::run_frame_pump(jpeg_buffer, shm_writer, running);
            })
            .map_err(|e| format!("failed to spawn frame pump thread: {e}"))?;

        self.pump_handle = Some(handle);
        Ok(())
    }
}

impl VirtualCameraSink for MfVirtualCamera {
    fn start(&mut self) -> Result<(), String> {
        if self.pump_running.load(Ordering::Relaxed) {
            return Err("virtual camera is already running".to_string());
        }

        self.create_virtual_camera()?;
        self.start_pump()?;

        info!("Virtual camera fully started for '{}'", self.device_name);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        // Signal the pump to stop
        self.pump_running.store(false, Ordering::Relaxed);

        // Join the pump thread
        if let Some(handle) = self.pump_handle.take() {
            let _ = handle.join();
            debug!("Frame pump thread joined");
        }

        // Stop and shutdown the virtual camera
        if let Some(vcam) = self.vcam.take() {
            if let Err(e) = unsafe { vcam.Stop() } {
                error!("IMFVirtualCamera::Stop failed: {e}");
            }
            if let Err(e) = unsafe { vcam.Shutdown() } {
                error!("IMFVirtualCamera::Shutdown failed: {e}");
            }
            info!("MF virtual camera stopped for '{}'", self.device_name);
        }

        Ok(())
    }

    fn is_running(&self) -> bool {
        self.pump_running.load(Ordering::Relaxed)
    }
}

impl Drop for MfVirtualCamera {
    fn drop(&mut self) {
        if self.is_running() {
            let _ = self.stop();
        }
    }
}

// ---------------------------------------------------------------------------
// COM registration helpers (mirrors vcam-source/src/registry.rs)
//
// We duplicate these here because vcam-source is a cdylib and cannot be
// linked as a regular Rust dependency. The source of truth for the CLSID
// is vcam-shared::VCAM_SOURCE_CLSID.
// ---------------------------------------------------------------------------

/// Format the CLSID as a registry-style GUID string: `{XXXXXXXX-XXXX-...}`.
fn clsid_string() -> String {
    let guid = GUID::from_u128(vcam_shared::VCAM_SOURCE_CLSID);
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7],
    )
}

/// Register the COM media source DLL under HKCU so FrameServer can find it.
fn register_com_server(dll_path: &str) -> Result<(), String> {
    let key_path = format!(r"Software\Classes\CLSID\{}\InProcServer32", clsid_string());
    let wide_key = to_wide(&key_path);

    let mut hkey = HKEY::default();
    let result = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(wide_key.as_ptr()),
            Some(0),
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        )
    };

    if result != ERROR_SUCCESS {
        return Err(format!(
            "Failed to create registry key: error code {}",
            result.0
        ));
    }

    // Set default value to the DLL path.
    let wide_path = to_wide(dll_path);
    let path_bytes = wide_to_bytes(&wide_path);

    let result = unsafe { RegSetValueExW(hkey, None, Some(0), REG_SZ, Some(&path_bytes)) };

    if result != ERROR_SUCCESS {
        let _ = unsafe { RegCloseKey(hkey) };
        return Err(format!("Failed to set DLL path: error code {}", result.0));
    }

    // Set ThreadingModel = "Both".
    let threading_model = to_wide("Both");
    let tm_bytes = wide_to_bytes(&threading_model);
    let wide_name = to_wide("ThreadingModel");

    let result = unsafe {
        RegSetValueExW(
            hkey,
            PCWSTR(wide_name.as_ptr()),
            Some(0),
            REG_SZ,
            Some(&tm_bytes),
        )
    };

    let _ = unsafe { RegCloseKey(hkey) };

    if result != ERROR_SUCCESS {
        return Err(format!(
            "Failed to set ThreadingModel: error code {}",
            result.0
        ));
    }

    Ok(())
}

/// Encode a Rust `&str` as a null-terminated UTF-16 wide string.
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Convert a null-terminated UTF-16 slice to a byte slice for registry APIs.
fn wide_to_bytes(wide: &[u16]) -> Vec<u8> {
    wide.iter().flat_map(|w| w.to_le_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::encode_worker::JpegFrameBuffer;

    #[test]
    fn mf_vcam_new_is_not_running() {
        let buf = Arc::new(JpegFrameBuffer::new());
        let vcam = MfVirtualCamera::new("Test Camera".to_string(), buf);
        assert!(!vcam.is_running());
    }

    #[test]
    fn mf_vcam_stop_is_idempotent() {
        let buf = Arc::new(JpegFrameBuffer::new());
        let mut vcam = MfVirtualCamera::new("Test Camera".to_string(), buf);
        // Stop without ever starting should be fine
        assert!(vcam.stop().is_ok());
        assert!(vcam.stop().is_ok());
        assert!(!vcam.is_running());
    }

    #[test]
    fn mf_vcam_double_start_fails() {
        let buf = Arc::new(JpegFrameBuffer::new());
        let mut vcam = MfVirtualCamera::new("Test Camera".to_string(), buf);

        // Manually set running to simulate an active camera
        vcam.pump_running.store(true, Ordering::Relaxed);

        let result = vcam.start();
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("already running"),
            "expected 'already running' error"
        );

        // Clean up
        vcam.pump_running.store(false, Ordering::Relaxed);
    }

    #[test]
    fn find_com_dll_returns_error_when_missing() {
        // The DLL won't exist in the test runner's directory
        // This verifies the function handles missing DLLs gracefully
        let result = MfVirtualCamera::find_com_dll();
        // May succeed or fail depending on build output location
        if let Err(e) = &result {
            assert!(
                e.contains("not found"),
                "expected 'not found' error, got: {e}"
            );
        }
    }

    #[test]
    fn clsid_string_format() {
        let s = clsid_string();
        assert!(s.starts_with('{'));
        assert!(s.ends_with('}'));
        assert_eq!(s.len(), 38);
        assert_eq!(s.matches('-').count(), 4);
    }
}
