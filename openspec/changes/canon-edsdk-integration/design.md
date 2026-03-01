## Context

The app has a `CameraBackend` trait (`src-tauri/src/camera/backend.rs`) implemented by `WindowsBackend` (DirectShow) and `DummyBackend` (testing). Canon EOS cameras over USB use PTP/MTP, not DirectShow. Canon's EDSDK provides native access to these cameras: enumeration, live view, property read/write, and state events.

EDSDK is COM-based on Windows (same STA concerns as DirectShow). The SDK DLLs are not yet available (developer approval pending), so all code is built with mocked FFI behind a feature flag.

The existing `CameraState` holds a single `Box<dyn CameraBackend>`. To support multiple backends simultaneously, we need a composite backend that merges device lists.

## Goals / non-goals

**Goals:**

- Canon EOS cameras appear in the sidebar alongside DirectShow cameras
- Live view JPEG frames flow through the existing preview pipeline
- Canon-specific controls render via the existing dynamic control system
- All code is testable without real EDSDK DLLs (TDD with mocks)
- Zero impact on existing DirectShow functionality when Canon feature is disabled

**Non-goals:**

- Image/file transfer from Canon cameras (tethered shooting)
- Canon-specific UI chrome or branding
- macOS EDSDK support in this change (Windows first)
- Remote shutter release (future enhancement)

## Decisions

### Decision 1: Feature-gated EDSDK FFI

All EDSDK code lives behind `#[cfg(feature = "canon")]` in Cargo.toml. When the feature is disabled (default), no EDSDK code compiles and the app behaves identically to today.

**Rationale:** EDSDK DLLs are not freely redistributable. Feature-gating keeps the default build clean and CI green without DLLs present.

### Decision 2: FFI layer with safe Rust wrapper

Raw EDSDK C function signatures are declared in an `ffi` module using `extern "C"`. A safe `EdsSdk` wrapper struct provides idiomatic Rust methods that handle error codes, null checks, and resource cleanup via RAII (`Drop` impls for camera refs, evf image refs, etc.).

```rust
// Raw FFI (unsafe)
extern "C" {
    fn EdsInitializeSDK() -> EdsError;
    fn EdsGetCameraList(list: *mut EdsCameraListRef) -> EdsError;
    // ...
}

// Safe wrapper
pub struct EdsSdk { /* initialised SDK handle */ }
impl EdsSdk {
    pub fn new() -> Result<Self> { /* EdsInitializeSDK */ }
    pub fn camera_list(&self) -> Result<Vec<EdsCameraRef>> { /* ... */ }
}
impl Drop for EdsSdk {
    fn drop(&mut self) { /* EdsTerminateSDK */ }
}
```

**Rationale:** Separating raw FFI from safe wrappers makes the unsafe surface area small and auditable. RAII ensures SDK/session cleanup even on panics.

### Decision 3: Mock EDSDK via trait abstraction

The safe `EdsSdk` wrapper implements an `EdsSdkApi` trait. A `MockEdsSdk` implements the same trait for testing. `CanonBackend` is generic over `impl EdsSdkApi`, so tests inject the mock.

```rust
pub trait EdsSdkApi: Send + Sync {
    fn camera_list(&self) -> Result<Vec<MockableCameraRef>>;
    fn open_session(&self, camera: &MockableCameraRef) -> Result<()>;
    fn get_device_info(&self, camera: &MockableCameraRef) -> Result<DeviceInfo>;
    fn start_live_view(&self, camera: &MockableCameraRef) -> Result<()>;
    fn download_evf_image(&self, camera: &MockableCameraRef) -> Result<Vec<u8>>;
    fn get_property(&self, camera: &MockableCameraRef, prop: PropertyId) -> Result<PropertyValue>;
    fn set_property(&self, camera: &MockableCameraRef, prop: PropertyId, value: PropertyValue) -> Result<()>;
    fn close_session(&self, camera: &MockableCameraRef) -> Result<()>;
}
```

**Rationale:** Trait-based mocking allows full TDD without any EDSDK DLLs. The mock can simulate multiple cameras, property changes, live view frames, and error conditions.

### Decision 4: Canon-specific ControlId variants

Extend `ControlId` enum with Canon-specific variants:

- `Iso` — ISO sensitivity (select: 100, 200, 400, ...)
- `Aperture` — f-stop (select: f/1.4, f/2.0, f/2.8, ...)
- `ShutterSpeed` — exposure time (select: 1/4000, 1/2000, ...)
- `ExposureCompensation` — EV offset (slider: -3.0 to +3.0)

