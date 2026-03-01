## MODIFIED Requirements

### Requirement: System tray presence

The system SHALL run as a system tray application, with the ability to minimise to the tray and restore from the tray icon.

#### Scenario: User minimises to tray

- **WHEN** the user closes or minimises the main window
- **THEN** the application continues running in the system tray
- **AND** a tray icon is visible with a context menu

#### Scenario: User left-clicks tray icon

- **WHEN** the user left-clicks the system tray icon
- **THEN** the main window visibility is toggled (shown if hidden, hidden if visible)
- **AND** if shown, the window is unminimised and brought to the foreground
- **AND** no context menu is displayed

#### Scenario: Tray context menu

- **WHEN** the user right-clicks the system tray icon
- **THEN** a context menu is displayed with exactly three items: "Show/Hide", "App Settings", "Exit"

#### Scenario: Show/Hide menu item

- **WHEN** the user clicks "Show/Hide" in the context menu
- **THEN** the main window visibility is toggled (shown if hidden, hidden if visible)

#### Scenario: App Settings menu item

- **WHEN** the user clicks "App Settings" in the context menu
- **THEN** the app settings window is opened (or focused if already open)

#### Scenario: Exit menu item

- **WHEN** the user clicks "Exit" in the context menu
- **THEN** the application exits completely
