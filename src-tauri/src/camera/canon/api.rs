//! `EdsSdkApi` trait — abstracts EDSDK operations for testability.
//!
//! The real `EdsSdk` and the `MockEdsSdk` both implement this trait,
//! allowing `CanonBackend<S>` to be generic over the SDK implementation.

use crate::camera::error::Result;

use super::types::{EdsDeviceInfo, EdsPropertyDesc, EdsPropertyID};

/// Opaque camera handle used across the API boundary.
///
/// For the real SDK this wraps an `EdsCameraRef`; for the mock it is
/// an index into the mock's internal camera list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CameraHandle(pub usize);

/// Abstraction over EDSDK operations.
///
/// All methods take `&self` — the implementation manages interior
/// mutability (e.g. via `Mutex` for the mock's state).
pub trait EdsSdkApi: Send + Sync {
    /// List all connected Canon cameras, returning handles.
    fn camera_list(&self) -> Result<Vec<CameraHandle>>;

    /// Open a session with a camera.
    fn open_session(&self, camera: CameraHandle) -> Result<()>;

    /// Close a session with a camera.
    fn close_session(&self, camera: CameraHandle) -> Result<()>;

    /// Get device information for a camera.
    fn get_device_info(&self, camera: CameraHandle) -> Result<EdsDeviceInfo>;

    /// Start live view output on the camera.
    fn start_live_view(&self, camera: CameraHandle) -> Result<()>;

    /// Stop live view output on the camera.
    fn stop_live_view(&self, camera: CameraHandle) -> Result<()>;

    /// Download the current live view JPEG frame.
    ///
    /// Returns `Err` with `EDS_ERR_OBJECT_NOTREADY` mapped to
    /// `CameraError` when the frame is not yet available.
    fn download_evf_image(&self, camera: CameraHandle) -> Result<Vec<u8>>;

    /// Read a property value from the camera.
    fn get_property(&self, camera: CameraHandle, prop: EdsPropertyID) -> Result<i32>;

    /// Write a property value to the camera.
    fn set_property(&self, camera: CameraHandle, prop: EdsPropertyID, value: i32) -> Result<()>;

    /// Get the property description (list of allowed values).
    fn get_property_desc(
        &self,
        camera: CameraHandle,
        prop: EdsPropertyID,
    ) -> Result<EdsPropertyDesc>;

    /// Process pending EDSDK events.
    fn get_event(&self) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_handle_equality() {
        let a = CameraHandle(0);
        let b = CameraHandle(0);
        let c = CameraHandle(1);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn camera_handle_debug_format() {
        let h = CameraHandle(42);
        assert_eq!(format!("{h:?}"), "CameraHandle(42)");
    }

    /// Verify the trait is object-safe (can be used as `dyn EdsSdkApi`).
    #[test]
    fn trait_is_object_safe() {
        fn _accepts_dyn(_sdk: &dyn EdsSdkApi) {}
    }

    /// Verify Send + Sync bounds are satisfied.
    #[test]
    fn trait_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn EdsSdkApi>>();
    }
}
