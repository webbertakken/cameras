## ADDED Requirements

### Requirement: Two-tier settings architecture (native vs software processing)

The system SHALL separate camera settings into two distinct tiers: **native hardware controls** (zero processing cost, applied directly by the camera) and **software processing controls** (CPU/GPU cost, applied in the app's processing pipeline). Native controls SHALL always appear first. Software processing controls SHALL be in a separate "Additional settings" section that the user must explicitly enable.

#### Scenario: User views settings for a camera

- **WHEN** the user selects a camera
- **THEN** native hardware controls (brightness, contrast, saturation, white balance, exposure, gain, sharpness, zoom, focus, etc.) are shown first in accordion sections
- **AND** below them, a clearly labelled "Additional settings" section is shown in a disabled/collapsed state with a toggle to enable it
- **AND** the additional settings section indicates it uses CPU/GPU processing

#### Scenario: User enables additional settings

- **WHEN** the user toggles "Additional settings" on
- **THEN** the software processing controls expand (colour grading, LUT, digital zoom, overlays)
- **AND** the software processing pipeline activates for that camera
- **AND** a subtle indicator shows that processing is active (e.g. a small badge or icon)

#### Scenario: User disables additional settings

- **WHEN** the user toggles "Additional settings" off
- **THEN** all software processing is bypassed — the feed is raw from the camera with only native hardware controls applied
- **AND** the additional settings section collapses back to its disabled state
- **AND** any virtual camera output reverts to the unprocessed feed

#### Scenario: Additional settings preference persists per camera

- **WHEN** the user enables additional settings for Camera A but not Camera B
- **THEN** switching to Camera B shows additional settings disabled
- **AND** switching back to Camera A shows additional settings enabled with previous software processing values restored

### Requirement: Accordion-based settings layout

Within each tier (native and additional), the system SHALL organise controls into collapsible accordion sections, implementing progressive disclosure so users see only what matters at each moment.

#### Scenario: User views native settings for a camera with many controls

- **WHEN** the user selects a camera that supports 15+ native hardware controls
- **THEN** native controls are grouped into accordion sections (e.g. "Image", "Exposure & White Balance", "Focus & Zoom", "Advanced")
- **AND** only the most commonly used section is expanded by default

#### Scenario: User views additional settings when enabled

- **WHEN** the user has enabled additional settings
- **THEN** software controls are grouped into accordion sections (e.g. "Colour Grading", "LUT", "Digital Zoom", "Overlays")

#### Scenario: User expands a section

- **WHEN** the user clicks on a collapsed "Focus & Zoom" section
- **THEN** that section expands to reveal its controls
- **AND** other sections remain in their current state (not auto-collapsed)

#### Scenario: Camera with few controls

- **WHEN** the user selects a basic camera with only 3 controls
- **THEN** controls are shown in a single expanded section without unnecessary grouping

### Requirement: Dynamic control rendering

The system SHALL render controls dynamically based on the selected camera's capabilities. Controls that the camera does not support SHALL be shown greyed out and disabled with a tooltip explaining "Not supported by [Camera Name]".

#### Scenario: Switching between cameras with different capabilities

- **WHEN** the user switches from a Logitech Brio (15 controls) to a basic laptop webcam (3 controls)
- **THEN** the settings panel immediately updates — the 3 supported controls are active, and remaining known controls are greyed out with tooltips
- **AND** accordion sections that contain only unsupported controls are collapsed by default

### Requirement: Responsive layout

The settings panel SHALL adapt gracefully to different window sizes, from the floating widget's compact size to a maximised full-screen panel.

#### Scenario: User resizes window to narrow width

- **WHEN** the window is resized below 800px wide
- **THEN** the sidebar collapses to show only camera thumbnails (no names)
- **AND** controls remain usable with appropriate touch targets

#### Scenario: User maximises window

- **WHEN** the window is maximised on a 1920x1080 display
- **THEN** the layout uses the space efficiently with the sidebar, preview, and settings panel all visible

### Requirement: Keyboard navigation

All controls SHALL be fully navigable via keyboard with a logical tab order, visible focus indicators, and keyboard-operable sliders.

#### Scenario: User navigates controls via keyboard

- **WHEN** the user presses Tab to navigate through controls
- **THEN** focus moves through controls in a logical top-to-bottom, section-by-section order
- **AND** each focused element has a clearly visible focus indicator

#### Scenario: User adjusts a slider via keyboard

- **WHEN** the user focuses a slider and presses the arrow keys
- **THEN** the slider value changes by one step per keypress
- **AND** holding Shift+Arrow changes by 10 steps

### Requirement: Screen reader support

All controls, sections, and interactive elements SHALL have appropriate ARIA labels and roles for screen reader compatibility.

#### Scenario: Screen reader reads a slider control

- **WHEN** a screen reader focuses the brightness slider
- **THEN** it announces the control name ("Brightness"), current value, and range (e.g. "Brightness, 128, slider, 0 to 255")

### Requirement: No cumulative layout shift

The settings panel SHALL not cause layout shifts when controls load, sections expand/collapse, or cameras switch. Transitions SHALL use overlays or smooth animations that do not push content.

#### Scenario: Camera switch with different control count

- **WHEN** the user switches from a camera with 15 controls to one with 3
- **THEN** the transition does not cause visible content jumping or layout reflow
- **AND** the change is animated smoothly
