//! MF Virtual Camera sink for Windows 11+.
//!
//! Uses `MFCreateVirtualCamera` to register a session-lifetime virtual camera
//! backed by our custom COM media source DLL (`vcam-source`). A background
//! frame pump thread reads JPEG frames from the preview session, decodes them,
//! converts to NV12, and writes into shared memory for the media source to read.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use tracing::{debug, error, info, warn};
use windows::core::{GUID, HSTRING};
use windows::Win32::Devices::Properties::DEVPROP_TYPE_GUID;
use windows::Win32::Media::KernelStreaming::{
    KSCATEGORY_CAPTURE, KSCATEGORY_VIDEO, KSCATEGORY_VIDEO_CAMERA,
};
use windows::Win32::Media::MediaFoundation::{
    IMFVirtualCamera, MFCreateVirtualCamera, MFVirtualCameraAccess_CurrentUser,
    MFVirtualCameraLifetime_Session, MFVirtualCameraType_SoftwareCameraSource,
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
    /// File-backed shared memory owner. Must outlive both the pump thread and
    /// the virtual camera — the DLL reads from the file until `Shutdown()`.
    /// Dropped last in `stop()` so `DeleteFileW` runs after all handles close.
    shm_owner: Option<Arc<vcam_shared::SharedMemoryOwner>>,
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
            shm_owner: None,
        }
    }

    /// Create the MF virtual camera backed by MSIX COM redirection.
    ///
    /// The MSIX sparse package must already be registered (done at install
    /// time by the NSIS post-install hook). COM redirection lets FrameServer
    /// resolve the vcam-source CLSID without any HKCU/HKLM registry entries.
    fn create_virtual_camera(&mut self) -> Result<(), String> {
        info!("Creating MF virtual camera for '{}'...", self.device_name);

        // Build the CLSID string for our media source
        let clsid_str = clsid_string();
        let friendly_name = format!("Cameras App \u{2014} {}", self.device_name);
        debug!("CLSID: {clsid_str}, friendly name: '{friendly_name}'");

        let friendly_name_h = HSTRING::from(&friendly_name);
        let clsid_h = HSTRING::from(&clsid_str);
        // Register under all three categories so FrameServer's DirectShow
        // compatibility bridge exposes the device to OBS and other DS apps.
        let categories = [
            KSCATEGORY_VIDEO_CAMERA,
            KSCATEGORY_VIDEO,
            KSCATEGORY_CAPTURE,
        ];

        info!("Calling MFCreateVirtualCamera...");
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
        .map_err(|e| {
            error!("MFCreateVirtualCamera failed: {e}");
            format!("MFCreateVirtualCamera failed: {e}")
        })?;

        info!("Created MF virtual camera: '{friendly_name}'");

        // Set the device class GUID to Camera so DirectShow-based apps (OBS)
        // can discover this device. Without this, MFCreateVirtualCamera
        // registers the device under SoftwareDevice which DirectShow ignores.
        //
        // Non-fatal: requires elevation. Session-lifetime virtual cameras
        // create a transient device node (no persistent devnode at install
        // time), so this cannot be set via registry/NSIS. For non-admin users,
        // OBS won't discover the device via DirectShow — this is a known
        // limitation of session-lifetime virtual cameras.
        if let Err(e) = set_camera_class_guid(&vcam) {
            warn!("Could not set Camera class GUID (OBS may not see this device): {e}");
        }

        info!("Starting IMFVirtualCamera...");
        unsafe { vcam.Start(None) }.map_err(|e| {
            error!("IMFVirtualCamera::Start failed: {e}");
            format!("IMFVirtualCamera::Start failed: {e}")
        })?;

        info!("MF virtual camera started and visible to consumers");
        self.vcam = Some(vcam);
        Ok(())
    }

    /// Spawn the background frame pump thread.
    ///
    /// Creates the file-backed shared memory BEFORE starting the pump, so
    /// the file is ready by the time FrameServer loads the COM DLL.
    fn start_pump(&mut self) -> Result<(), String> {
        let file_path = std::path::Path::new(vcam_shared::SHARED_MEMORY_FILE_PATH);

        // Create the parent directory if it doesn't exist.
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                error!("Failed to create directory {}: {e}", parent.display());
                format!("mkdir: {e}")
            })?;
        }

        let shm_owner = Arc::new(
            vcam_shared::SharedMemoryOwner::new(
                file_path,
                vcam_shared::DEFAULT_WIDTH,
                vcam_shared::DEFAULT_HEIGHT,
                3,
            )
            .map_err(|e| {
                error!("Failed to create shared memory file: {e}");
                format!("shared memory: {e}")
            })?,
        );

        // Store the owner so it outlives the pump thread AND the virtual
        // camera shutdown. The DLL reads from the file until Shutdown().
        self.shm_owner = Some(Arc::clone(&shm_owner));

        info!(
            "Created shared memory file '{}'",
            vcam_shared::SHARED_MEMORY_FILE_PATH
        );

        self.pump_running.store(true, Ordering::Relaxed);

        let jpeg_buffer = Arc::clone(&self.jpeg_buffer);
        let running = Arc::clone(&self.pump_running);

        info!("Spawning vcam-pump thread...");
        let handle = std::thread::Builder::new()
            .name("vcam-pump".to_string())
            .spawn(move || {
                super::frame_pump::run_frame_pump(jpeg_buffer, shm_owner, running);
            })
            .map_err(|e| {
                error!("Failed to spawn frame pump thread: {e}");
                format!("failed to spawn frame pump thread: {e}")
            })?;

        self.pump_handle = Some(handle);
        info!("Frame pump thread spawned successfully");
        Ok(())
    }
}

