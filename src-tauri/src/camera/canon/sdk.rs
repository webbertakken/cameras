//! Safe EDSDK wrapper with RAII lifecycle management.
//!
//! Only compiled when the `canon` feature is enabled and EDSDK DLLs are
//! available. Production code uses this; tests use `MockEdsSdk` instead.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

use crate::camera::error::{CameraError, Result};

use super::api::{CameraHandle, EdsSdkApi};
use super::ffi;
use super::types::*;

/// Whether the SDK has been initialised (global, since EDSDK is per-process).
static SDK_INITIALISED: AtomicBool = AtomicBool::new(false);

/// Send+Sync wrapper for raw EDSDK pointers.
///
/// EDSDK handles (`EdsCameraRef`, `EdsCameraListRef`) are `*mut c_void`
/// which are not `Send`/`Sync` by default. The Canon SDK is thread-safe
/// when used with proper COM apartment initialisation, so this is sound
/// as long as COM is initialised on each thread that calls EDSDK.
#[derive(Debug, Clone, Copy)]
struct SendSyncPtr(*mut std::ffi::c_void);

// SAFETY: EDSDK handles are thread-safe when COM is properly initialised.
unsafe impl Send for SendSyncPtr {}
// SAFETY: All access to stored handles goes through Mutex.
unsafe impl Sync for SendSyncPtr {}

/// COM apartment guard — ensures CoInitializeEx/CoUninitialize pairing.
///
/// EDSDK requires COM STA (Single-Threaded Apartment). This guard
/// initializes COM before the SDK and keeps it alive for the SDK's
/// lifetime. Handles the case where COM is already initialized
/// (e.g. by Tauri's tao event loop).
struct ComGuard {
    owns_init: bool,
}

