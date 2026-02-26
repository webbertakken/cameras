## ADDED Requirements

### Requirement: Text overlay

The system SHALL allow the user to add text overlays to the camera feed. Text overlays SHALL include configurable content, font, size, colour, position, and opacity.

#### Scenario: User adds a text overlay

- **WHEN** the user creates a new text overlay with content "LIVE" and positions it in the top-right corner
- **THEN** the text "LIVE" appears on the preview and virtual camera output at the specified position
- **AND** the overlay is rendered on top of the camera feed after all colour processing

#### Scenario: User edits overlay properties

- **WHEN** the user changes the text overlay's font size and colour
- **THEN** the preview updates in real time to reflect the changes

### Requirement: Image overlay

The system SHALL allow the user to add image overlays (logos, watermarks) to the camera feed. Image overlays SHALL support PNG and JPEG files with configurable position, scale, and opacity.

#### Scenario: User adds a logo overlay

- **WHEN** the user imports a PNG logo and positions it in the bottom-left corner at 50% opacity
- **THEN** the logo appears on the preview and virtual camera output at the specified position and opacity

#### Scenario: Image overlay with transparency

- **WHEN** the user adds a PNG image with an alpha channel
- **THEN** the transparent areas of the image let the camera feed show through

### Requirement: Border overlay

The system SHALL allow the user to add configurable border overlays (frames) around the camera feed, with adjustable colour, thickness, and style.

#### Scenario: User adds a border

- **WHEN** the user enables a border overlay with 4px white solid style
- **THEN** a white border appears around the camera feed in the preview and virtual camera output

### Requirement: Watermark overlay

The system SHALL provide a dedicated watermark mode that tiles a text or image across the feed with configurable opacity and angle.

#### Scenario: User enables a watermark

- **WHEN** the user creates a watermark with text "DRAFT" at 20% opacity and 45-degree angle
- **THEN** the watermark text tiles across the entire feed at the specified opacity and angle

### Requirement: Overlay layering and management

The system SHALL allow multiple overlays to be active simultaneously, with a layer order that the user can reorder. Each overlay SHALL be independently toggleable.

#### Scenario: Multiple overlays active

- **WHEN** the user has a logo overlay, a text overlay, and a border all active
- **THEN** all three overlays are composited onto the feed in their layer order
- **AND** each can be individually toggled on/off without affecting the others

#### Scenario: User reorders overlay layers

- **WHEN** the user drags the text overlay above the logo overlay in the layer list
- **THEN** the text renders on top of the logo in the preview

### Requirement: Overlay pipeline position

All feed overlays SHALL be applied after the colour grading pipeline and before virtual camera output, so that overlays appear in the virtual camera feed.

#### Scenario: Virtual camera includes overlays

- **WHEN** the virtual camera is active and overlays are enabled
- **THEN** the virtual camera output includes all active overlays composited onto the feed

### Requirement: Overlay persistence

Overlay configurations SHALL be persisted per camera and restored when the camera reconnects.

#### Scenario: Overlays restored on reconnect

- **WHEN** a camera with configured overlays is reconnected
- **THEN** the previously configured overlays are restored with all their properties
