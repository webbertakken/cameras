# Diagnostic overlay spec

## MODIFIED requirements

### Requirement: Combined diagnostics polling

The `useDiagnostics` hook SHALL fetch both `get_diagnostics` and
`get_encoding_stats` for the selected device. The polling interval SHALL be
1000ms. Both calls SHALL happen in parallel within the same interval tick.

#### Scenario: Both stats available (DirectShow camera)

- **WHEN** the overlay is enabled for a device with an active encode worker
- **THEN** the hook SHALL return both capture stats and encoding stats

#### Scenario: Encoding stats unavailable (Canon camera)

- **WHEN** `get_encoding_stats` returns an error for a device
- **THEN** the hook SHALL return capture stats with encoding fields set to
  `null`
- **AND** the overlay SHALL hide the encoding section

#### Scenario: Polling stops when disabled

- **WHEN** the overlay is toggled off
- **THEN** both polling calls SHALL stop immediately

### Requirement: Encoding stats in overlay

The `DiagnosticOverlay` SHALL display encoding statistics below the existing
capture statistics, visually separated.

#### Scenario: Encoding stats displayed

- **WHEN** encoding stats are available
- **THEN** the overlay SHALL show: encoder kind, average encode time (ms),
  last encode time (ms), frames encoded, frames dropped (encode)

#### Scenario: Encoder kind colour coding

- **WHEN** encoder kind is `mfHardware` — the label SHALL use a green colour
- **WHEN** encoder kind is `mfSoftware` — the label SHALL use an amber colour
- **WHEN** encoder kind is `cpuFallback` — the label SHALL use a red colour

#### Scenario: Encoding section hidden when unavailable

- **WHEN** encoding stats are `null`
- **THEN** the overlay SHALL not render the encoding section

## ADDED requirements

### Requirement: Overlay wired into preview

The diagnostic overlay SHALL render inside the preview area of `App.tsx`,
positioned over the camera preview. It SHALL only be active when a camera is
selected.

#### Scenario: Overlay visible when toggled on

- **WHEN** the user presses Ctrl+D or clicks the "Stats" button
- **THEN** the overlay SHALL appear over the preview canvas
- **AND** polling SHALL begin for the selected camera

#### Scenario: Overlay hidden by default

- **WHEN** the app starts
- **THEN** the overlay SHALL not be visible
- **AND** no diagnostic polling SHALL occur

#### Scenario: Camera switch resets stats

- **WHEN** the user selects a different camera while the overlay is visible
- **THEN** the overlay SHALL clear the previous stats and begin polling the
  new camera

### Requirement: Keyboard shortcut toggle

The overlay SHALL be toggleable via `Ctrl+D` keyboard shortcut.

#### Scenario: Ctrl+D toggles overlay

- **WHEN** the user presses Ctrl+D
- **THEN** the overlay visibility SHALL toggle
- **AND** the "Stats" button aria-pressed state SHALL update accordingly
