## ADDED Requirements

### Requirement: Live camera preview

The system SHALL render a live preview of the selected camera's feed in the main content area, updating in real time.

#### Scenario: Camera selected with active feed

- **WHEN** the user selects a camera from the sidebar
- **THEN** a live preview of that camera's feed is displayed in the main content area
- **AND** the preview updates at the camera's current frame rate

#### Scenario: Camera feed interrupted

- **WHEN** the camera feed is interrupted (e.g. camera enters a sleep state)
- **THEN** the preview displays the last received frame with an overlay indicating the feed is paused

### Requirement: Sidebar thumbnail previews

The system SHALL render small live preview thumbnails for each camera listed in the left sidebar.

#### Scenario: Multiple cameras with sidebar thumbnails

- **WHEN** three cameras are connected
- **THEN** the sidebar shows three entries, each with a live updating thumbnail
- **AND** thumbnails render at a reduced frame rate (e.g. 5-10 fps) to conserve resources

### Requirement: Split before/after comparison

The system SHALL provide a split-view comparison mode showing the camera feed before and after settings changes, side by side.

#### Scenario: User enables before/after comparison

- **WHEN** the user activates the split comparison mode
- **THEN** the preview area splits into two panes: "Before" (snapshot of feed at time of activation) and "After" (live feed with current settings)

#### Scenario: User adjusts settings in comparison mode

- **WHEN** the user adjusts a control while in comparison mode
- **THEN** the "After" pane updates in real time to reflect the change
- **AND** the "Before" pane remains unchanged as a reference

#### Scenario: User exits comparison mode

- **WHEN** the user deactivates comparison mode
- **THEN** the preview returns to a single full-size live view with current settings applied

### Requirement: Full diagnostic overlay

The system SHALL provide a toggleable diagnostic overlay on the preview showing real-time technical information about the camera feed.

#### Scenario: User enables diagnostics

- **WHEN** the user toggles the diagnostics overlay on
- **THEN** the preview shows an overlay with: current resolution, frame rate (actual vs target), pixel format, bandwidth usage, dropped frame count, frame latency (capture to display), and USB bus info (where available)
- **AND** the overlay updates in real time

#### Scenario: User disables diagnostics

- **WHEN** the user toggles the diagnostics overlay off
- **THEN** the overlay is hidden and the preview shows the feed without diagnostic info

#### Scenario: RTSP camera diagnostics

- **WHEN** diagnostics are enabled for an RTSP network camera
- **THEN** the overlay shows resolution, frame rate, format, network latency, and dropped frames
- **AND** USB bus info is omitted (not applicable)

### Requirement: Preview rendering performance

The system SHALL render the main preview at the camera's native frame rate without dropping more than 5% of frames under normal system load.

#### Scenario: 1080p 30fps preview

- **WHEN** the camera is outputting 1920x1080 at 30 fps
- **THEN** the preview renders at least 28.5 fps (95% of 30)
- **AND** CPU usage for the preview remains below 15% on a modern quad-core system
