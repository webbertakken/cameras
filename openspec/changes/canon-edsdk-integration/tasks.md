# Canon EDSDK integration tasks

## 0. Developer setup

### 0.1 Create `DEVELOPMENT.md`

- Create `DEVELOPMENT.md` in the repo root covering:
  - General dev setup: Rust (1.77.2+), Node (22.x via Volta), Yarn (4.x via Volta), Tauri v2 prerequisites (platform-specific system deps)
  - EDSDK setup: where to download the SDK (Canon developer portal), where to place DLLs (`src-tauri/lib/edsdk/`), how to build with `--features canon`, what happens without the SDK (mock-only, tests pass, feature-gated code skipped)
  - Placeholder sections for future SDKs (GoPro HTTP API, etc.)
- Keep concise — bullet points, no essays

### 0.2 Gitignore proprietary SDK DLLs

- Add `lib/edsdk/` to `src-tauri/.gitignore` to prevent accidental commits of proprietary Canon EDSDK DLLs
- Add a general `lib/` pattern or keep it SDK-specific for future GoPro etc.

## 1. Foundation

### 1.1 Add `canon` feature flag to Cargo.toml

- Add `[features] canon = []` to `src-tauri/Cargo.toml`
- Add `#[cfg(feature = "canon")]` module declaration in `src-tauri/src/camera/mod.rs`
- Create `src-tauri/src/camera/canon/mod.rs` with submodule declarations
- Verify: `cargo check` passes with and without `--features canon`

### 1.2 EDSDK type definitions

- Define EDSDK C types in `src-tauri/src/camera/canon/types.rs`: `EdsError` (u32), `EdsBaseRef`, `EdsCameraRef`, `EdsCameraListRef`, `EdsEvfImageRef`, `EdsPropertyID`, `EdsDeviceInfo`
- Define EDSDK error code constants (`EDS_ERR_OK`, `EDS_ERR_DEVICE_BUSY`, `EDS_ERR_SESSION_NOT_OPEN`, etc.)
- Define EDSDK property ID constants (`kEdsPropID_ISOSpeed`, `kEdsPropID_Av`, `kEdsPropID_Tv`, etc.)
- Define EDSDK command constants (`kEdsCameraCommand_TakePicture`, `kEdsCameraCommand_EvfMode`)
- Tests: verify error code values match EDSDK header, verify type sizes

### 1.3 EDSDK FFI declarations

- Declare `extern "C"` functions in `src-tauri/src/camera/canon/ffi.rs`: `EdsInitializeSDK`, `EdsTerminateSDK`, `EdsGetCameraList`, `EdsGetChildCount`, `EdsGetChildAtIndex`, `EdsOpenSession`, `EdsCloseSession`, `EdsGetDeviceInfo`, `EdsSendCommand`, `EdsCreateEvfImageRef`, `EdsDownloadEvfImage`, `EdsGetPropertyData`, `EdsSetPropertyData`, `EdsGetPropertyDesc`, `EdsSetCameraStateEventHandler`, `EdsGetEvent`, `EdsRelease`
- All behind `#[cfg(feature = "canon")]`
- Link directive for EDSDK.dll (delay-loaded)

### 1.4 Safe EDSDK wrapper (`EdsSdk`)

- Implement `EdsSdk` struct wrapping `EdsInitializeSDK`/`EdsTerminateSDK` with RAII
- Implement `Drop` for `EdsSdk` calling `EdsTerminateSDK`
- Implement methods: `camera_list()`, `open_session()`, `close_session()`, `get_device_info()`, `send_command()`, `create_evf_image_ref()`, `download_evf_image()`, `get_property()`, `set_property()`, `get_property_desc()`, `set_state_event_handler()`, `get_event()`, `release()`
- Map `EdsError` to `CameraError` with human-readable messages
- Tests: verify RAII cleanup, error mapping

## 2. Testability

### 2.1 `EdsSdkApi` trait

- Define trait in `src-tauri/src/camera/canon/api.rs` with all methods from the safe wrapper
- Implement `EdsSdkApi` for `EdsSdk` (delegates to real FFI)
- Use associated types or type-erased refs for camera handles
- Tests: verify trait is object-safe if needed, verify `Send + Sync` bounds

