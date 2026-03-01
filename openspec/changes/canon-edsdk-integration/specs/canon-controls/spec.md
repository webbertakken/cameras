## ADDED requirements

### Requirement: Canon property mapping to ControlDescriptor

The system SHALL map Canon EDSDK properties to `ControlDescriptor` values compatible with the existing dynamic control rendering system.

**Context**: Canon cameras expose properties via `EdsGetPropertyData()` / `EdsSetPropertyData()`. Unlike UVC controls (continuous integer ranges), Canon properties often have enumerated allowed values (e.g. ISO: 100, 200, 400, 800...).

#### Scenario: ISO control

- **WHEN** controls are requested for a Canon camera
- **THEN** an ISO control appears with `control_type: Select`
- **AND** `options` contains the camera's supported ISO values (e.g. 100, 200, 400, 800, 1600, 3200)
- **AND** `current` reflects the camera's current ISO setting

#### Scenario: Aperture control

- **WHEN** controls are requested for a Canon camera with a lens attached
- **THEN** an Aperture control appears with `control_type: Select`
- **AND** `options` contains the lens's supported f-stops
- **AND** if no lens is attached, the control is marked `supported: false`

#### Scenario: Shutter speed control

- **WHEN** controls are requested for a Canon camera in Manual mode
- **THEN** a Shutter Speed control appears with `control_type: Select`
- **AND** `options` contains values like "1/4000", "1/2000", ..., "1", "2", "30", "Bulb"

#### Scenario: White balance control

- **WHEN** controls are requested for a Canon camera
- **THEN** a White Balance control appears with `control_type: Select`
- **AND** `options` contains modes: Auto, Daylight, Cloudy, Tungsten, Fluorescent, Flash, Custom

#### Scenario: Exposure compensation control

- **WHEN** controls are requested for a Canon camera
- **THEN** an Exposure Compensation control appears with `control_type: Slider`
- **AND** `min`/`max` reflect the camera's EV range (typically -3 to +3, in 1/3 stop increments)

#### Scenario: Setting a Canon control

- **WHEN** the user changes ISO to 800 via the UI
- **THEN** `set_control()` calls `EdsSetPropertyData(camera, kEdsPropID_ISOSpeed, 0, iso_800_value)`
- **AND** the camera applies the change
- **AND** subsequent `get_controls()` reflects the new value

#### Scenario: Read-only property

- **WHEN** a Canon property is not settable in the current camera mode
- **THEN** the `ControlDescriptor` has `flags.is_read_only: true`
- **AND** the frontend renders it as disabled

### Requirement: Canon ControlId variants

The system SHALL extend `ControlId` with Canon-specific variants: `Iso`, `Aperture`, `ShutterSpeed`, `ExposureCompensation`. These use a `canon_` prefix in their `as_id_str()` representation to avoid collisions with UVC controls.

#### Scenario: Canon controls in separate group

- **WHEN** Canon controls are rendered
- **THEN** they appear in a "Canon" or "Camera" group in the accordion UI
- **AND** they are distinct from UVC image/exposure groups

## Technical notes

- Controls in `src-tauri/src/camera/canon/controls.rs`
- EDSDK property IDs: `kEdsPropID_ISOSpeed`, `kEdsPropID_Av`, `kEdsPropID_Tv`, `kEdsPropID_WhiteBalance`, `kEdsPropID_ExposureCompensation`
- Enumerated values: EDSDK provides `EdsGetPropertyDesc()` which returns the list of allowed values for a property
- The `ControlDescriptor` already has `min`/`max`/`step` for sliders. For select controls, we'll use the existing structure but add an `options` field (already present in the design document's TypeScript interface but not yet in the Rust struct — needs adding)
