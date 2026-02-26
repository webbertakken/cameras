## ADDED Requirements

### Requirement: Global keyboard hotkeys
The system SHALL support global keyboard hotkeys that work even when the application is not focused, allowing quick toggling of camera settings.

#### Scenario: User toggles auto-exposure via hotkey
- **WHEN** the user presses the configured hotkey (e.g. Ctrl+Shift+E) while in any application
- **THEN** the auto-exposure mode toggles between auto and manual on the active camera
- **AND** a toast notification confirms the change

#### Scenario: User switches preset via hotkey
- **WHEN** the user presses a preset hotkey (e.g. Ctrl+Shift+1)
- **THEN** the corresponding preset is loaded on the active camera

### Requirement: Customisable hotkey bindings
The system SHALL allow the user to assign and reassign global hotkeys to any supported action (toggle auto-exposure, toggle auto-white balance, switch preset, toggle virtual camera, mute/unmute camera).

#### Scenario: User assigns a new hotkey
- **WHEN** the user opens hotkey settings and records a new key combination for "Toggle auto-exposure"
- **THEN** the hotkey is saved and immediately active

#### Scenario: Hotkey conflict
- **WHEN** the user assigns a hotkey that conflicts with an existing binding
- **THEN** the system warns the user and asks them to choose a different binding or overwrite

### Requirement: Stream Deck integration
The system SHALL provide a Stream Deck plugin that exposes camera controls as assignable Stream Deck actions.

#### Scenario: User assigns a preset to a Stream Deck button
- **WHEN** the user configures a Stream Deck button with the "Load Preset" action and selects "Warm Studio"
- **THEN** pressing that Stream Deck button loads the "Warm Studio" preset on the active camera

#### Scenario: Stream Deck shows camera status
- **WHEN** a Stream Deck button is assigned to toggle auto-exposure
- **THEN** the button icon reflects whether auto-exposure is currently on or off

### Requirement: MIDI controller support
The system SHALL support MIDI controller input, mapping MIDI CC (continuous controller) messages to camera control sliders and MIDI note messages to toggle actions.

#### Scenario: User maps a MIDI knob to brightness
- **WHEN** the user assigns MIDI CC channel 1 to the brightness control
- **THEN** turning the MIDI knob adjusts the camera brightness in real time

#### Scenario: User maps a MIDI button to preset
- **WHEN** the user assigns MIDI note 60 to load the "Cool Meeting" preset
- **THEN** pressing that MIDI button loads the preset

### Requirement: Controller mapping persistence
All hotkey, Stream Deck, and MIDI mappings SHALL be persisted and restored across application sessions.

#### Scenario: Mappings survive restart
- **WHEN** the user restarts the application
- **THEN** all hotkey, Stream Deck, and MIDI mappings are restored exactly as configured
