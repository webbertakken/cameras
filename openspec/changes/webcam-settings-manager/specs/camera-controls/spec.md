## ADDED Requirements

### Requirement: Dynamic UVC control rendering

The system SHALL dynamically enumerate all UVC controls supported by the selected camera and render appropriate UI controls for each. The UI MUST NOT hardcode a fixed set of controls — it SHALL adapt to whatever the camera hardware exposes.

#### Scenario: Camera with standard controls

- **WHEN** a camera is selected that supports brightness, contrast, saturation, white balance, exposure, gain, and sharpness
- **THEN** the settings panel renders a slider for each of those controls
- **AND** each slider reflects the control's min, max, step, and current value from the hardware

#### Scenario: Camera with extended controls

- **WHEN** a camera is selected that supports pan, tilt, gamma, hue, backlight compensation, and powerline frequency in addition to standard controls
- **THEN** all supported controls are rendered in the settings panel
- **AND** controls are grouped into logical accordion sections

#### Scenario: Control not supported by camera

- **WHEN** a camera does not support a particular UVC control (e.g. pan/tilt)
- **THEN** that control is shown greyed out and disabled in the settings panel
- **AND** a tooltip on the disabled control explains "Not supported by [Camera Name]"

### Requirement: Real-time control application

The system SHALL apply control value changes to the camera hardware in real time as the user adjusts sliders, without requiring an explicit "apply" action.

#### Scenario: User adjusts brightness slider

- **WHEN** the user drags the brightness slider to a new value
- **THEN** the camera's brightness is updated in real time
- **AND** the preview reflects the change immediately

#### Scenario: Control value out of range

- **WHEN** a control value cannot be set (hardware rejects it)
- **THEN** the slider reverts to the last valid value
- **AND** an inline error message is displayed

### Requirement: Control value display

Each control slider SHALL display its current numeric value and allow direct numeric input for precise adjustment.

#### Scenario: User types a specific value

- **WHEN** the user clicks the numeric display next to a slider and types "128"
- **THEN** the slider moves to 128 and the camera updates accordingly

#### Scenario: Typed value exceeds control range

- **WHEN** the user types a value outside the control's min/max range
- **THEN** the value is clamped to the nearest valid value
- **AND** the input field shows the clamped value

### Requirement: Reset individual control

The system SHALL allow the user to reset any individual control to its hardware default value.

#### Scenario: User resets brightness to default

- **WHEN** the user clicks the reset button next to the brightness control
- **THEN** the brightness value is set to the camera's reported default
- **AND** the slider and preview update accordingly
