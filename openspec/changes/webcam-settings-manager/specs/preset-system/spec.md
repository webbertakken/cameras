## ADDED Requirements

### Requirement: Save named presets

The system SHALL allow the user to save the current camera settings as a named preset.

#### Scenario: User saves a preset

- **WHEN** the user clicks "Save preset" and enters the name "Warm Studio"
- **THEN** all current camera control values are stored under the name "Warm Studio"
- **AND** the preset appears in the preset list for the current camera

### Requirement: Load presets

The system SHALL allow the user to load a previously saved preset, applying all stored control values to the camera.

#### Scenario: User loads a preset

- **WHEN** the user selects "Warm Studio" from the preset list
- **THEN** all camera controls are set to the values stored in that preset
- **AND** the preview updates to reflect the loaded settings

#### Scenario: Preset has controls not supported by current camera

- **WHEN** the user loads a preset that includes a zoom value on a camera without zoom
- **THEN** the unsupported control values are skipped
- **AND** all supported control values are applied
- **AND** a toast notification lists which controls were skipped

### Requirement: Built-in starter presets

The system SHALL include a set of built-in starter presets (e.g. "Warm", "Cool", "High Contrast", "Natural", "Low Light") that work with common controls.

#### Scenario: User views starter presets on a new camera

- **WHEN** the user opens the preset list for a newly connected camera with no saved presets
- **THEN** the built-in starter presets are available
- **AND** they are visually distinct from user-created presets

### Requirement: Per-camera preset libraries

Each camera SHALL have its own preset library. Presets saved for one camera are scoped to that camera's identifier.

#### Scenario: Two cameras with different presets

- **WHEN** the user saves "Warm Studio" on Camera A and "Cool Meeting" on Camera B
- **THEN** selecting Camera A shows "Warm Studio" in its preset list (not "Cool Meeting")
- **AND** selecting Camera B shows "Cool Meeting" in its preset list (not "Warm Studio")

### Requirement: Import/export presets

The system SHALL allow the user to export presets as files and import presets from files, enabling sharing across machines or users.

#### Scenario: User exports a preset

- **WHEN** the user clicks "Export" on the "Warm Studio" preset
- **THEN** a JSON file is saved to disk containing the preset name and all control values

#### Scenario: User imports a preset

- **WHEN** the user imports a preset file
- **THEN** the preset is added to the current camera's preset library
- **AND** if a preset with the same name already exists, the user is prompted to rename or overwrite

### Requirement: Delete and rename presets

The system SHALL allow the user to delete and rename user-created presets. Built-in presets MUST NOT be deletable but MAY be hidden.

#### Scenario: User deletes a preset

- **WHEN** the user deletes "Warm Studio"
- **THEN** the preset is removed from the camera's library
- **AND** a confirmation dialog is shown before deletion

#### Scenario: User renames a preset

- **WHEN** the user renames "Warm Studio" to "Studio Daylight"
- **THEN** the preset's display name is updated
- **AND** the stored settings are unchanged
