## ADDED Requirements

### Requirement: Resolution selection
The system SHALL allow the user to select the camera's output resolution from the list of resolutions the camera supports.

#### Scenario: User changes resolution
- **WHEN** the user selects "1920x1080" from the resolution dropdown
- **THEN** the camera switches to 1920x1080 output
- **AND** the preview updates to reflect the new resolution

#### Scenario: Resolution list populated from camera capabilities
- **WHEN** a camera is selected that supports 640x480, 1280x720, and 1920x1080
- **THEN** the resolution dropdown shows exactly those three options
- **AND** the current active resolution is pre-selected

### Requirement: Frame rate selection
The system SHALL allow the user to select the camera's frame rate from the rates supported at the current resolution.

#### Scenario: User changes frame rate
- **WHEN** the user selects 30 fps from the frame rate dropdown
- **THEN** the camera switches to 30 fps output
- **AND** the preview reflects the frame rate change

#### Scenario: Available frame rates change with resolution
- **WHEN** the user switches to a higher resolution that only supports 30 fps (not 60 fps)
- **THEN** the frame rate dropdown updates to show only the available rates for that resolution
- **AND** if the previously selected frame rate is no longer available, the closest valid rate is auto-selected

### Requirement: Pixel format selection
The system SHALL allow the user to select the pixel format (e.g. MJPEG, YUY2, NV12) from the formats the camera supports at the current resolution and frame rate.

#### Scenario: User selects MJPEG format
- **WHEN** the user selects "MJPEG" from the format dropdown
- **THEN** the camera switches to MJPEG output
- **AND** the preview pipeline adjusts to decode the selected format

#### Scenario: Format options update based on resolution/fps selection
- **WHEN** the user selects a resolution and frame rate combination
- **THEN** the format dropdown shows only the formats available for that combination
