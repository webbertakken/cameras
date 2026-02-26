## ADDED Requirements

### Requirement: Hardware zoom control
The system SHALL expose the camera's hardware optical zoom control when supported.

#### Scenario: Camera supports optical zoom
- **WHEN** the selected camera reports hardware zoom capability
- **THEN** a zoom slider is shown with the hardware's min/max range
- **AND** adjusting the slider changes the camera's optical zoom in real time

#### Scenario: Camera does not support optical zoom
- **WHEN** the selected camera has no hardware zoom
- **THEN** the hardware zoom slider is not shown

### Requirement: Digital zoom
The system SHALL provide software-based digital zoom for all cameras, including those without hardware zoom. Digital zoom SHALL crop and scale the camera feed.

#### Scenario: User applies digital zoom
- **WHEN** the user increases the digital zoom level
- **THEN** the preview shows a cropped and upscaled view of the camera feed
- **AND** the zoom level is displayed as a percentage or multiplier (e.g. 2x)

#### Scenario: Digital zoom combined with hardware zoom
- **WHEN** a camera supports hardware zoom and the user applies digital zoom on top
- **THEN** the hardware zoom is applied first, and digital zoom further crops the result
- **AND** the UI clearly differentiates between hardware and digital zoom levels

### Requirement: Hardware focus control
The system SHALL expose the camera's hardware focus control when supported, with auto-focus toggle.

#### Scenario: Camera supports auto-focus
- **WHEN** the selected camera reports auto-focus capability
- **THEN** an auto-focus toggle and manual focus slider are shown
- **AND** when auto-focus is enabled, the manual slider is disabled

#### Scenario: Manual focus adjustment
- **WHEN** the user disables auto-focus and adjusts the focus slider
- **THEN** the camera's focus is updated in real time

### Requirement: Click-to-focus (region of interest)
The system SHALL allow the user to click a point in the preview to set the auto-focus region of interest, directing the camera to focus on that area.

#### Scenario: User clicks to focus
- **WHEN** the user clicks a point in the camera preview while auto-focus is enabled
- **THEN** the camera's auto-focus targets the clicked region
- **AND** a brief visual indicator appears at the click point in the preview

#### Scenario: Camera does not support ROI focus
- **WHEN** the selected camera does not support region-of-interest auto-focus
- **THEN** clicking the preview does not trigger any focus action
- **AND** the click-to-focus affordance is not shown
