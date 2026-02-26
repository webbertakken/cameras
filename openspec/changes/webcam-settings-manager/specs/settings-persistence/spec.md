## ADDED Requirements

### Requirement: Auto-persist last settings per camera

The system SHALL automatically save the last-applied settings for each camera, keyed by the camera's stable identifier, without requiring explicit user action.

#### Scenario: User adjusts settings and closes the app

- **WHEN** the user adjusts brightness to 150 and contrast to 60 on Camera A, then closes the app
- **THEN** those values are persisted to disk

#### Scenario: Settings persist across sessions

- **WHEN** the user reopens the app with Camera A connected
- **THEN** brightness is restored to 150 and contrast to 60

### Requirement: Auto-apply settings on camera connect

The system SHALL automatically apply the last saved settings to a recognised camera when it is detected (on launch or hot-plug).

#### Scenario: Known camera detected on launch

- **WHEN** the app launches and detects a previously configured Camera A
- **THEN** the saved settings for Camera A are applied to the hardware automatically
- **AND** a toast notification confirms "Settings restored for Camera A"

#### Scenario: Known camera hot-plugged

- **WHEN** Camera A is plugged in while the app is running and has saved settings
- **THEN** the saved settings are applied within 3 seconds of detection

### Requirement: Reset to camera defaults

The system SHALL allow the user to reset all controls to the camera's hardware-reported default values.

#### Scenario: User resets all settings

- **WHEN** the user clicks "Reset to defaults"
- **THEN** all controls are set to the camera's reported default values
- **AND** the preview updates to reflect the defaults
- **AND** a confirmation dialog is shown before resetting

### Requirement: Settings storage format

The system SHALL store settings in a user-accessible JSON file in the application's data directory, allowing manual backup and editing.

#### Scenario: Settings file location

- **WHEN** the user looks in the application data directory
- **THEN** a `cameras.json` (or similar) file exists containing per-camera settings keyed by camera identifier
