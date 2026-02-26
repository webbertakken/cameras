## ADDED Requirements

### Requirement: Camera device enumeration
The system SHALL detect all connected camera devices on application launch, including USB webcams, built-in laptop cameras, capture cards (Elgato, AVerMedia), tethered cameras (GoPro Hero 12, Canon EOS2000D), IP/network cameras via RTSP, and virtual camera devices. Detection SHALL use platform-native APIs: DirectShow/Media Foundation on Windows, AVFoundation on macOS, and V4L2 on Linux. RTSP camera discovery SHALL support manual URL entry and optional mDNS/ONVIF auto-discovery.

#### Scenario: Application launch with multiple cameras connected
- **WHEN** the application starts with three cameras connected (e.g. built-in webcam, USB Logitech Brio, GoPro Hero 12)
- **THEN** all three cameras appear in the left sidebar within 2 seconds of launch
- **AND** each camera displays its model name or a friendly identifier

#### Scenario: Application launch with no cameras connected
- **WHEN** the application starts with no camera devices connected
- **THEN** the left sidebar displays an empty state with a message indicating no cameras were found
- **AND** the system continues polling for new device connections

### Requirement: Hot-plug detection
The system SHALL detect camera devices being connected or disconnected in real time without requiring an application restart or manual refresh.

#### Scenario: Camera connected while app is running
- **WHEN** a user plugs in a USB webcam while the application is open
- **THEN** the new camera appears in the left sidebar within 3 seconds
- **AND** a toast notification confirms the camera was detected

#### Scenario: Camera disconnected while app is running
- **WHEN** a user unplugs a camera that is currently listed in the sidebar
- **THEN** the camera is removed from the sidebar within 3 seconds
- **AND** if the disconnected camera was the active selection, the app switches to the next available camera or shows the empty state

### Requirement: Camera capability enumeration
The system SHALL query each detected camera for its supported controls, resolutions, frame rates, and pixel formats, and expose this information to the UI layer.

#### Scenario: Camera with full UVC control support
- **WHEN** a Logitech Brio is detected
- **THEN** the system enumerates all supported UVC controls (brightness, contrast, saturation, white balance, exposure, gain, sharpness, zoom, focus, pan, tilt, etc.)
- **AND** each control includes its minimum, maximum, step, default value, and current value

#### Scenario: Camera with limited controls
- **WHEN** a basic built-in laptop webcam is detected that only supports brightness and contrast
- **THEN** the system reports only those two controls as supported
- **AND** unsupported controls are reported as unsupported (for the UI to render them greyed out with tooltips)

### Requirement: Left sidebar camera list
The system SHALL display all detected cameras in a left sidebar as the primary navigation surface. Each camera entry SHALL show a live preview thumbnail and the camera's model name.

#### Scenario: User views sidebar with cameras
- **WHEN** the application has detected two cameras
- **THEN** the left sidebar shows two entries, each with a live preview thumbnail updating in real time and the camera model name

#### Scenario: User selects a camera from the sidebar
- **WHEN** the user clicks on a camera entry in the sidebar
- **THEN** the main panel updates to show that camera's full preview and settings controls
- **AND** the selected camera entry is visually highlighted in the sidebar

### Requirement: RTSP network camera support
The system SHALL allow users to add IP/network cameras by entering an RTSP URL. RTSP cameras SHALL appear in the sidebar alongside local cameras and support the same preview and control workflows where the protocol allows.

#### Scenario: User adds an RTSP camera
- **WHEN** the user clicks "Add network camera" and enters an RTSP URL (e.g. rtsp://192.168.1.100:554/stream)
- **THEN** the system connects to the RTSP stream
- **AND** the camera appears in the left sidebar with a live thumbnail
- **AND** the camera entry shows "Network" or the provided name as its identifier

#### Scenario: RTSP camera goes offline
- **WHEN** an RTSP camera's network connection drops
- **THEN** the sidebar entry remains but shows an "Offline" indicator
- **AND** the system periodically attempts to reconnect

#### Scenario: RTSP camera with no UVC controls
- **WHEN** an RTSP camera is selected that has no controllable parameters
- **THEN** the settings panel shows only resolution/format info as read-only
- **AND** only software-side controls (colour grading, digital zoom) are available

### Requirement: Camera identification persistence
The system SHALL persistently identify cameras across sessions using a stable identifier (e.g. USB VID:PID + serial number, or device path fingerprint) so that per-camera settings and presets can be recalled when the same camera reconnects.

#### Scenario: Known camera reconnected
- **WHEN** a previously configured Logitech Brio is plugged back in
- **THEN** the system recognises it as the same device
- **AND** the camera's saved settings profile is available for auto-apply
