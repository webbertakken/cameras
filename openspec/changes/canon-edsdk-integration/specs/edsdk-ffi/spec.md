## ADDED requirements

### Requirement: EDSDK FFI bindings

The system SHALL provide Rust FFI bindings for Canon EDSDK functions, compiled only when the `canon` Cargo feature is enabled.

**Context**: EDSDK is a C library. Rust needs `extern "C"` declarations for each function used. All unsafe code is isolated in this module; the rest of the codebase uses safe wrappers.

#### Scenario: SDK lifecycle

- **WHEN** `EdsInitializeSDK()` is called via the FFI layer
- **THEN** the SDK is initialised and returns `EDS_ERR_OK` (0)
- **AND** `EdsTerminateSDK()` cleans up on drop

#### Scenario: Error code mapping

- **WHEN** any EDSDK function returns a non-zero `EdsError`
- **THEN** the FFI wrapper maps it to a `CameraError` variant with a human-readable message
- **AND** common error codes (`EDS_ERR_DEVICE_BUSY`, `EDS_ERR_SESSION_NOT_OPEN`, `EDS_ERR_TAKE_PICTURE_AF_NG`) have specific messages

#### Scenario: Feature gate

- **WHEN** the `canon` feature is disabled (default)
- **THEN** no EDSDK code compiles
- **AND** the app builds and runs identically to the current state

### Requirement: Safe EDSDK wrapper

The system SHALL provide a safe Rust wrapper (`EdsSdk` struct) around the raw FFI, implementing RAII for SDK init/terminate, camera sessions, and evf image refs.

#### Scenario: RAII cleanup

- **WHEN** an `EdsSdk` instance is dropped
- **THEN** `EdsTerminateSDK()` is called exactly once
- **AND** any open camera sessions are closed first

## Technical notes

- FFI declarations in `src-tauri/src/camera/canon/ffi.rs`
- Safe wrapper in `src-tauri/src/camera/canon/sdk.rs`
- EDSDK types: `EdsError` (u32), `EdsBaseRef` (\*mut c_void), `EdsCameraRef`, `EdsCameraListRef`, `EdsEvfImageRef`
- Link against `EDSDK.dll` at runtime (delay-loaded)
