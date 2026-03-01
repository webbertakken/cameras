## ADDED requirements

### Requirement: CompositeBackend merges multiple backends

The system SHALL provide a `CompositeBackend` that implements `CameraBackend` by aggregating devices from multiple child backends. Operations are routed to the correct child backend based on device ID ownership.

#### Scenario: Mixed device list

- **WHEN** `enumerate_devices()` is called on the composite backend
- **AND** the DirectShow backend returns 2 webcams
- **AND** the Canon backend returns 1 Canon camera
- **THEN** 3 devices are returned in total

#### Scenario: Control routing

- **WHEN** `get_controls()` is called with a `canon:012345` device ID
- **THEN** the request is routed to the Canon backend
- **AND** Canon-specific controls are returned

#### Scenario: Control routing for DirectShow device

- **WHEN** `set_control()` is called with a `046d:085e:serial` device ID
- **THEN** the request is routed to the DirectShow backend

#### Scenario: Backend failure isolation

- **WHEN** the Canon backend fails during `enumerate_devices()` (e.g. EDSDK not initialised)
- **THEN** the composite backend still returns devices from other backends
- **AND** the Canon error is logged (not propagated as a total failure)

#### Scenario: Hotplug aggregation

- **WHEN** `watch_hotplug()` is called on the composite backend
- **THEN** the callback is registered with all child backends
- **AND** events from any backend are forwarded to the single callback

### Requirement: Device ID ownership resolution

The system SHALL determine which backend owns a device by asking each backend. The first backend that recognises a device ID handles the operation.

#### Scenario: Unknown device ID

- **WHEN** an operation is requested for a device ID not owned by any backend
- **THEN** `CameraError::DeviceNotFound` is returned

## Technical notes

- Composite in `src-tauri/src/camera/composite.rs` (not inside `canon/` — it's backend-agnostic)
- `CameraState` changes from `backend: Box<dyn CameraBackend>` to holding a `CompositeBackend`
- Device ownership: either by ID prefix convention (`canon:` vs UVC-style) or by attempting `get_controls()` on each backend until one succeeds (prefix is faster)
- The `DummyBackend` also participates in the composite when `DUMMY_CAMERA=1`
