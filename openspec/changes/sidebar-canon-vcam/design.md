## Context

The app has a dual-pipeline preview system: DirectShow cameras produce raw RGB frames (→ encode worker → JPEG), while Canon cameras deliver native JPEG directly. The sidebar calls `get_thumbnail` which only reads from `buffer()` (raw RGB) — Canon sessions return `None` for `buffer()`, so their thumbnails are broken.

For virtual camera output, the app already has per-device `PreviewSession` entries in a `HashMap<String, PreviewSession>` and each session produces frames via `JpegFrameBuffer`. The virtual camera sink needs to tap into this existing buffer and feed frames to a platform-native virtual device.

## Goals / Non-Goals

**Goals:**

- Canon cameras show live thumbnails in the sidebar
- Per-camera virtual camera toggle in the sidebar UI
- Virtual camera output on Windows (DirectShow) and Linux (v4l2loopback)
- Clean separation between platform sinks and shared orchestration

**Non-Goals:**

- macOS virtual camera (deferred — no simple user-mode API)
- Virtual camera settings (resolution, frame rate selection) — uses whatever the preview produces
- Multiple simultaneous virtual cameras per physical camera
- Audio passthrough

## Decisions

### 1. Canon thumbnails: decode JPEG → resize → re-encode

Add `compress_thumbnail_from_jpeg()` to `compress.rs` that uses the `image` crate (already a dependency) to decode JPEG bytes, resize with `fast_image_resize`, and re-encode at quality 70. In `get_thumbnail`, fall back to `jpeg_buffer()` when `buffer()` is `None`.

**Why not use the full-size JPEG directly?** The 960x640 Canon JPEG is ~384KB — too large for sidebar thumbnails at 5fps polling. Downscaling to 160x120 keeps it under 10KB per frame.

### 2. Virtual camera module at `src-tauri/src/virtual_camera/`

New domain module (not under `preview/` or `integration/`) with:

- `mod.rs` — `VirtualCameraState` (Tauri managed state), `VirtualCameraSink` trait
- `commands.rs` — `start_virtual_camera`, `stop_virtual_camera` IPC commands
- `windows.rs` — DirectShow source filter via `windows` crate FFI
- `linux.rs` — v4l2loopback `/dev/videoN` writer
- `stub.rs` — "not supported" fallback for macOS and other platforms

**Why a separate domain module?** Virtual camera is a distinct concern from preview capture — it's an output sink, not an input source. Keeping it separate follows the existing domain-driven structure (`camera/`, `preview/`, `settings/`, `diagnostics/`).

### 3. Windows: DirectShow source filter (not MF virtual camera)

Use a DirectShow source filter registered as a COM object. This approach works on Windows 10 and 11, whereas `MFCreateVirtualCamera` is Windows 11+ only.

The filter implements `IBaseFilter` with a single output pin delivering NV12 or RGB24 frames. Registration uses `regsvr32`-free COM (SxS manifest) to avoid requiring admin elevation.

**Alternative considered:** Media Foundation virtual camera — simpler API but Windows 11+ only, limiting compatibility.

### 4. Linux: direct v4l2loopback write

Open `/dev/videoN` (the loopback device), set format via `VIDIOC_S_FMT`, and write raw frames with `write()`. Requires `v4l2loopback` kernel module loaded by the user.

**Why not bundle v4l2loopback?** It's a kernel module requiring DKMS — can't be shipped in-app. The user must install it (`sudo modprobe v4l2loopback`). The app detects available loopback devices and reports clear errors if none exist.

### 5. Sink thread reads from JpegFrameBuffer

The sink spawns a dedicated thread that polls `JpegFrameBuffer::latest()`, decodes JPEG to RGB using the `image` crate, and writes to the virtual device. Polling interval matches the source frame rate (~33ms for 30fps DirectShow, ~200ms for Canon live view).

**Why poll instead of a channel?** The `JpegFrameBuffer` is a single-slot latest-frame buffer shared with the preview UI. Adding a broadcast channel would require changing the existing architecture. Polling the existing buffer is simpler and sufficient — dropping frames is acceptable for virtual camera output.

### 6. Frontend: icon button with tooltip (not toggle switch)

Use a small icon button (webcam icon) below the camera name, with a tooltip showing "Expose as virtual camera" / "Stop virtual camera". Active state uses `--colour-accent`. This is more compact than a full toggle switch and fits the 200px sidebar width.

State tracked in a new Zustand slice `useVirtualCameraStore` with `activeDevices: Set<string>`.

## Risks / Trade-offs

- **[DirectShow COM registration complexity]** → Mitigate with SxS manifest for registration-free COM. If that proves unreliable, fall back to a one-time `regsvr32` during install via NSIS custom action.
- **[v4l2loopback not installed]** → Detect loopback devices at startup, show clear error toast when user tries to enable without v4l2loopback loaded.
- **[JPEG decode overhead in sink thread]** → At 30fps 1080p, JPEG decode takes ~5-10ms per frame. Acceptable on a dedicated thread. For Canon (5fps, 960x640) it's negligible.
- **[Virtual camera persists after app crash]** → On Windows, the COM filter is in-process so it dies with the app. On Linux, the loopback device stays but stops receiving frames — consumers get stale/black frames, which is standard behaviour.
- **[Sidebar space for the toggle]** → Icon button is 24px, fits below the camera name without increasing entry height significantly.
