//! Raw EDSDK FFI declarations.
//!
//! These are only compiled when the `canon` feature is enabled AND the
//! EDSDK DLLs are available for linking. All access goes through the
//! safe `EdsSdk` wrapper — never call these directly.

#![allow(non_snake_case)]

use super::types::{
    EdsCameraCommand, EdsCameraListRef, EdsCameraRef, EdsDataType, EdsDeviceInfo, EdsError,
    EdsEvfImageRef, EdsPropertyID, EdsStateEvent, EdsStreamRef,
};

/// State event handler callback type.
pub type EdsStateEventHandler =
    unsafe extern "C" fn(event: EdsStateEvent, parameter: u32, context: *mut std::ffi::c_void);

#[link(name = "EDSDK")]
extern "C" {
    /// Initialise the EDSDK. Must be called before any other SDK function.
    pub fn EdsInitializeSDK() -> EdsError;

    /// Shut down the EDSDK and release all resources.
    pub fn EdsTerminateSDK() -> EdsError;

    /// Get the list of connected Canon cameras.
    pub fn EdsGetCameraList(camera_list: *mut EdsCameraListRef) -> EdsError;

    /// Get the number of children (cameras) in a camera list.
    pub fn EdsGetChildCount(ref_: EdsCameraListRef, count: *mut u32) -> EdsError;

    /// Get a child (camera) at a specific index.
    pub fn EdsGetChildAtIndex(
        ref_: EdsCameraListRef,
        index: u32,
        child: *mut EdsCameraRef,
    ) -> EdsError;

    /// Open a session with a camera.
    pub fn EdsOpenSession(camera: EdsCameraRef) -> EdsError;

    /// Close a session with a camera.
    pub fn EdsCloseSession(camera: EdsCameraRef) -> EdsError;

    /// Get device information for a camera.
    pub fn EdsGetDeviceInfo(camera: EdsCameraRef, info: *mut EdsDeviceInfo) -> EdsError;

    /// Send a command to a camera.
    pub fn EdsSendCommand(camera: EdsCameraRef, command: EdsCameraCommand, param: i32) -> EdsError;

    /// Create a memory stream for EVF image data.
    pub fn EdsCreateMemoryStream(size: u64, stream: *mut EdsStreamRef) -> EdsError;

    /// Create an EVF image reference from a stream.
    pub fn EdsCreateEvfImageRef(stream: EdsStreamRef, image: *mut EdsEvfImageRef) -> EdsError;

    /// Download the current EVF image from the camera.
    pub fn EdsDownloadEvfImage(camera: EdsCameraRef, image: EdsEvfImageRef) -> EdsError;

    /// Get a property value from a camera.
    pub fn EdsGetPropertyData(
        camera: EdsCameraRef,
        prop_id: EdsPropertyID,
        param: i32,
        size: u32,
        data: *mut std::ffi::c_void,
    ) -> EdsError;

    /// Set a property value on a camera.
    pub fn EdsSetPropertyData(
        camera: EdsCameraRef,
        prop_id: EdsPropertyID,
        param: i32,
        size: u32,
        data: *const std::ffi::c_void,
    ) -> EdsError;

    /// Get the property description (available values) for a property.
    pub fn EdsGetPropertySize(
        camera: EdsCameraRef,
        prop_id: EdsPropertyID,
        param: i32,
        data_type: *mut EdsDataType,
        size: *mut u32,
    ) -> EdsError;

    /// Register a state event handler for a camera.
    pub fn EdsSetCameraStateEventHandler(
        camera: EdsCameraRef,
        event: EdsStateEvent,
        handler: EdsStateEventHandler,
        context: *mut std::ffi::c_void,
    ) -> EdsError;

    /// Process pending EDSDK events (must be called periodically).
    pub fn EdsGetEvent() -> EdsError;

    /// Release an EDSDK object reference.
    pub fn EdsRelease(ref_: *mut std::ffi::c_void) -> u32;
}
