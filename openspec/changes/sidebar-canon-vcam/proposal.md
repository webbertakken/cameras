## Why

Canon cameras show a blank placeholder icon in the sidebar because `get_thumbnail` only reads raw RGB frames — Canon live view delivers native JPEG with no raw buffer. Additionally, there is no way to expose a camera feed as a virtual webcam for use in other applications (Zoom, Teams, OBS, etc.).

## What Changes

- **Canon sidebar thumbnails**: `get_thumbnail` gains a JPEG-based fallback path — decode Canon's native JPEG, downscale, re-encode — so Canon cameras show live thumbnails like DirectShow cameras
- **Virtual camera toggle**: Each camera entry in the sidebar gets a toggle to expose that camera's feed as a virtual webcam visible to other applications
- **Virtual camera backend**: New IPC commands and platform-specific output sinks that write frames to a virtual camera device

## Capabilities

### New Capabilities

- `virtual-camera`: Per-camera virtual webcam output — toggle on/off, platform-specific sink (Windows DirectShow/MF, Linux v4l2loopback, macOS deferred), IPC commands, sidebar UI toggle

### Modified Capabilities

- None (no existing specs to modify)

## Impact

- **Rust backend**: `src-tauri/src/preview/commands.rs` — fix `get_thumbnail` for Canon; add `start_virtual_camera` / `stop_virtual_camera` commands
- **Rust backend**: `src-tauri/src/preview/compress.rs` — add `compress_thumbnail_from_jpeg` function
- **Rust backend**: New `src-tauri/src/integration/` module for virtual camera output sinks
- **Frontend**: `src/features/camera-sidebar/CameraEntry.tsx` — add virtual camera toggle
- **Frontend**: New IPC bindings for virtual camera commands
- **Dependencies**: Possible new crate for virtual camera output or FFI bindings
- **CI**: No changes expected — virtual camera is runtime-only
