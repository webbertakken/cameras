## ADDED requirements

### Requirement: Canon camera enumeration via EDSDK

The system SHALL enumerate Canon cameras using `EdsGetCameraList()` → `EdsGetChildCount()` → `EdsGetChildAtIndex()` → `EdsGetDeviceInfo()`.

#### Scenario: Canon DSLR detected

- **WHEN** a Canon EOS camera is connected via USB
- **AND** `enumerate_devices()` is called
- **THEN** the camera appears in the returned device list
- **AND** the device name matches the EDSDK-reported model (e.g. "Canon EOS R5")
- **AND** `is_connected` is `true`

#### Scenario: No Canon cameras connected

- **WHEN** no Canon cameras are connected
- **AND** `enumerate_devices()` is called
- **THEN** an empty list is returned (not an error)

#### Scenario: Multiple Canon cameras

- **WHEN** 2 Canon cameras are connected
- **THEN** both appear in the device list with distinct IDs

### Requirement: Stable Canon device IDs

The system SHALL generate device IDs in the format `canon:<serial_number>` using the serial number from `EdsGetDeviceInfo`. If no serial is available, fall back to `canon:<model_hash>`.

#### Scenario: Same camera reconnected

- **WHEN** a Canon EOS R5 with serial "012345" is disconnected and reconnected
- **THEN** it receives the same `DeviceId("canon:012345")`
- **AND** persisted settings are available

#### Scenario: No serial number available

- **WHEN** a Canon camera reports no serial number
- **THEN** the device ID falls back to `canon:<hash_of_model_name>`

## Technical notes

- Discovery in `src-tauri/src/camera/canon/discovery.rs`
- EDSDK `EdsDeviceInfo` struct contains `szDeviceDescription` (model name) and `szPortName` (connection port)
- Serial number accessed via `EdsGetPropertyData(camera, kEdsPropID_BodyIDEx)`
