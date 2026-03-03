# Canon live view preview wiring

## Problem

Canon cameras deliver JPEG frames via EDSDK live view (`EdsDownloadEvfImage`), but
the preview command layer always creates a DirectShow `CaptureSession`. This means:

- Canon cameras fail to preview (DirectShow can't open `edsdk://` device paths)
- No EDSDK session is opened before reading camera properties
- Chainlink #71, #72

## Solution

Wire the existing `CanonCaptureSession` / `PreviewSession` enum (already in
`capture.rs`) into the command layer so Canon devices route through EDSDK live
view instead of DirectShow.

## Changes

### 1. Fix Tauri state management (`lib.rs`)

`create_camera_state()` returns `(CameraState, CanonSdkState)` but line 178 does
`.manage(create_camera_state())` which manages a **tuple** — not two separate
states. Fix:

```rust
let (camera_state, canon_sdk_state) = create_camera_state();
app.manage(camera_state);
app.manage(canon_sdk_state);
```

### 2. Update PreviewState sessions map (`commands.rs`)

Change `HashMap<String, CaptureSession>` to `HashMap<String, PreviewSession>`.

### 3. Route Canon devices in start_preview (`commands.rs`)

Detect `edsdk://` prefix in device_path. If present, create `CanonCaptureSession`
via `CanonSdkState`. Otherwise, create `CaptureSession` (DirectShow).

The `CanonSdkState` must be injected as Tauri managed state into
`start_preview`, `start_all_previews`, and `start_preview_for_device`.

### 4. Update get_frame (`commands.rs`)

`PreviewSession` already has `jpeg_buffer()` returning `Option<&Arc<JpegFrameBuffer>>`.
The existing get_frame logic should work with minor adjustments — the fallback
raw-buffer path uses `session.buffer()` which returns `Option<&Arc<FrameBuffer>>`
on `PreviewSession` (None for Canon). The fallback path should return an error
for Canon sessions that have no JPEG yet rather than panic.

### 5. Update get_thumbnail (`commands.rs`)

Canon sessions have no raw RGB buffer. Options:

- Return an error ("thumbnails not available for Canon live view")
- Decode the JPEG, resize, re-encode (expensive, skip for now)

Use option A for now — return a clear error.

### 6. Update diagnostics/encoding_stats (`commands.rs`)

`PreviewSession` already implements `diagnostics()` (returns default for Canon)
and `encoding_snapshot()` (returns None for Canon). These should just work.

### 7. Update startup auto-start (`lib.rs`)

The setup block that auto-starts previews needs the same Canon detection logic.
Inject `CanonSdkState` and route accordingly.

### 8. Update hotplug handler (`commands.rs`)

`start_preview_for_device` needs access to `CanonSdkState` via `app.try_state()`.

### 9. Canon camera handle resolution

`CanonCaptureSession::new` needs a `CameraHandle`. The device_path for Canon is
`edsdk://Canon EOS 2000D` — we need to map this back to the camera handle index.
Check how `CanonBackend` stores handles and provide a lookup mechanism.

## Testing

- Unit tests for PreviewSession routing (edsdk:// vs DirectShow paths)
- Unit tests for get_frame with Canon session (JPEG passthrough)
- Unit tests for get_thumbnail with Canon session (returns error)
- Compilation checks with and without `--features canon`

## Affected files

- `src-tauri/src/preview/commands.rs` (main changes)
- `src-tauri/src/lib.rs` (state management + startup)
- `src-tauri/src/preview/capture.rs` (may need CameraHandle lookup)
- `src-tauri/src/camera/canon/backend.rs` (camera handle lookup)
