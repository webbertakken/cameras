## ADDED requirements

### Requirement: CanonBackend implements CameraBackend

The system SHALL provide a `CanonBackend` struct that implements the `CameraBackend` trait, routing all operations through an `EdsSdkApi` implementation.

#### Scenario: Backend construction

- **WHEN** `CanonBackend::new(sdk)` is called with an `EdsSdkApi` implementation
- **THEN** the backend is ready to enumerate devices
- **AND** EDSDK is initialised (via the sdk instance)

#### Scenario: Backend is Send + Sync

- **WHEN** `CanonBackend` is constructed
- **THEN** it satisfies `Send + Sync` bounds (required by `CameraBackend` trait)
- **AND** it can be stored in `CameraState` alongside other backends

#### Scenario: Unknown device ID returns error

- **WHEN** `get_controls()` is called with a device ID not owned by this backend
- **THEN** `CameraError::DeviceNotFound` is returned

### Requirement: Canon device tracking

The system SHALL maintain an internal map of discovered Canon cameras, keyed by `DeviceId`, storing the EDSDK camera ref and cached device info.

#### Scenario: Device map populated on enumeration

- **WHEN** `enumerate_devices()` discovers 2 Canon cameras
- **THEN** subsequent `get_controls()` calls for those device IDs succeed
- **AND** the backend does not re-enumerate on every control access

## Technical notes

- Backend in `src-tauri/src/camera/canon/backend.rs`
- `CanonBackend<S: EdsSdkApi>` — generic over SDK implementation
- Internal state: `Arc<Mutex<HashMap<DeviceId, CanonCamera>>>` where `CanonCamera` holds the camera ref, device info, session state, and cached properties
