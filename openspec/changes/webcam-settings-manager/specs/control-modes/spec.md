## ADDED Requirements

### Requirement: Three-mode control system for white balance
The system SHALL offer three control modes for white balance: full auto, semi-auto (auto with user-defined limits), and full manual.

#### Scenario: Full auto white balance
- **WHEN** the user selects "Auto" mode for white balance
- **THEN** the camera's automatic white balance is enabled
- **AND** the white balance slider is disabled and shows the camera's auto-determined value in real time

#### Scenario: Semi-auto white balance
- **WHEN** the user selects "Semi-auto" mode for white balance
- **THEN** the camera's automatic white balance is enabled
- **AND** the user can set a temperature range (min/max) within which auto adjustment operates

#### Scenario: Full manual white balance
- **WHEN** the user selects "Manual" mode for white balance
- **THEN** the camera's automatic white balance is disabled
- **AND** the user has full slider control over the white balance temperature value

### Requirement: Three-mode control system for exposure
The system SHALL offer three control modes for exposure: full auto, semi-auto (auto with limits), and full manual.

#### Scenario: Full auto exposure
- **WHEN** the user selects "Auto" mode for exposure
- **THEN** the camera's auto-exposure is enabled
- **AND** the exposure slider is disabled and reflects the camera's auto-determined value

#### Scenario: Semi-auto exposure
- **WHEN** the user selects "Semi-auto" mode for exposure
- **THEN** auto-exposure is enabled but the user can define an exposure range to constrain the auto algorithm

#### Scenario: Full manual exposure
- **WHEN** the user selects "Manual" mode for exposure
- **THEN** auto-exposure is disabled
- **AND** the user has full control over the exposure value via the slider

### Requirement: Auto-set-once lock
The system SHALL provide an "auto-set once" button that reads the current auto-determined value and locks it as a manual value.

#### Scenario: User locks auto white balance value
- **WHEN** white balance is in auto mode and the user clicks "Lock current value"
- **THEN** the mode switches to manual
- **AND** the slider is set to the value that auto had determined at the moment of clicking
