//! Safe EDSDK wrapper with RAII lifecycle management.
//!
//! Only compiled when the `canon` feature is enabled and EDSDK DLLs are
//! available. Production code uses this; tests use `MockEdsSdk` instead.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::camera::error::{CameraError, Result};

use super::api::{CameraHandle, EdsSdkApi};
use super::ffi;
use super::types::*;

/// Whether the SDK has been initialised (global, since EDSDK is per-process).
static SDK_INITIALISED: AtomicBool = AtomicBool::new(false);

/// Safe wrapper around the Canon EDSDK.
///
/// Initialises the SDK on construction and terminates it on drop.
/// Only one instance should exist per process.
pub struct EdsSdk {
    _private: (),
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

        let err = unsafe { ffi::EdsInitializeSDK() };
        if err != EDS_ERR_OK {
            SDK_INITIALISED.store(false, Ordering::SeqCst);
            return Err(CameraError::CanonSdkError(format!(
                "EdsInitializeSDK failed: {} (0x{:08X})",
                error_description(err),
                err
            )));
        }

        Ok(Self { _private: () })
    }
}

impl Drop for EdsSdk {
    fn drop(&mut self) {
        unsafe {
            ffi::EdsTerminateSDK();
        }
        SDK_INITIALISED.store(false, Ordering::SeqCst);
    }
}

impl EdsSdkApi for EdsSdk {
    fn camera_list(&self) -> Result<Vec<CameraHandle>> {
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

            let handles: Vec<CameraHandle> = (0..count).map(|i| CameraHandle(i as usize)).collect();
            ffi::EdsRelease(list);
            Ok(handles)
        }
    }

    fn open_session(&self, _camera: CameraHandle) -> Result<()> {
        // Real implementation would use stored EdsCameraRef handles
        Ok(())
    }

    fn close_session(&self, _camera: CameraHandle) -> Result<()> {
        Ok(())
    }

    fn get_device_info(&self, _camera: CameraHandle) -> Result<EdsDeviceInfo> {
        Err(CameraError::CanonSdkError(
            "get_device_info not fully wired yet".to_string(),
        ))
    }

    fn start_live_view(&self, _camera: CameraHandle) -> Result<()> {
        Ok(())
    }

    fn stop_live_view(&self, _camera: CameraHandle) -> Result<()> {
        Ok(())
    }

    fn download_evf_image(&self, _camera: CameraHandle) -> Result<Vec<u8>> {
        Err(CameraError::CanonSdkError(
            "download_evf_image not fully wired yet".to_string(),
        ))
    }

    fn get_property(&self, _camera: CameraHandle, _prop: EdsPropertyID) -> Result<i32> {
        Err(CameraError::CanonSdkError(
            "get_property not fully wired yet".to_string(),
        ))
    }

    fn set_property(&self, _camera: CameraHandle, _prop: EdsPropertyID, _value: i32) -> Result<()> {
        Ok(())
    }

    fn get_property_desc(
        &self,
        _camera: CameraHandle,
        _prop: EdsPropertyID,
    ) -> Result<EdsPropertyDesc> {
        Err(CameraError::CanonSdkError(
            "get_property_desc not fully wired yet".to_string(),
        ))
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
