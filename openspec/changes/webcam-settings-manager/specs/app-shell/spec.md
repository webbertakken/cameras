## ADDED Requirements

### Requirement: System tray presence

The system SHALL run as a system tray application, with the ability to minimise to the tray and restore from the tray icon.

#### Scenario: User minimises to tray

- **WHEN** the user closes or minimises the main window
- **THEN** the application continues running in the system tray
- **AND** a tray icon is visible with a context menu

#### Scenario: User restores from tray

- **WHEN** the user clicks the system tray icon
- **THEN** the main application window is restored and brought to the foreground

#### Scenario: Tray context menu

- **WHEN** the user right-clicks the system tray icon
- **THEN** a menu shows: active camera name, quick preset switching, open full panel, and quit

### Requirement: Floating widget

The system SHALL provide a floating always-on-top widget showing the most-used controls for the active camera, with the ability to expand to the full settings panel.

#### Scenario: User opens floating widget

- **WHEN** the user activates the floating widget (via tray menu or hotkey)
- **THEN** a small always-on-top window appears with the most critical controls (exposure, white balance mode, active preset)

#### Scenario: User expands widget to full panel

- **WHEN** the user clicks the expand button on the floating widget
- **THEN** the full settings panel opens
- **AND** the floating widget closes

### Requirement: Auto-start option

The system SHALL offer a configurable option to start automatically on system login, minimised to the system tray, with settings auto-applied.

#### Scenario: Auto-start enabled

- **WHEN** the user enables auto-start in settings
- **THEN** on next system login, the app starts minimised to the tray
- **AND** saved settings are applied to any detected cameras

#### Scenario: Auto-start disabled

- **WHEN** auto-start is disabled
- **THEN** the app does not start on login

### Requirement: OS theme following

The system SHALL follow the operating system's light/dark theme setting and update its appearance when the OS theme changes.

#### Scenario: OS switches from light to dark mode

- **WHEN** the user changes their OS from light mode to dark mode while the app is running
- **THEN** the app's UI switches to dark mode without requiring a restart

#### Scenario: App respects OS theme on launch

- **WHEN** the app launches and the OS is in dark mode
- **THEN** the app renders in dark mode from the first frame

### Requirement: Auto-updater

The system SHALL check for updates on launch and allow the user to download and install updates from within the application, using Tauri's built-in updater.

#### Scenario: Update available

- **WHEN** the app detects a new version is available
- **THEN** a non-intrusive notification informs the user
- **AND** the user can choose to download and install or dismiss

#### Scenario: No update available

- **WHEN** the app checks for updates and the current version is the latest
- **THEN** no notification is shown

### Requirement: Cross-platform support

The system SHALL run on Windows (10+), macOS (12+), and Linux (distributions with glibc 2.31+, e.g. Ubuntu 20.04+), with platform-appropriate UI behaviour and camera API usage.

#### Scenario: App runs on Windows 11

- **WHEN** the app is launched on Windows 11
- **THEN** camera discovery uses DirectShow/Media Foundation APIs
- **AND** the system tray integrates with the Windows notification area

#### Scenario: App runs on macOS

- **WHEN** the app is launched on macOS
- **THEN** camera discovery uses AVFoundation APIs
- **AND** the app integrates with the macOS menu bar

#### Scenario: App runs on Linux

- **WHEN** the app is launched on a supported Linux distribution
- **THEN** camera discovery uses V4L2 APIs
- **AND** the system tray integrates with the desktop environment's tray