### 2.2 `MockEdsSdk` test double

- Implement `MockEdsSdk` in `src-tauri/src/camera/canon/mock.rs`
- Builder pattern: `.with_cameras(n)`, `.with_camera(name, serial)`, `.with_live_view_frame(jpeg_bytes)`, `.with_property(prop, value)`, `.with_error(operation, error)`
- In-memory property storage per camera
- Configurable error injection per operation
- Tests: verify mock returns configured cameras, verify property read/write, verify error injection

## 3. Canon backend core

### 3.1 `CanonBackend` struct

- Implement `CanonBackend<S: EdsSdkApi>` in `src-tauri/src/camera/canon/backend.rs`
- Internal state: `Arc<Mutex<HashMap<DeviceId, CanonCamera>>>` for tracking discovered cameras
- Implement `CameraBackend` trait: `enumerate_devices`, `watch_hotplug`, `get_controls`, `get_control`, `set_control`, `get_formats`
- `Send + Sync` verification
- Tests: construct with `MockEdsSdk`, verify all trait methods

### 3.2 Canon ControlId variants

- Add `Iso`, `Aperture`, `ShutterSpeed`, `ExposureCompensation` to `ControlId` enum
- Implement `display_name()`, `as_id_str()` (prefixed: `canon_iso`, `canon_aperture`, etc.), `group()` (return `"camera"`)
- Implement `from_str_id()` for the new variants
- Tests: roundtrip `as_id_str` <-> `from_str_id`, verify group assignment

### 3.3 Add `options` field to `ControlDescriptor`

- Add `options: Option<Vec<ControlOption>>` to `ControlDescriptor` struct
- Define `ControlOption { value: i32, label: String }`
- Ensure JSON serialisation matches frontend expectations
- Tests: verify serialisation with and without options

## 4. Discovery

### 4.1 Canon camera enumeration

- Implement `discover_cameras()` in `src-tauri/src/camera/canon/discovery.rs`
- Call `camera_list()` -> iterate with `get_device_info()` for each
- Generate `DeviceId` as `canon:<serial>` (fallback: `canon:<model_hash>`)
- Return `Vec<CameraDevice>` with `device_path: "edsdk://<model>"`
- Tests: mock with 0, 1, 2 cameras; verify device IDs; verify fallback when no serial

### 4.2 Integrate discovery into `CanonBackend::enumerate_devices`

- Wire `discover_cameras()` into the `CameraBackend::enumerate_devices` implementation
- Update internal camera map with newly discovered cameras
- Remove stale cameras from the map
- Tests: verify map updates on re-enumeration, verify stale removal

## 5. Live view

### 5.1 Live view polling thread

- Implement `LiveViewSession` in `src-tauri/src/camera/canon/live_view.rs`
- Spawn a thread that polls `download_evf_image()` at configurable intervals (default 200ms)
- Push JPEG frames into a `FrameBuffer`
- Stop signalling via `Arc<AtomicBool>` (same pattern as `CaptureSession`)
- Handle `EDS_ERR_OBJECT_NOTREADY` by retrying silently
- Tests: verify frames appear in buffer, verify stop signal works, verify OBJECT_NOTREADY is handled

### 5.2 JPEG passthrough in preview pipeline

- Ensure Canon live view JPEG frames can flow through the preview pipeline without re-encoding
- If `Frame` requires RGB data, add a JPEG path (either `Frame.jpeg_data` field or separate handling in preview commands)
- Tests: verify JPEG frames from Canon backend reach the frontend response without double-encoding

### 5.3 Start/stop live view integration

- Wire live view start/stop into `CanonBackend` (called when camera is selected/deselected)
- Call `EdsSendCommand(EvfMode, 1)` on start, `EdsSendCommand(EvfMode, 0)` on stop
- Clean up `LiveViewSession` on stop
- Tests: verify SDK commands are called in correct order, verify session cleanup

## 6. Controls

### 6.1 Canon property to ControlDescriptor mapping

