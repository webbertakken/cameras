## ADDED Requirements

### Requirement: PTP/MTP camera discovery and live view capture

The system SHALL detect cameras connected via USB that expose as PTP/MTP devices (e.g. Canon EOS, Nikon, Sony Alpha DSLRs/mirrorless) and support live view frame capture without requiring vendor-specific desktop utilities (e.g. Canon EOS Webcam Utility, Nikon Webcam Utility).

**Context**: Many DSLRs and mirrorless cameras connect via USB using the PTP (Picture Transfer Protocol) / MTP (Media Transfer Protocol) standard. These cameras do NOT appear as DirectShow/UVC video devices — they appear as PTP/MTP devices instead. Vendor utilities (e.g. Canon's "EOS Webcam Utility") create a virtual DirectShow device to bridge this gap, but these utilities are increasingly paywalled (Canon's is now pro-only). Native PTP support removes the dependency on vendor utilities entirely.

**Technical approach**: Use `libgphoto2` (the standard cross-platform PTP library) via Rust FFI bindings (`gphoto2` crate or raw FFI) to:

1. Discover PTP/MTP devices on USB
2. Initiate live view (camera-specific PTP operation)
3. Capture live view frames as JPEG
4. Expose frames through the existing `FrameBuffer` pipeline

#### Scenario: Canon DSLR connected via USB without vendor utility

- **WHEN** a Canon EOS 2000D is connected via USB with no Canon software installed
- **THEN** the camera appears in the sidebar as a PTP camera within 5 seconds
- **AND** the camera entry indicates it is a "PTP/Tethered" device (distinct from UVC webcams)
- **AND** live view preview is available when the camera is selected

#### Scenario: PTP camera with limited controls

- **WHEN** a PTP camera is selected
- **THEN** the settings panel shows PTP-accessible controls only (ISO, shutter speed, aperture, white balance — where the camera supports PTP property access)
- **AND** UVC-style controls (brightness, contrast, saturation) are NOT shown (these are not applicable to PTP cameras)
- **AND** software-side controls (colour grading, digital zoom, overlays) are available as usual

#### Scenario: PTP camera disconnected

- **WHEN** a PTP camera is unplugged while active
- **THEN** the camera is removed from the sidebar within 3 seconds
- **AND** if it was the active selection, the app switches to the next available camera or shows empty state

### Requirement: PTP device discovery

The system SHALL enumerate USB-connected PTP/MTP devices separately from DirectShow/UVC devices. PTP discovery SHALL run alongside DirectShow enumeration so that both UVC webcams and PTP cameras appear in the sidebar simultaneously.

#### Scenario: Mixed device types

- **WHEN** the system has a USB webcam (UVC) and a Canon DSLR (PTP) connected
- **THEN** both cameras appear in the sidebar
- **AND** the UVC webcam shows standard UVC controls
- **AND** the Canon DSLR shows PTP-specific controls

### Requirement: PTP live view frame capture

The system SHALL capture live view frames from PTP cameras and deliver them through the existing preview pipeline (`FrameBuffer` → JPEG compression → IPC delivery).

PTP live view delivers JPEG frames directly (unlike DirectShow which delivers raw RGB/YUY2). The capture pipeline SHALL accept these JPEG frames and pass them through without re-encoding where possible.

#### Scenario: Live view at camera-native resolution

- **WHEN** live view is started on a Canon EOS 2000D
- **THEN** frames are delivered at the camera's live view resolution (typically 960x640 or 1024x680 for Canon)
- **AND** the preview updates at the camera's live view frame rate (typically 15-30fps depending on model)

### Requirement: PTP camera controls via PTP properties

The system SHALL read and write camera settings via PTP device properties where the camera supports them. Common PTP-controllable properties include:

- ISO sensitivity
- Shutter speed
- Aperture (f-stop)
- White balance mode
- Image quality/format
- Exposure compensation

These SHALL be exposed as `ControlDescriptor` values compatible with the existing dynamic control rendering system, using appropriate UI widgets (select dropdowns for enumerated values like ISO/shutter speed, toggles for modes).

#### Scenario: Adjusting ISO on a PTP camera

- **WHEN** the user changes the ISO dropdown on a connected Canon EOS
- **THEN** the camera's ISO setting changes in real time
- **AND** the live view preview reflects the new exposure

### Requirement: Stable PTP device identification

The system SHALL generate stable device identifiers for PTP cameras using the camera's serial number (available via PTP DeviceInfo) combined with vendor/product IDs, so that per-camera settings persist across reconnections.

#### Scenario: Known PTP camera reconnected

- **WHEN** a previously configured Canon EOS 2000D is reconnected
- **THEN** the system recognises it as the same device
- **AND** saved PTP-specific settings are available for recall

## Technical notes

### libgphoto2

- Standard cross-platform library for PTP/MTP camera control
- Supports Canon, Nikon, Sony, Fujifilm, Panasonic, Olympus, and many others
- Rust bindings available via `gphoto2` crate (safe wrapper) or raw `libgphoto2-sys` FFI
- Provides: device discovery, live view capture, property read/write, file transfer
- Works on Windows (via libusb/WinUSB), macOS, and Linux
- On Windows, camera must NOT be claimed by Windows' built-in MTP driver — may require WinUSB driver installation (e.g. via Zadig)

### Architecture integration

- New `PtpBackend` struct implementing a subset of `CameraBackend` (or a new `PtpCameraBackend` trait)
- Runs alongside `WindowsBackend` / `MacosBackend` / `LinuxBackend` — both backends contribute to the unified camera list
- PTP live view frames are JPEG — can skip the RGB→JPEG compression step in the pipeline
- PTP cameras use a fundamentally different control model (enumerated property values rather than continuous UVC ranges) — the `ControlDescriptor` system already supports `select` type which maps well

### Known limitations

- PTP live view resolution and frame rate are camera-dependent and generally lower than UVC (960x640 @ 15-30fps typical for Canon)
- Not all cameras support PTP live view (entry-level compacts may not)
- Windows WinUSB driver requirement adds friction for end users (needs Zadig or similar)
- PTP is inherently single-client — only one application can access the camera at a time
- Canon's proprietary EDSDK offers more features than PTP but is closed-source and requires an SDK licence

### Supported camera families (via libgphoto2)

- Canon EOS (DSLR and mirrorless via Canon PTP extensions)
- Nikon D-series and Z-series (via Nikon PTP extensions)
- Sony Alpha (limited PTP support, better via Sony Remote SDK)
- Fujifilm X-series (via Fuji PTP extensions)
- Panasonic Lumix (limited)
- Olympus/OM System (limited)

### Phase placement

This feature spans camera discovery (Phase 1 extension), controls (Phase 3 extension), and cross-platform (Phase 6). Recommended implementation order:

1. PTP discovery + live view on Windows (Phase 1 extension or new Phase 1b)
2. PTP controls (alongside Phase 3 advanced controls)
3. Cross-platform PTP (alongside Phase 6)
