## ADDED Requirements

### Requirement: Canon sidebar thumbnails

The system SHALL generate sidebar thumbnails for Canon cameras by decoding native JPEG frames, downscaling to 160x120, and re-encoding to JPEG. The `get_thumbnail` command SHALL fall back to reading from `jpeg_buffer()` when `buffer()` returns `None`.

#### Scenario: Canon camera shows live thumbnail in sidebar

- **WHEN** a Canon camera has an active preview session delivering JPEG frames
- **THEN** the sidebar SHALL display a live 160x120 thumbnail updated at 5fps

#### Scenario: DirectShow camera thumbnails unchanged

- **WHEN** a DirectShow camera has an active preview session
- **THEN** the sidebar SHALL continue to use the existing raw RGB thumbnail path

### Requirement: Virtual camera toggle in sidebar

Each camera entry in the sidebar SHALL display a toggle to expose that camera's feed as a virtual webcam. The toggle SHALL be visible below the camera name. The toggle SHALL be disabled when no preview session is active for that camera.

#### Scenario: Toggle appears for each camera

- **WHEN** the camera sidebar renders a camera entry
- **THEN** a virtual camera toggle SHALL be visible below the camera name

#### Scenario: Toggle is disabled without active preview

- **WHEN** a camera has no active preview session
- **THEN** the virtual camera toggle SHALL be disabled with reduced opacity

#### Scenario: User enables virtual camera

- **WHEN** the user toggles the virtual camera on for a camera with an active preview
- **THEN** the system SHALL call `start_virtual_camera` with the device ID
- **AND** the toggle SHALL show the active (accent colour) state

#### Scenario: User disables virtual camera

- **WHEN** the user toggles the virtual camera off
- **THEN** the system SHALL call `stop_virtual_camera` with the device ID
- **AND** the toggle SHALL return to the inactive state

### Requirement: Virtual camera IPC commands

The backend SHALL expose `start_virtual_camera` and `stop_virtual_camera` Tauri commands. `start_virtual_camera` SHALL create a platform-specific virtual camera sink that writes frames from the preview session's JPEG buffer. `stop_virtual_camera` SHALL tear down the sink and release the virtual device.

#### Scenario: Start virtual camera for active preview

- **WHEN** `start_virtual_camera` is called with a valid device ID that has an active preview
- **THEN** the system SHALL create a virtual camera output sink and begin writing frames

#### Scenario: Start virtual camera without preview returns error

- **WHEN** `start_virtual_camera` is called for a device with no active preview
- **THEN** the command SHALL return an error string

#### Scenario: Stop virtual camera tears down sink

- **WHEN** `stop_virtual_camera` is called for a device with an active virtual camera
- **THEN** the system SHALL stop writing frames and release the virtual device

#### Scenario: Stop virtual camera is idempotent

- **WHEN** `stop_virtual_camera` is called for a device with no active virtual camera
- **THEN** the command SHALL succeed without error

### Requirement: Platform-specific virtual camera sinks

The system SHALL implement virtual camera output using platform-native APIs. On Windows, the system SHALL use DirectShow source filter or Media Foundation virtual camera API. On Linux, the system SHALL write frames to a v4l2loopback device. On macOS, virtual camera output is deferred (not implemented in this change).

#### Scenario: Windows virtual camera visible to other apps

- **WHEN** a virtual camera is started on Windows
- **THEN** the virtual camera SHALL appear as a video input device in Zoom, Teams, and other DirectShow/Media Foundation consumers

#### Scenario: Linux virtual camera visible to other apps

- **WHEN** a virtual camera is started on Linux with v4l2loopback loaded
- **THEN** the virtual camera SHALL appear as `/dev/videoN` readable by v4l2 consumers

#### Scenario: macOS returns not-supported error

- **WHEN** `start_virtual_camera` is called on macOS
- **THEN** the command SHALL return a "virtual camera not yet supported on macOS" error

### Requirement: Virtual camera frame delivery

The virtual camera sink SHALL read JPEG frames from the preview session's `JpegFrameBuffer`, decode them to raw RGB/NV12, and write to the virtual device at the frame rate produced by the capture session. The sink SHALL run on a dedicated thread to avoid blocking the preview pipeline.

#### Scenario: Frames flow from preview to virtual camera

- **WHEN** a virtual camera is active and the preview session produces frames
- **THEN** the virtual camera sink SHALL deliver decoded frames to the virtual device

#### Scenario: Virtual camera stops when preview stops

- **WHEN** a preview session is stopped while its virtual camera is active
- **THEN** the virtual camera sink SHALL automatically stop and release the device
