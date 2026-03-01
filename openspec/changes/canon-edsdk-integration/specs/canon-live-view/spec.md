## ADDED requirements

### Requirement: Canon live view frame capture

The system SHALL start live view on a Canon camera via EDSDK and poll JPEG frames into the existing `FrameBuffer`.

**Context**: Canon live view is enabled via `EdsSendCommand(camera, kEdsCameraCommand_EvfMode, 1)`. Frames are pulled via `EdsCreateEvfImageRef()` + `EdsDownloadEvfImage()`, which returns JPEG data directly.

#### Scenario: Live view start

- **WHEN** live view is started for a Canon camera
- **THEN** a dedicated polling thread is spawned
- **AND** JPEG frames arrive in the `FrameBuffer` within 1 second
- **AND** the preview pipeline renders them

#### Scenario: Live view frame rate

- **WHEN** live view is active
- **THEN** frames are polled at ~200ms intervals (~5fps)
- **AND** the interval is configurable per backend

#### Scenario: Live view stop

- **WHEN** live view is stopped (camera deselected or disconnected)
- **THEN** the polling thread is signalled to stop
- **AND** `EdsSendCommand(camera, kEdsCameraCommand_EvfMode, 0)` is called
- **AND** the thread joins cleanly within 1 second

#### Scenario: Live view with camera busy

- **WHEN** `EdsDownloadEvfImage()` returns `EDS_ERR_OBJECT_NOTREADY`
- **THEN** the polling thread retries on the next interval
- **AND** no error is surfaced to the user (this is normal during autofocus)

### Requirement: JPEG passthrough

Canon live view frames are JPEG. The system SHALL pass them directly to the preview pipeline without re-encoding.

#### Scenario: No double JPEG compression

- **WHEN** a Canon live view JPEG frame enters the preview pipeline
- **THEN** it is delivered to the frontend as-is (no RGB decode + JPEG re-encode)

## Technical notes

- Live view in `src-tauri/src/camera/canon/live_view.rs`
- The `Frame` struct in `capture.rs` currently holds raw RGB. For JPEG passthrough, either:
  - Add a `jpeg_data: Option<Vec<u8>>` field to `Frame`, or
  - Create a parallel `JpegFrame` path
- Canon live view resolution is model-dependent: typically 960x640 (crop), 1056x704 (full), or 1920x1280 (newer bodies)
- The polling thread uses `Arc<AtomicBool>` for stop signalling (same pattern as `CaptureSession`)