- Implement property mapping in `src-tauri/src/camera/canon/controls.rs`
- Map `kEdsPropID_ISOSpeed` -> ISO select control (use `get_property_desc()` for allowed values)
- Map `kEdsPropID_Av` -> Aperture select control
- Map `kEdsPropID_Tv` -> Shutter Speed select control
- Map `kEdsPropID_WhiteBalance` -> White Balance select control
- Map `kEdsPropID_ExposureCompensation` -> Exposure Compensation slider control
- Tests: verify each mapping produces correct `ControlDescriptor`

### 6.2 Canon property read/write

- Implement `get_control()` reading current property value from EDSDK
- Implement `set_control()` writing property value to EDSDK
- Handle read-only properties (mark `is_read_only: true`)
- Handle lens-dependent properties (aperture unavailable without lens -> `supported: false`)
- Tests: verify read returns mock values, verify write calls SDK, verify read-only handling

### 6.3 EDSDK value translation

- Translate between EDSDK internal values and human-readable labels (e.g. EDSDK ISO value `0x48` -> "100", aperture `0x20` -> "f/2.8")
- Use EDSDK property description for available values
- Tests: verify translation for common ISO, aperture, and shutter speed values

## 7. Hotplug

### 7.1 EDSDK state event handler

- Register `EdsSetCameraStateEventHandler` for `kEdsStateEvent_Shutdown` on each discovered camera
- On disconnect event, remove camera from internal map and emit `HotplugEvent::Disconnected`
- Clean up EDSDK session and live view on disconnect
- Tests: simulate disconnect event via mock, verify hotplug event emitted, verify cleanup

### 7.2 Periodic re-enumeration for new connections

- Run periodic re-enumeration (every 3-5 seconds) on the EDSDK thread
- Compare new camera list with known cameras
- Emit `HotplugEvent::Connected` for newly detected cameras
- Tests: verify new camera detected after re-enumeration, verify no duplicate events

### 7.3 EDSDK event loop

- Call `EdsGetEvent()` periodically on the EDSDK thread to process pending events
- Integrate with the re-enumeration loop (share the same thread/timer)
- Tests: verify events are processed, verify thread shutdown

## 8. Composite backend

### 8.1 CompositeBackend implementation

- Implement `CompositeBackend` in `src-tauri/src/camera/composite.rs`
- Hold `Vec<Box<dyn CameraBackend>>`
- `enumerate_devices()`: merge device lists from all backends, log failures
- `get_controls()` / `get_control()` / `set_control()` / `get_formats()`: route to correct backend by trying each until one succeeds
- `watch_hotplug()`: register callback with all backends
- Tests: verify merged device lists, verify routing, verify failure isolation, verify hotplug aggregation

### 8.2 Wire CompositeBackend into app startup

- Update `create_camera_state()` in `lib.rs` to construct a `CompositeBackend`
- When `canon` feature is enabled: include `CanonBackend` in the composite
- When `DUMMY_CAMERA=1`: include `DummyBackend` in the composite
- Always include `WindowsBackend` (on Windows)
- Tests: verify composite is constructed correctly with different feature/env combinations

## 9. Integration and polish

### 9.1 CameraError variants for Canon

- Add Canon-specific error variants: `CanonSdkError(String)`, `CanonSessionNotOpen(String)`, `CanonDeviceBusy(String)`
- Map EDSDK error codes to these variants in the safe wrapper
- Add human-friendly messages to `humanise_error()`
- Tests: verify error mapping, verify humanised messages

### 9.2 CI configuration for Canon feature

- Add a `cargo check --features canon` step to `checks.yml` (allowed to fail until DLLs available)
- Document the Canon feature flag in project README or AGENTS.md
- Verify `cargo test --features canon` runs mock-based tests without real DLLs

### 9.3 End-to-end smoke test with MockEdsSdk

- Write an integration test that constructs a `CompositeBackend` with `MockEdsSdk`-backed `CanonBackend`
- Enumerate devices (mixed DirectShow mock + Canon mock)
- Read Canon controls, set a value, read back
- Start and stop live view, verify frames in buffer
- Tests: full lifecycle test covering discovery -> controls -> live view -> disconnect
