## ADDED Requirements

### Requirement: App settings window

The system SHALL provide a separate native window for application-level settings, accessible from the tray context menu.

#### Scenario: User opens app settings from tray

- **WHEN** the user clicks "App Settings" in the tray context menu
- **THEN** a new native window opens with the settings page
- **AND** the window is centred on screen with dimensions approximately 500x400

#### Scenario: Settings window already open

- **WHEN** the user clicks "App Settings" and the settings window is already open
- **THEN** the existing settings window is brought to the foreground and focused
- **AND** no duplicate window is created

#### Scenario: Settings window closed independently

- **WHEN** the user closes the settings window via the OS close button
- **THEN** the settings window is destroyed
- **AND** the main application continues running (not affected by settings window lifecycle)

#### Scenario: Settings page renders placeholder content

- **WHEN** the settings window opens
- **THEN** it SHALL render a placeholder settings page
- **AND** the page follows the application's current theme (light/dark)
