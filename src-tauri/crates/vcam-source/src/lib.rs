//! COM media source DLL for the MF Virtual Camera.
//!
//! Windows Camera FrameServer loads this DLL out-of-process. It reads NV12
//! frames from shared memory (via `vcam-shared`) and delivers them as
//! `IMFSample` objects to consumer applications (Zoom, Teams, etc.).

#[cfg(windows)]
mod class_factory;
#[cfg(windows)]
mod media_source;
#[cfg(windows)]
mod media_stream;
#[cfg(windows)]
pub mod registry;
#[cfg(windows)]
mod sample_factory;

#[cfg(windows)]
mod com_exports;

#[cfg(windows)]
pub use com_exports::{DllCanUnloadNow, DllGetClassObject};

/// CLSID for the Cameras App virtual camera media source.
///
/// {7B2E3A1F-4D5C-4E8B-9A6F-1C2D3E4F5A6B}
pub const VCAM_SOURCE_CLSID: u128 = 0x7B2E_3A1F_4D5C_4E8B_9A6F_1C2D_3E4F_5A6B;

/// Well-known shared memory name pattern used by both the main app and this DLL.
pub const SHARED_MEMORY_NAME: &str = r"Local\CamerasApp_VCam_0";

/// Default frame dimensions when no shared memory is connected.
pub const DEFAULT_WIDTH: u32 = 1920;

/// Default frame height.
pub const DEFAULT_HEIGHT: u32 = 1080;

/// Target frame rate (frames per second).
pub const TARGET_FPS: u32 = 30;

#[cfg(windows)]
use std::sync::atomic::{AtomicU32, Ordering};

/// Global count of active COM objects. `DllCanUnloadNow` checks this.
#[cfg(windows)]
pub(crate) static ACTIVE_OBJECTS: AtomicU32 = AtomicU32::new(0);

#[cfg(windows)]
pub(crate) fn increment_object_count() {
    ACTIVE_OBJECTS.fetch_add(1, Ordering::Relaxed);
}

#[cfg(windows)]
pub(crate) fn decrement_object_count() {
    ACTIVE_OBJECTS.fetch_sub(1, Ordering::Relaxed);
}
