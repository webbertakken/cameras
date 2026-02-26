## ADDED Requirements

### Requirement: Software colour correction controls

The system SHALL provide software-based colour correction controls that process the camera feed before output. These controls operate independently of hardware UVC controls.

#### Scenario: User adjusts colour temperature

- **WHEN** the user adjusts the software colour temperature slider toward warm
- **THEN** the preview shows a warmer colour cast applied in software
- **AND** the adjustment is applied on top of any hardware white balance setting

#### Scenario: User adjusts tint

- **WHEN** the user adjusts the tint slider toward green
- **THEN** the preview reflects the tint shift
- **AND** the effect is visible in real time

### Requirement: RGB balance controls

The system SHALL provide individual red, green, and blue channel level controls for fine colour adjustment.

#### Scenario: User boosts the red channel

- **WHEN** the user increases the red channel level
- **THEN** the preview shows increased red intensity
- **AND** the adjustment is applied in the software colour pipeline

### Requirement: LUT file import and application

The system SHALL support importing .cube LUT (lookup table) files and applying them to the camera feed as a colour grading effect.

#### Scenario: User imports a .cube LUT file

- **WHEN** the user imports a 3D LUT file in .cube format
- **THEN** the LUT is added to the LUT library and available for selection

#### Scenario: User applies a LUT

- **WHEN** the user selects a LUT from the library
- **THEN** the LUT colour transform is applied to the camera feed in real time
- **AND** the preview shows the graded result

#### Scenario: User removes a LUT

- **WHEN** the user deselects the active LUT (sets to "None")
- **THEN** the LUT is no longer applied to the feed
- **AND** the preview reverts to the non-graded image

### Requirement: Colour pipeline ordering

The software colour pipeline SHALL process in a defined order: hardware controls first, then software colour correction (temperature, tint, RGB), then LUT application.

#### Scenario: Full colour pipeline active

- **WHEN** the user has hardware brightness at 140, software temperature shifted warm, and a cinematic LUT applied
- **THEN** the preview shows the result of all three stages applied in order
- **AND** the virtual camera output (if enabled) includes all colour processing

### Requirement: Colour correction bypass

The system SHALL allow the user to toggle the entire software colour pipeline on or off with a single control, for quick A/B comparison.

#### Scenario: User toggles colour processing off

- **WHEN** the user disables the colour correction bypass toggle
- **THEN** all software colour adjustments and LUTs are temporarily removed from the preview
- **AND** only hardware controls remain active

#### Scenario: User re-enables colour processing

- **WHEN** the user re-enables the colour correction toggle
- **THEN** all previously configured software adjustments and LUT are re-applied