These use `ControlType::Select` with `options` arrays for enumerated values (ISO, aperture, shutter speed) and `ControlType::Slider` for continuous values (exposure compensation). The existing `ControlRenderer` frontend component already handles both types.

**Rationale:** Reusing the existing `ControlDescriptor` + dynamic rendering system means zero frontend changes. Canon controls "just work" in the UI.

### Decision 5: Composite backend pattern

A new `CompositeBackend` struct holds multiple `Box<dyn CameraBackend>` instances and implements `CameraBackend` itself by merging results:

```rust
pub struct CompositeBackend {
    backends: Vec<Box<dyn CameraBackend>>,
}

impl CameraBackend for CompositeBackend {
    fn enumerate_devices(&self) -> Result<Vec<CameraDevice>> {
        let mut all = Vec::new();
        for backend in &self.backends {
            match backend.enumerate_devices() {
                Ok(devices) => all.extend(devices),
                Err(e) => tracing::warn!("Backend enumeration failed: {e}"),
            }
        }
        Ok(all)
    }
    // ... route get_controls/set_control to the correct backend by device ID prefix
}
```

**Rationale:** This cleanly separates backend concerns. Each backend owns its devices. The composite routes operations by device ID prefix (e.g. `canon:` vs `046d:`). Adding future backends (PTP/libgphoto2, RTSP) follows the same pattern.

### Decision 6: Device ID scheme for Canon cameras

Canon device IDs use the format `canon:<serial>` where `<serial>` comes from EDSDK `EdsGetDeviceInfo`. This is stable across reconnections and USB ports.

**Rationale:** Canon cameras report their serial number via EDSDK. Using it directly (prefixed with `canon:`) provides natural stability for settings persistence without hashing.

### Decision 7: Live view polling thread

Canon live view runs on a dedicated thread per camera, polling `EdsDownloadEvfImage()` at ~200ms intervals (~5fps, matching Canon's hardware limit). JPEG frames are pushed directly into the existing `FrameBuffer`.

Since Canon live view delivers JPEG natively, frames skip the RGB-to-JPEG compression step used by DirectShow. The preview pipeline already handles JPEG input via the `Frame` struct.

**Rationale:** EDSDK live view is pull-based (no callback). A polling thread with configurable interval keeps the architecture simple. 200ms matches Canon's typical live view refresh rate.

### Decision 8: COM threading for EDSDK

EDSDK requires COM initialisation on Windows. Like DirectShow, EDSDK calls must happen on the same COM apartment. The Canon backend initialises COM (STA) on its dedicated thread, matching the existing pattern in `WindowsBackend` where COM is initialised per-thread in `enumerate_directshow_devices()` and `find_device_filter()`.

**Rationale:** Avoids the COM apartment mismatch that caused the STA/MTA panic in the DirectShow backend (see MEMORY.md COM root cause chain). Each backend thread owns its COM initialisation.

## Risks / trade-offs

- **EDSDK redistribution**: Canon EDSDK DLLs cannot be bundled in releases without a licence agreement. The app must either prompt users to install EDSDK separately or ship without Canon support until the licence is secured. Feature-gating mitigates this.
- **EDSDK version coupling**: EDSDK FFI signatures are tied to a specific SDK version. Version changes may break FFI. Mitigation: pin to a known SDK version; version check at runtime.
- **Single-client access**: EDSDK claims exclusive access to the camera. If another app (e.g. Canon EOS Utility) holds the connection, our app cannot connect. Clear error messaging is essential.
- **Live view resolution**: Canon live view is typically 960x640 to 1056x704, much lower than DirectShow webcam resolutions. The UI should indicate this is a live view preview, not full-resolution capture.
- **COM thread management**: Two COM-using backends (DirectShow + EDSDK) on separate threads adds complexity. Each must manage its own COM lifecycle without interfering with the other or with Tauri's main thread.

## Open questions

- **EDSDK macOS support**: Canon provides macOS EDSDK with a different API surface (Cocoa-based). Should we plan for macOS Canon support in this change or defer?
- **Multiple Canon cameras**: Does EDSDK support simultaneous sessions with multiple Canon bodies? Need to verify when DLLs are available.
- **EDSDK event loop**: EDSDK events require `EdsGetEvent()` to be called periodically or a Windows message pump. Should this share the live view polling thread or run separately?