impl VirtualCameraSink for MfVirtualCamera {
    fn start(&mut self) -> Result<(), String> {
        if self.pump_running.load(Ordering::Relaxed) {
            warn!(
                "Virtual camera for '{}' is already running — ignoring start",
                self.device_name
            );
            return Err("virtual camera is already running".to_string());
        }

        info!(
            "Starting virtual camera pipeline for '{}'...",
            self.device_name
        );
        // Create the shared memory file and start the pump BEFORE creating the
        // virtual camera. FrameServer loads the COM DLL during
        // MFCreateVirtualCamera / Start(), so the file must already exist.
        self.start_pump()?;
        self.create_virtual_camera()?;

        info!("Virtual camera fully started for '{}'", self.device_name);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        info!("Stopping virtual camera for '{}'...", self.device_name);

        // Signal the pump to stop
        self.pump_running.store(false, Ordering::Relaxed);

        // Join the pump thread
        if let Some(handle) = self.pump_handle.take() {
            info!("Waiting for frame pump thread to finish...");
            let _ = handle.join();
            info!("Frame pump thread joined");
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
        } else {
            debug!(
                "No active MF virtual camera to stop for '{}'",
                self.device_name
            );
        }

        // Drop the shared memory owner LAST — the DLL may still have read
        // handles open until Shutdown() completes. Drop triggers unmap +
        // close file handle + DeleteFileW.
        if let Some(owner) = self.shm_owner.take() {
            drop(owner);
            info!("Shared memory file cleaned up");
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
// Device property helpers
// ---------------------------------------------------------------------------

/// Camera device class GUID: `{ca3e7ab9-b4c3-4ae6-8251-579ef933890f}`.
///
/// Devices in this class appear in DirectShow enumeration (used by OBS).
/// Without this, MFCreateVirtualCamera registers under `SoftwareDevice`.
const CAMERA_CLASS_GUID: GUID = GUID::from_u128(0xca3e7ab9_b4c3_4ae6_8251_579ef933890f);

/// Set `DEVPKEY_Device_ClassGuid` to the Camera class on the virtual camera
/// device so DirectShow-based consumers (OBS, older apps) can discover it.
fn set_camera_class_guid(vcam: &IMFVirtualCamera) -> Result<(), String> {
    use windows::Win32::Devices::Properties::DEVPKEY_Device_ClassGuid;

    // Serialise the GUID as a 16-byte Windows GUID structure (little-endian)
    let mut guid_bytes = [0u8; 16];
    guid_bytes[0..4].copy_from_slice(&CAMERA_CLASS_GUID.data1.to_le_bytes());
    guid_bytes[4..6].copy_from_slice(&CAMERA_CLASS_GUID.data2.to_le_bytes());
    guid_bytes[6..8].copy_from_slice(&CAMERA_CLASS_GUID.data3.to_le_bytes());
    guid_bytes[8..16].copy_from_slice(&CAMERA_CLASS_GUID.data4);

    info!(
        "Setting DEVPKEY_Device_ClassGuid to Camera class ({:?})...",
        CAMERA_CLASS_GUID
    );

    unsafe { vcam.AddProperty(&DEVPKEY_Device_ClassGuid, DEVPROP_TYPE_GUID, &guid_bytes) }
        .map_err(|e| {
            error!("IMFVirtualCamera::AddProperty(ClassGuid) failed: {e}");
            format!("failed to set Camera class GUID: {e}")
        })?;

    info!("Camera class GUID set successfully");
    Ok(())
}

// ---------------------------------------------------------------------------
// CLSID helpers
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
    fn camera_class_guid_matches_expected_value() {
        // {ca3e7ab9-b4c3-4ae6-8251-579ef933890f} is the Camera device class
        assert_eq!(CAMERA_CLASS_GUID.data1, 0xca3e7ab9);
        assert_eq!(CAMERA_CLASS_GUID.data2, 0xb4c3);
        assert_eq!(CAMERA_CLASS_GUID.data3, 0x4ae6);
        assert_eq!(
            CAMERA_CLASS_GUID.data4,
            [0x82, 0x51, 0x57, 0x9e, 0xf9, 0x33, 0x89, 0x0f]
        );
    }

    #[test]
    fn camera_class_guid_serialises_to_16_bytes() {
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&CAMERA_CLASS_GUID.data1.to_le_bytes());
        bytes[4..6].copy_from_slice(&CAMERA_CLASS_GUID.data2.to_le_bytes());
        bytes[6..8].copy_from_slice(&CAMERA_CLASS_GUID.data3.to_le_bytes());
        bytes[8..16].copy_from_slice(&CAMERA_CLASS_GUID.data4);
        assert_eq!(bytes.len(), 16);
        // First 4 bytes should be data1 in little-endian
        assert_eq!(&bytes[0..4], &[0xb9, 0x7a, 0x3e, 0xca]);
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
