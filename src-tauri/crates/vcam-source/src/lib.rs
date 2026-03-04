//! COM media source DLL for the MF Virtual Camera.
//!
//! Windows Camera FrameServer loads this DLL out-of-process. It reads NV12
//! frames from shared memory (via `vcam-shared`) and delivers them as
//! `IMFSample` objects to consumer applications (Zoom, Teams, etc.).

#[cfg(windows)]
mod activate;
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
pub(crate) mod trace;

#[cfg(windows)]
pub use com_exports::{DllCanUnloadNow, DllGetClassObject};

// Re-export shared constants from vcam-shared so both the main app and this
// DLL use the same values.
pub use vcam_shared::{
    DEFAULT_HEIGHT, DEFAULT_WIDTH, SHARED_MEMORY_NAME, TARGET_FPS, VCAM_SOURCE_CLSID,
};

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
