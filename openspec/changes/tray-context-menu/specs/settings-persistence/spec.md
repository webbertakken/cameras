## MODIFIED Requirements

### Requirement: Auto-persist last settings per camera

The system SHALL automatically save the last-applied settings for each camera, keyed by the camera's stable identifier, without requiring explicit user action.

#### Scenario: User adjusts settings and closes the app

- **WHEN** the user adjusts brightness to 150 and contrast to 60 on Camera A, then closes the app
- **THEN** those values are persisted to disk

#### Scenario: Settings persist across sessions

- **WHEN** the user reopens the app with Camera A connected
- **THEN** brightness is restored to 150 and contrast to 60

#### Scenario: Camera name passed from frontend for persistence

- **WHEN** a control value is set via `set_camera_control`
- **THEN** the camera name SHALL be provided by the frontend as a command parameter
- **AND** the backend SHALL NOT call `enumerate_devices()` to look up the name

#### Scenario: Debounce saves without losing changes

- **WHEN** a control value is changed during the gap between a save completing and the next `notified()` registration
- **THEN** the change SHALL still be persisted after the debounce period
- **AND** no changes SHALL be silently dropped due to the race window
