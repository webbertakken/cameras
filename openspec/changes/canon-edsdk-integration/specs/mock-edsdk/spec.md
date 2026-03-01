## ADDED requirements

### Requirement: EdsSdkApi trait for testability

The system SHALL define an `EdsSdkApi` trait abstracting all EDSDK operations used by `CanonBackend`. Both the real `EdsSdk` and the test `MockEdsSdk` implement this trait.

#### Scenario: Trait completeness

- **WHEN** `CanonBackend` is instantiated
- **THEN** it accepts any `impl EdsSdkApi`
- **AND** all EDSDK operations used by the backend are covered by the trait

### Requirement: MockEdsSdk test double

The system SHALL provide a `MockEdsSdk` that simulates EDSDK behaviour for testing:

- Configurable camera list (0, 1, or multiple cameras)
- Simulated device info (model name, serial number)
- Simulated live view frame delivery (returns configurable JPEG bytes)
- Simulated property read/write with in-memory storage
- Configurable error injection for any operation

#### Scenario: Mock returns configured cameras

- **WHEN** `MockEdsSdk` is configured with 2 cameras
- **THEN** `camera_list()` returns 2 camera refs
- **AND** each has distinct device info (name, serial)

#### Scenario: Mock simulates live view

- **WHEN** `download_evf_image()` is called on the mock
- **THEN** it returns a valid JPEG byte array (configurable)
- **AND** sequential calls return frames (simulating a frame stream)

#### Scenario: Mock error injection

- **WHEN** the mock is configured to fail on `open_session`
- **THEN** `open_session()` returns `Err(CameraError::...)`
- **AND** `CanonBackend` propagates the error correctly

## Technical notes

- Mock in `src-tauri/src/camera/canon/mock.rs`
- Uses builder pattern for configuration: `MockEdsSdk::builder().with_cameras(2).with_live_view_frame(jpeg_bytes).build()`
- Mock is `Send + Sync` for use in async test contexts