impl ComGuard {
    fn init() -> Result<Self> {
        unsafe {
            let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            if hr.is_err() {
                // RPC_E_CHANGED_MODE means COM is already initialised
                // with a different apartment type. EDSDK works in both
                // STA and MTA, so accept the existing apartment.
                if hr == windows::core::HRESULT(0x80010106u32 as i32) {
                    tracing::debug!(
                        "COM already initialised, reusing existing apartment for EDSDK"
                    );
                    return Ok(Self { owns_init: false });
                }
                return Err(CameraError::CanonSdkError(format!(
                    "CoInitializeEx for EDSDK failed: {hr:?}"
                )));
            }
        }
        Ok(Self { owns_init: true })
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.owns_init {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

/// Safe wrapper around the Canon EDSDK.
///
/// Initialises COM and the SDK on construction, terminates both on drop.
/// Only one instance should exist per process.
pub struct EdsSdk {
    /// COM guard — must be dropped AFTER the SDK is terminated.
    /// Field order matters: Rust drops fields in declaration order.
    _com: ComGuard,
    /// Stored camera references, keyed by CameraHandle index.
    cameras: Mutex<HashMap<CameraHandle, SendSyncPtr>>,
    /// The camera list reference from the most recent enumeration.
    camera_list_ref: Mutex<Option<SendSyncPtr>>,
}

impl EdsSdk {
    /// Initialise the EDSDK.
    ///
    /// # Errors
    ///
    /// Returns `CameraError::CanonSdkError` if `EdsInitializeSDK` fails.
    pub fn new() -> Result<Self> {
        if SDK_INITIALISED.swap(true, Ordering::SeqCst) {
            return Err(CameraError::CanonSdkError(
                "EDSDK already initialised".to_string(),
            ));
        }

        // Initialize COM before the SDK — EDSDK requires COM STA.
        let com = ComGuard::init().inspect_err(|_| {
            SDK_INITIALISED.store(false, Ordering::SeqCst);
        })?;

        let err = unsafe { ffi::EdsInitializeSDK() };
        if err != EDS_ERR_OK {
            SDK_INITIALISED.store(false, Ordering::SeqCst);
            return Err(CameraError::CanonSdkError(format!(
                "EdsInitializeSDK failed: {} (0x{:08X})",
                error_description(err),
                err
            )));
        }

        Ok(Self {
            _com: com,
            cameras: Mutex::new(HashMap::new()),
            camera_list_ref: Mutex::new(None),
        })
    }

    /// Look up the stored EdsCameraRef for a handle.
    fn get_camera_ref(&self, camera: CameraHandle) -> Result<EdsCameraRef> {
        let cameras = self.cameras.lock().unwrap();
        cameras.get(&camera).map(|p| p.0).ok_or_else(|| {
            CameraError::CanonSdkError(format!("invalid camera handle: {}", camera.0))
        })
    }

    /// Release all stored camera references and the camera list ref.
    fn release_stored_refs(&self) {
        let mut cameras = self.cameras.lock().unwrap();
        for (_, ptr) in cameras.drain() {
            unsafe {
                ffi::EdsRelease(ptr.0);
            }
        }

        let mut list_ref = self.camera_list_ref.lock().unwrap();
        if let Some(ptr) = list_ref.take() {
            unsafe {
                ffi::EdsRelease(ptr.0);
            }
        }
    }
}

impl Drop for EdsSdk {
    fn drop(&mut self) {
        self.release_stored_refs();
        unsafe {
            ffi::EdsTerminateSDK();
        }
        SDK_INITIALISED.store(false, Ordering::SeqCst);
    }
}

impl EdsSdkApi for EdsSdk {
    fn camera_list(&self) -> Result<Vec<CameraHandle>> {
        // Release previously stored refs before re-enumerating
        self.release_stored_refs();

        unsafe {
            let mut list: EdsCameraListRef = std::ptr::null_mut();
            let err = ffi::EdsGetCameraList(&mut list);
            if err != EDS_ERR_OK {
                return Err(CameraError::CanonSdkError(format!(
                    "EdsGetCameraList failed: {}",
                    error_description(err)
                )));
            }

            let mut count: u32 = 0;
            let err = ffi::EdsGetChildCount(list, &mut count);
            if err != EDS_ERR_OK {
                ffi::EdsRelease(list);
                return Err(CameraError::CanonSdkError(format!(
                    "EdsGetChildCount failed: {}",
                    error_description(err)
                )));
            }

            // Store the list ref so we can release it later
            {
                let mut list_ref = self.camera_list_ref.lock().unwrap();
                *list_ref = Some(SendSyncPtr(list));
            }

            let mut cameras = self.cameras.lock().unwrap();
            let mut handles = Vec::with_capacity(count as usize);

            for i in 0..count {
                let mut camera_ref: EdsCameraRef = std::ptr::null_mut();
                let err = ffi::EdsGetChildAtIndex(list, i as i32, &mut camera_ref);
                if err != EDS_ERR_OK {
                    tracing::warn!("EdsGetChildAtIndex({i}) failed: {}", error_description(err));
                    continue;
                }

                let handle = CameraHandle(i as usize);
                cameras.insert(handle, SendSyncPtr(camera_ref));
                handles.push(handle);
            }

            Ok(handles)
        }
    }

    fn open_session(&self, camera: CameraHandle) -> Result<()> {
        let camera_ref = self.get_camera_ref(camera)?;
        let err = unsafe { ffi::EdsOpenSession(camera_ref) };
        if err != EDS_ERR_OK {
            return Err(CameraError::CanonSdkError(format!(
                "EdsOpenSession failed: {}",
                error_description(err)
            )));
        }
        Ok(())
    }

    fn close_session(&self, camera: CameraHandle) -> Result<()> {
        let camera_ref = self.get_camera_ref(camera)?;
        let err = unsafe { ffi::EdsCloseSession(camera_ref) };
        if err != EDS_ERR_OK {
            return Err(CameraError::CanonSdkError(format!(
                "EdsCloseSession failed: {}",
                error_description(err)
            )));
        }
        Ok(())
    }

    fn get_device_info(&self, camera: CameraHandle) -> Result<EdsDeviceInfo> {
        let camera_ref = self.get_camera_ref(camera)?;
        unsafe {
            let mut info: EdsDeviceInfo = std::mem::zeroed();
            let err = ffi::EdsGetDeviceInfo(camera_ref, &mut info);
            if err != EDS_ERR_OK {
                return Err(CameraError::CanonSdkError(format!(
                    "EdsGetDeviceInfo failed: {}",
                    error_description(err)
                )));
            }
            Ok(info)
        }
    }

    fn start_live_view(&self, camera: CameraHandle) -> Result<()> {
        let camera_ref = self.get_camera_ref(camera)?;

        // Enable EVF output on the camera
        let evf_mode: u32 = 1;
        let err = unsafe {
            ffi::EdsSetPropertyData(
                camera_ref,
                PROP_ID_EVF_OUTPUT_DEVICE,
                0,
                std::mem::size_of::<u32>() as u32,
                &evf_mode as *const u32 as *const std::ffi::c_void,
            )
        };
        if err != EDS_ERR_OK {
            return Err(CameraError::CanonSdkError(format!(
                "start_live_view (set EVF output) failed: {}",
                error_description(err)
            )));
        }
        Ok(())
    }

    fn stop_live_view(&self, camera: CameraHandle) -> Result<()> {
        let camera_ref = self.get_camera_ref(camera)?;

        // Disable EVF output on the camera
        let evf_mode: u32 = 0;
        let err = unsafe {
            ffi::EdsSetPropertyData(
                camera_ref,
                PROP_ID_EVF_OUTPUT_DEVICE,
                0,
                std::mem::size_of::<u32>() as u32,
                &evf_mode as *const u32 as *const std::ffi::c_void,
            )
        };
        if err != EDS_ERR_OK {
            return Err(CameraError::CanonSdkError(format!(
                "stop_live_view (set EVF output) failed: {}",
                error_description(err)
            )));
        }
        Ok(())
    }

    fn download_evf_image(&self, camera: CameraHandle) -> Result<Vec<u8>> {
        let camera_ref = self.get_camera_ref(camera)?;

        unsafe {
            // Create a memory stream to receive the EVF image data
            let mut stream: EdsStreamRef = std::ptr::null_mut();
            let err = ffi::EdsCreateMemoryStream(0, &mut stream);
            if err != EDS_ERR_OK {
                return Err(CameraError::CanonSdkError(format!(
                    "EdsCreateMemoryStream failed: {}",
                    error_description(err)
                )));
            }

            // Create an EVF image reference from the stream
            let mut evf_image: EdsEvfImageRef = std::ptr::null_mut();
            let err = ffi::EdsCreateEvfImageRef(stream, &mut evf_image);
            if err != EDS_ERR_OK {
                ffi::EdsRelease(stream);
                return Err(CameraError::CanonSdkError(format!(
                    "EdsCreateEvfImageRef failed: {}",
                    error_description(err)
                )));
            }

            // Download the live view image
            let err = ffi::EdsDownloadEvfImage(camera_ref, evf_image);
            if err != EDS_ERR_OK {
                ffi::EdsRelease(evf_image);
                ffi::EdsRelease(stream);
                return Err(CameraError::CanonSdkError(format!(
                    "EdsDownloadEvfImage failed: {}",
                    error_description(err)
                )));
            }

            // Read the JPEG data from the memory stream
            let data = read_stream_data(stream);

            ffi::EdsRelease(evf_image);
            ffi::EdsRelease(stream);

            data
        }
    }

    fn get_property(&self, camera: CameraHandle, prop: EdsPropertyID) -> Result<i32> {
        let camera_ref = self.get_camera_ref(camera)?;
        unsafe {
            let mut value: i32 = 0;
            let err = ffi::EdsGetPropertyData(
                camera_ref,
                prop,
                0,
                std::mem::size_of::<i32>() as u32,
                &mut value as *mut i32 as *mut std::ffi::c_void,
            );
            if err != EDS_ERR_OK {
                return Err(CameraError::CanonSdkError(format!(
                    "EdsGetPropertyData(0x{prop:04X}) failed: {}",
                    error_description(err)
                )));
            }
            Ok(value)
        }
    }

    fn set_property(&self, camera: CameraHandle, prop: EdsPropertyID, value: i32) -> Result<()> {
        let camera_ref = self.get_camera_ref(camera)?;
        let err = unsafe {
            ffi::EdsSetPropertyData(
                camera_ref,
                prop,
                0,
                std::mem::size_of::<i32>() as u32,
                &value as *const i32 as *const std::ffi::c_void,
            )
        };
        if err != EDS_ERR_OK {
            return Err(CameraError::CanonSdkError(format!(
                "EdsSetPropertyData(0x{prop:04X}) failed: {}",
                error_description(err)
            )));
        }
        Ok(())
    }

    fn get_property_desc(
        &self,
        camera: CameraHandle,
        prop: EdsPropertyID,
    ) -> Result<EdsPropertyDesc> {
        let camera_ref = self.get_camera_ref(camera)?;
        unsafe {
            let mut data_type: EdsDataType = 0;
            let mut size: u32 = 0;
            let err = ffi::EdsGetPropertySize(camera_ref, prop, 0, &mut data_type, &mut size);
            if err != EDS_ERR_OK {
                return Err(CameraError::CanonSdkError(format!(
                    "EdsGetPropertySize(0x{prop:04X}) failed: {}",
                    error_description(err)
                )));
            }

            // Property descriptions for select-type properties are typically
            // fetched by reading the available values. EDSDK doesn't have a
            // direct "list of allowed values" call via EdsGetPropertySize alone;
            // we return the size info as a single-element descriptor.
            // The mock provides full value lists; the real SDK needs
            // EdsGetPropertyDesc which is not in our FFI declarations.
            // For now, return a descriptor with the current value.
            let current = self.get_property(camera, prop).unwrap_or(0);
            Ok(EdsPropertyDesc {
                num_elements: 1,
                prop_desc: vec![current],
            })
        }
    }

    fn get_event(&self) -> Result<()> {
        unsafe {
            let err = ffi::EdsGetEvent();
            if err != EDS_ERR_OK {
                return Err(CameraError::CanonSdkError(format!(
                    "EdsGetEvent failed: {}",
                    error_description(err)
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::canon::api::EdsSdkApi;

    #[test]
    #[ignore] // requires Canon camera connected + EDSDK DLLs
    fn real_sdk_discovers_connected_canon_camera() {
        let sdk = EdsSdk::new().expect("EDSDK should initialise");

        let handles = sdk.camera_list().expect("camera_list should succeed");
        assert!(
            !handles.is_empty(),
            "expected at least one Canon camera connected"
        );

        // Verify we can get device info for the first camera
        let info = sdk
            .get_device_info(handles[0])
            .expect("get_device_info should succeed");
        let model = info.model_name();
        let port = info.port_name();

        assert!(!model.is_empty(), "model name should not be empty");
        println!("Canon camera: model={model}, port={port}");

        // SDK should be droppable without errors (RAII cleanup)
        drop(sdk);
    }
}

/// Read all data from an EDSDK memory stream.
///
/// Reads the stream length via `EdsGetLength`, then copies the raw bytes
/// out via `EdsGetPointer`.
unsafe fn read_stream_data(stream: EdsStreamRef) -> Result<Vec<u8>> {
    let mut length: u64 = 0;
    let err = ffi::EdsGetLength(stream, &mut length);
    if err != EDS_ERR_OK {
        return Err(CameraError::CanonSdkError(format!(
            "EdsGetLength failed: {}",
            error_description(err)
        )));
    }

    let mut ptr: *mut u8 = std::ptr::null_mut();
    let err = ffi::EdsGetPointer(stream, &mut ptr);
    if err != EDS_ERR_OK {
        return Err(CameraError::CanonSdkError(format!(
            "EdsGetPointer failed: {}",
            error_description(err)
        )));
    }

    if ptr.is_null() || length == 0 {
        return Err(CameraError::CanonSdkError(
            "empty EVF image stream".to_string(),
        ));
    }

    let data = std::slice::from_raw_parts(ptr, length as usize).to_vec();
    Ok(data)
}
