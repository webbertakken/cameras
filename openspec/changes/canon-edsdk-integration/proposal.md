## Why

Canon EOS cameras (DSLRs and mirrorless) connected via USB use PTP/MTP rather than DirectShow/UVC, so they are invisible to the current `WindowsBackend`. Canon's official "EOS Webcam Utility" is now paywalled (pro-only), making native SDK support essential. Canon's EDSDK provides the richest feature set for EOS cameras: live view, full property control (ISO, aperture, shutter speed, white balance), and camera state events.

EDSDK developer approval is pending. We build with mocked SDK calls (TDD), then plug in real DLLs when available.

## What changes

- New `CanonBackend` implementing the `CameraBackend` trait, wrapping EDSDK via Rust FFI
- EDSDK FFI bindings behind `#[cfg(feature = "canon")]` feature flag
- Mock EDSDK layer for TDD without real DLLs
- Canon cameras appear in the unified sidebar alongside DirectShow devices
- Canon live view JPEG frames fed into the existing preview pipeline
- Canon-specific controls (ISO, aperture, shutter speed, white balance, exposure compensation) exposed as `ControlDescriptor` values
- EDSDK camera state events for hotplug connect/disconnect
- Composite backend that merges devices from multiple backends

## Capabilities

### New capabilities

- `edsdk-ffi`: Rust FFI bindings for Canon EDSDK functions with feature-gated compilation
- `mock-edsdk`: Test double simulating EDSDK behaviour for development without real hardware
- `canon-backend`: `CanonBackend` struct implementing `CameraBackend` trait
- `canon-discovery`: Enumerate Canon cameras via EDSDK, generate stable device IDs from serial numbers
- `canon-live-view`: Start/stop live view, poll JPEG frames, deliver via `FrameBuffer`
- `canon-controls`: Map EDSDK properties to `ControlDescriptor` (ISO, aperture, shutter speed, WB, exposure comp)
- `canon-hotplug`: EDSDK camera state events for connect/disconnect detection
- `composite-backend`: Merge devices from multiple backends (DirectShow + Canon) into unified camera list

### Modified capabilities

- `camera-discovery`: Extended to include Canon EDSDK devices alongside DirectShow/UVC devices

## Impact

- **Rust backend**: New `src-tauri/src/camera/canon/` module with FFI, mock, backend, discovery, live view, controls, and hotplug submodules
- **Cargo.toml**: New `canon` feature flag, conditional EDSDK dependencies
- **Camera commands**: `CameraState` updated to hold composite backend
- **Frontend**: No changes required (dynamic control rendering handles Canon controls automatically)
- **CI**: Canon feature disabled by default; separate CI job for `--features canon` when DLLs available
