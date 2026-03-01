//! Canon EDSDK camera backend.
//!
//! Provides Canon EOS camera support via the Canon EDSDK (digital camera SDK).
//! All EDSDK FFI is behind `#[cfg(feature = "canon")]` — mock-based tests
//! run without the real DLLs.

pub mod api;
pub mod backend;
pub mod controls;
pub mod discovery;
#[cfg(all(feature = "canon", target_os = "windows"))]
pub mod ffi;
pub mod hotplug;
pub mod live_view;
pub mod mock;
#[cfg(all(feature = "canon", target_os = "windows"))]
pub mod sdk;
pub mod types;
