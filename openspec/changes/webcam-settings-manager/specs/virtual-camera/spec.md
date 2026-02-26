## ADDED Requirements

### Requirement: Virtual camera output device
The system SHALL create a virtual camera device that other applications (OBS, Zoom, Discord, Google Meet, etc.) can select as a video input source. The virtual camera SHALL output the camera feed with all adjustments applied (hardware controls, software colour, LUT).

#### Scenario: User enables virtual camera
- **WHEN** the user toggles the virtual camera output on
- **THEN** a virtual camera device appears in the system's camera device list
- **AND** other applications can select this virtual camera as their video input

#### Scenario: Virtual camera feed includes all processing
- **WHEN** the virtual camera is active and the user has hardware brightness at 140, a warm colour temperature, and a LUT applied
- **THEN** the virtual camera feed includes all of those adjustments
- **AND** consuming applications receive the fully processed feed

#### Scenario: User disables virtual camera
- **WHEN** the user toggles the virtual camera off
- **THEN** the virtual camera device is removed from the system's device list
- **AND** any application currently using it loses the feed gracefully

### Requirement: Virtual camera resolution
The virtual camera SHALL output at the same resolution and frame rate as the source camera's current configuration.

#### Scenario: Source camera at 1080p 30fps
- **WHEN** the source camera is set to 1920x1080 at 30 fps
- **THEN** the virtual camera outputs at 1920x1080 at 30 fps

### Requirement: Virtual camera naming
The virtual camera device SHALL be named in a way that is recognisable to users in other applications (e.g. "Webcam Settings Manager - [Camera Name]").

#### Scenario: Virtual camera name in app picker
- **WHEN** a user opens Zoom's camera selection
- **THEN** the virtual camera appears with a descriptive name like "Webcam Settings Manager - Logitech Brio"
