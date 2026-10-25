# MFCreateVirtualCamera — Windows 11+ virtual camera spec

## Summary

This spec documents how to implement the Windows virtual camera sink using the
**Media Foundation Virtual Camera API** (`MFCreateVirtualCamera`) introduced in
Windows 11 (Build 22000+). Since the project already requires Windows 11+, MF
virtual camera is the preferred approach over the legacy DirectShow source filter
— it is simpler, does not require COM registration, and integrates natively with
the Windows Camera FrameServer pipeline.

## 1. API overview

### MFCreateVirtualCamera function

```c
HRESULT MFCreateVirtualCamera(
    MFVirtualCameraType     type,        // MFVirtualCameraType_SoftwareCameraSource
    MFVirtualCameraLifetime lifetime,    // Session or System
    MFVirtualCameraAccess   access,      // CurrentUser or AllUsers
    LPCWSTR                 friendlyName,
    LPCWSTR                 sourceClassId, // CLSID of the custom media source
    LPCGUID                 categoryGuid,  // KSCATEGORY_VIDEO_CAMERA
    UINT32                  streamCount,
    IMFVirtualCamera**      ppCamera
);
```

- **Header**: `mfvirtualcamera.h`
- **Library**: `mfvirtualcamera.lib`
- **Min OS**: Windows 11 21H2 (Build 22000)
- **`windows` crate feature**: `"Win32_Media_MediaFoundation"` (already enabled in Cargo.toml)

### Key parameters

| Parameter       | Value                                      | Notes                                                     |
| --------------- | ------------------------------------------ | --------------------------------------------------------- |
| `type`          | `MFVirtualCameraType_SoftwareCameraSource` | Only valid type                                           |
| `lifetime`      | `MFVirtualCameraLifetime_Session`          | Camera exists while app runs — no persistent registration |
| `access`        | `MFVirtualCameraAccess_CurrentUser`        | No admin elevation needed                                 |
| `friendlyName`  | `"Cameras App — {device_name}"`            | Visible in Zoom/Teams device picker                       |
| `sourceClassId` | Custom CLSID string                        | Identifies our media source DLL                           |
| `categoryGuid`  | `KSCATEGORY_VIDEO_CAMERA`                  | Makes it appear as a camera device                        |
| `streamCount`   | `1`                                        | Single video stream                                       |

### Lifetime modes

- **Session** (`MFVirtualCameraLifetime_Session`): The virtual camera exists only
  while the `IMFVirtualCamera` COM object is alive. When the app exits or calls
  `Stop()` / `Shutdown()`, the virtual camera disappears from device enumeration.
  This is ideal — no cleanup needed after crash.
- **System** (`MFVirtualCameraLifetime_System`): Camera persists across reboots,
  needs explicit `Remove()`. Not appropriate for our use case.

## 2. COM interfaces needed

### IMFVirtualCamera

The primary interface returned by `MFCreateVirtualCamera`. Key methods:

| Method                    | Purpose                                                              |
| ------------------------- | -------------------------------------------------------------------- |
| `Start()`                 | Activates the virtual camera so it appears in device enumeration     |
| `Stop()`                  | Deactivates the camera (removes from enumeration)                    |
| `Shutdown()`              | Final cleanup — releases all resources                               |
| `AddProperty(...)`        | Sets device properties (e.g. `DEVPKEY_DeviceInterface_FriendlyName`) |
| `GetMediaSource()`        | Returns the `IMFMediaSource` backing this camera                     |
| `SendCameraProperty(...)` | Sends custom properties to the media source                          |

### IMFMediaSource (custom implementation required)

This is where frame delivery happens. We must implement a **custom media source**
as a COM DLL that the Windows Camera FrameServer loads in a separate process.

Required interfaces on the media source:

| Interface                            | Purpose                                                                                 |
| ------------------------------------ | --------------------------------------------------------------------------------------- |
| `IMFMediaSource`                     | Core media source — `Start()`, `Stop()`, `Shutdown()`, `CreatePresentationDescriptor()` |
| `IMFMediaEventGenerator`             | Event queue for `MENewStream`, `MESourceStarted`, etc.                                  |
| `IMFMediaStream`                     | Delivers `IMFSample` objects containing video frames                                    |
| `IMFMediaEventGenerator` (on stream) | Event queue for stream events (`MEStreamStarted`, `MEMediaSample`)                      |
| `IKsControl`                         | Required by FrameServer for property negotiation                                        |
| `IMFGetService` (optional)           | Enables custom service queries from consumer apps                                       |

### IMFSample / IMFMediaBuffer

Frame data is delivered as `IMFSample` objects containing `IMFMediaBuffer`:

```
IMFSample
  └── IMFMediaBuffer (or IMF2DBuffer for NV12)
      └── Raw pixel data (NV12 or RGB32)
```

## 3. Frame delivery pattern

### Architecture: out-of-process media source

The MF virtual camera API works via Windows Camera FrameServer:

```
┌─────────────────────┐     ┌──────────────────────┐     ┌──────────────┐
│  Cameras App        │     │  FrameServer Host     │     │  Consumer    │
│  (Tauri process)    │     │  (svchost.exe)        │     │  (Zoom etc.) │
│                     │     │                       │     │              │
│  JpegFrameBuffer ──────── │  Custom MediaSource   │ ──→ │  IMFSample   │
│  IMFVirtualCamera   │ IPC │  (our COM DLL)        │     │  delivery    │
│  .Start() / .Stop() │     │                       │     │              │
└─────────────────────┘     └──────────────────────┘     └──────────────┘
```

1. **Our app** calls `MFCreateVirtualCamera()` with session lifetime
2. Windows **FrameServer** loads our custom media source COM DLL in a separate process
3. Our media source receives frames via **shared memory / named pipe IPC** from the main app
4. Consumer apps (Zoom, Teams) receive `IMFSample` objects from FrameServer

### Frame flow from JpegFrameBuffer to virtual camera

```
JpegFrameBuffer::latest()
  → JPEG decode (image crate, ~5ms for 1080p)
  → Convert to NV12 (preferred) or RGB32
  → Write to shared memory / named pipe
  → Media source reads and wraps in IMFSample
  → FrameServer delivers to consumers
```

### IPC mechanism between app and media source

Since the media source runs in a separate process (FrameServer host), we need IPC:

| Option                                         | Pros                                         | Cons                                 |
| ---------------------------------------------- | -------------------------------------------- | ------------------------------------ |
| **Named shared memory** (`CreateFileMappingW`) | Zero-copy for large frames, lowest latency   | Needs synchronisation (events/mutex) |
| **Named pipe** (`CreateNamedPipeW`)            | Simple byte stream, built-in synchronisation | Memory copy overhead                 |
| **Memory-mapped file + event**                 | Best of both — zero-copy with signalling     | More complex setup                   |

**Recommended: named shared memory + named event**

- `CreateFileMappingW` with name like `Local\CamerasApp_VCam_{device_id}`
- Ring buffer in shared memory: header (write index, frame size, width, height, format) + N frame slots
- `CreateEventW` for signalling new frames
- Media source opens the same mapping and event, reads frames when signalled

### Frame format

Use **NV12** for the virtual camera output:

- NV12 is the native format for most webcam consumers (Zoom, Teams, Chrome)
- Avoids colour space conversion in the consumer
- 1.5 bytes/pixel vs 4 bytes/pixel for RGB32 (saves bandwidth in shared memory)

JPEG → RGB decode → NV12 conversion:

```
Y  = 0.299*R + 0.587*G + 0.114*B
Cb = -0.169*R - 0.331*G + 0.500*B + 128
Cr = 0.500*R - 0.419*G - 0.081*B + 128
```

NV12 memory layout for WxH:

- Y plane: W\*H bytes (one luma per pixel)
- UV plane: W\*(H/2) bytes (interleaved Cb,Cr, subsampled 2x2)
- Total: W*H*3/2 bytes

## 4. Required `windows` crate features

The project already has `"Win32_Media_MediaFoundation"` enabled. Additional features
needed:

```toml
[target.'cfg(windows)'.dependencies.windows]
version = "0.62"
features = [
    # Existing features (keep all current ones)
    "Win32_Media_MediaFoundation",

    # New features needed for MFCreateVirtualCamera
    "Win32_Devices_Properties",        # DEVPKEY_* for AddProperty
    "Win32_System_Threading",          # CreateEventW, WaitForSingleObject
    "Win32_Storage_FileSystem",        # CreateFileMappingW (if not already available)
    "Win32_Security",                  # SECURITY_ATTRIBUTES for shared memory
]
```

**Note**: `MFCreateVirtualCamera` itself lives in `Win32_Media_MediaFoundation`.
The `windows` crate generates bindings from Windows SDK metadata — the function
should be available if the crate version includes the Windows 11 SDK metadata.
If `windows` 0.62 does not include it (it was added in SDK 10.0.22000.0), we may
need to declare the FFI binding manually:

```rust
// Fallback if not in windows crate
windows::core::imp::link!("mfvirtualcamera.dll" "system"
    fn MFCreateVirtualCamera(
        r#type: MFVirtualCameraType,
        lifetime: MFVirtualCameraLifetime,
        access: MFVirtualCameraAccess,
        friendly_name: PCWSTR,
        source_class_id: PCWSTR,
        category_guid: *const GUID,
        stream_count: u32,
        pp_camera: *mut *mut core::ffi::c_void,
    ) -> HRESULT
);
```

## 5. Registration and lifecycle management

### No registration required (session lifetime)

Unlike DirectShow source filters, MF virtual cameras with session lifetime do
**not** require COM registration via `regsvr32` or SxS manifests. The media source
DLL just needs to be accessible to FrameServer.

However, the **custom media source COM DLL** must be registered so FrameServer
can instantiate it. Options:

| Approach                                                  | Admin required | Persistent   |
| --------------------------------------------------------- | -------------- | ------------ |
| `regsvr32` at install time                                | Yes (once)     | Yes          |
| Per-user COM registration (`HKCU\Software\Classes\CLSID`) | No             | Yes          |
| Registry-free COM (SxS manifest)                          | No             | No           |
| `MFRegisterLocalMediaSource` (in-process only)            | No             | No (session) |

**Recommended: per-user COM registration** at first run, falling back to SxS manifest.

### Lifecycle

```
App start
  └── User toggles virtual camera ON
      ├── Register media source COM DLL (if not already registered)
      ├── MFCreateVirtualCamera(..., Session, CurrentUser, ...) → IMFVirtualCamera
      ├── IMFVirtualCamera::Start() → camera appears in device list
      ├── Start IPC frame pump thread:
      │     loop {
      │       jpeg = JpegFrameBuffer::latest()
      │       rgb = decode(jpeg)
      │       nv12 = convert_to_nv12(rgb)
      │       write_to_shared_memory(nv12)
      │       signal_event()
      │     }
      └── User toggles OFF (or app exits)
          ├── Stop IPC frame pump thread
          ├── IMFVirtualCamera::Stop()
          └── IMFVirtualCamera::Shutdown()
              → FrameServer unloads media source
              → Camera disappears from device list
```

### Crash safety

With `MFVirtualCameraLifetime_Session`, the virtual camera is automatically
cleaned up when the `IMFVirtualCamera` COM object is released — which happens
when our process exits (even via crash). No orphaned devices.

## 6. How it fits the existing VirtualCameraSink trait

The existing `VirtualCameraSink` trait in `mod.rs`:

```rust
pub trait VirtualCameraSink: Send + Sync {
    fn start(&mut self) -> Result<(), String>;
    fn stop(&mut self) -> Result<(), String>;
    fn is_running(&self) -> bool;
}
```

### Proposed changes

The trait needs a **minor extension** to receive the `JpegFrameBuffer` reference:

```rust
pub trait VirtualCameraSink: Send + Sync {
    /// Start the virtual camera output, reading frames from the given buffer.
    fn start(&mut self, jpeg_buffer: Arc<JpegFrameBuffer>) -> Result<(), String>;
    /// Stop the virtual camera output. Idempotent.
    fn stop(&mut self) -> Result<(), String>;
    /// Whether the sink is currently outputting frames.
    fn is_running(&self) -> bool;
}
```

**Alternative**: pass `Arc<JpegFrameBuffer>` in the constructor instead of `start()`.
This is cleaner because the buffer is a dependency, not an action parameter:

```rust
// Preferred: constructor takes the buffer
pub fn create_sink(
    device_name: &str,
    jpeg_buffer: Arc<JpegFrameBuffer>,
) -> Box<dyn VirtualCameraSink>;
```

The `VirtualCameraSink` trait stays unchanged, but `create_sink()` gains parameters.

### Windows implementation structure

```rust
pub struct MfVirtualCameraSink {
    device_name: String,
    jpeg_buffer: Arc<JpegFrameBuffer>,
    // COM handle — released on drop
    vcam: Option<IMFVirtualCamera>,
    // Frame pump thread
    pump_thread: Option<JoinHandle<()>>,
    pump_running: Arc<AtomicBool>,
}

impl VirtualCameraSink for MfVirtualCameraSink {
    fn start(&mut self) -> Result<(), String> {
        // 1. Call MFCreateVirtualCamera with session lifetime
        // 2. Call vcam.Start()
        // 3. Spawn frame pump thread
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        // 1. Signal pump thread to stop
        // 2. Join pump thread
        // 3. vcam.Stop()
        // 4. vcam.Shutdown()
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.pump_running.load(Ordering::Relaxed)
    }
}
```

### Frame pump thread

```rust
fn run_frame_pump(
    jpeg_buffer: &JpegFrameBuffer,
    shared_mem: &SharedMemory,
    event: &HANDLE,
    running: &AtomicBool,
) {
    let mut last_seq = 0u64;
    while running.load(Ordering::Relaxed) {
        let seq = jpeg_buffer.sequence();
        if seq == last_seq {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        last_seq = seq;

        if let Some(frame) = jpeg_buffer.latest() {
            let rgb = decode_jpeg(&frame.jpeg_bytes);
            let nv12 = rgb_to_nv12(&rgb, frame.width, frame.height);
            shared_mem.write_frame(&nv12, frame.width, frame.height);
            SetEvent(event);
        }
    }
}
```

## 7. Threading model

### COM threading

- `MFCreateVirtualCamera` must be called from an STA or MTA thread
- The `IMFVirtualCamera` COM object is apartment-agile (supports both STA and MTA)
- Our frame pump thread should initialise COM MTA (like the encode worker does)
- The media source DLL runs in FrameServer's process — threading is managed by FrameServer

### Thread map

| Thread           | Responsibility                              | COM                    |
| ---------------- | ------------------------------------------- | ---------------------- |
| Tauri main (STA) | Calls `start()`/`stop()` via IPC commands   | STA (Tauri's tao)      |
| Frame pump       | Reads JpegFrameBuffer, writes shared memory | MTA (own init)         |
| FrameServer host | Runs our custom media source                | Managed by FrameServer |

### Thread safety

- `IMFVirtualCamera` is apartment-agile — safe to call `Start()`/`Stop()` from any thread
- `JpegFrameBuffer` is `Send + Sync` (parking_lot Mutex) — safe to read from pump thread
- Shared memory access is synchronised via named events

## 8. Custom media source COM DLL

This is the most complex part. The media source DLL must:

1. Implement `DllGetClassObject` and `DllCanUnloadNow` COM entry points
2. Provide a class factory that creates our media source
3. The media source implements `IMFMediaSource`, `IMFMediaEventGenerator`, `IKsControl`
4. It opens the named shared memory and event created by the main app
5. When `Start()` is called, it reads frames from shared memory and delivers `IMFSample` objects

### Build as separate Cargo target

The media source DLL must be a separate binary (loaded by FrameServer):

```toml
# src-tauri/Cargo.toml
[[bin]]
name = "vcam-source"
path = "src/virtual_camera/media_source/main.rs"
crate-type = ["cdylib"]
```

Or as a separate crate in the workspace:

```
src-tauri/
  vcam-source/
    Cargo.toml
    src/
      lib.rs          # DllGetClassObject, DllCanUnloadNow
      media_source.rs # IMFMediaSource impl
      media_stream.rs # IMFMediaStream impl
      class_factory.rs
      shared_memory.rs
```

### Simplified media source (frame server custom source)

Windows 11 22H2+ added **SimpleMediaSource** sample patterns. The minimum viable
implementation needs:

```rust
#[implement(IMFMediaSource, IMFMediaEventGenerator, IKsControl)]
struct VCamMediaSource {
    event_queue: IMFMediaEventQueue,
    stream: Option<VCamMediaStream>,
    shared_mem_name: String,
    // ...
}

impl IMFMediaSource_Impl for VCamMediaSource_Impl {
    fn GetCharacteristics(&self) -> Result<u32> {
        Ok(MFMEDIASOURCE_IS_LIVE.0 as u32)
    }

    fn CreatePresentationDescriptor(&self) -> Result<IMFPresentationDescriptor> {
        // Create descriptor with single NV12 stream
    }

    fn Start(
        &self,
        descriptor: Option<&IMFPresentationDescriptor>,
        _time_format: *const GUID,
        _start_position: *const PROPVARIANT,
    ) -> Result<()> {
        // Open shared memory, start delivering samples
    }

    fn Stop(&self) -> Result<()> {
        // Stop delivering, close shared memory
    }

    fn Shutdown(&self) -> Result<()> {
        // Final cleanup
    }
}
```

## 9. Rust crate alternatives

### Existing crates

| Crate     | Status     | Notes                                                           |
| --------- | ---------- | --------------------------------------------------------------- |
| `nokhwa`  | Active     | Webcam capture library — does NOT support virtual camera output |
| `eye`     | Stale      | Capture only, no output                                         |
| `v4l`     | Linux only | v4l2 bindings, could help with Linux v4l2loopback sink          |
| `windows` | Active     | Direct FFI to `MFCreateVirtualCamera` — our best option         |

**There are no existing Rust crates for MF virtual camera output.** The `windows`
crate provides raw FFI bindings, but all the COM interface implementations must
be written by hand.

### C++ reference implementations

- **Windows Camera FrameServer samples** (Microsoft): Reference `SimpleMediaSource`
  in the Windows SDK samples repo — the canonical implementation pattern
- **OBS Virtual Camera**: Uses DirectShow (not MF), but shows the frame delivery pattern
- **Unity Capture**: Uses DirectShow source filter
- **SoftCam** (GitHub): C++ DirectShow virtual camera — good reference for the
  alternative approach

### Recommendation

Use the `windows` crate directly for FFI. No third-party virtual camera crate
exists in the Rust ecosystem for the MF API. The implementation requires:

1. The main app side: `MFCreateVirtualCamera` + shared memory writer (~300 LoC)
2. The media source DLL: COM boilerplate + IMFMediaSource impl (~800-1200 LoC)

## 10. Implementation plan

### Phase 1: Shared memory + frame pump (in main app)

1. Create `SharedFrameMemory` abstraction (named shared memory + event)
2. Implement frame pump thread (reads JpegFrameBuffer → decodes → NV12 → shared memory)
3. Unit tests with mock shared memory

### Phase 2: Custom media source DLL

1. Set up `vcam-source` crate in workspace
2. Implement COM boilerplate (`DllGetClassObject`, class factory)
3. Implement `IMFMediaSource` + `IMFMediaStream`
4. Read frames from shared memory, deliver as `IMFSample`
5. Build as cdylib, test with `MFCreateVirtualCamera`

### Phase 3: Integration

1. Wire `MfVirtualCameraSink` into the existing `VirtualCameraSink` trait
2. COM registration on first use (per-user HKCU)
3. Update `create_sink()` to create `MfVirtualCameraSink` on Windows
4. End-to-end test: camera app → virtual camera → Zoom/Teams

### Phase 4: Polish

1. Error handling and diagnostics
2. Automatic cleanup on crash (session lifetime handles this)
3. Handle resolution/format changes mid-stream
4. Performance profiling (target: <5ms per frame for 1080p)

## 11. Risks and mitigations

| Risk                                                          | Impact             | Mitigation                                                                      |
| ------------------------------------------------------------- | ------------------ | ------------------------------------------------------------------------------- |
| `MFCreateVirtualCamera` not in `windows` 0.62                 | Build failure      | Declare FFI binding manually via `link!` macro                                  |
| Custom media source COM DLL is complex (~1000 LoC)            | High dev effort    | Start with SimpleMediaSource pattern from Windows SDK                           |
| FrameServer process isolation complicates debugging           | Slow iteration     | Add extensive tracing in media source, use ETW for cross-process debugging      |
| Shared memory IPC adds latency                                | Frame delay        | Use memory-mapped file with event signalling — measured <1ms overhead           |
| Per-user COM registration may fail in restricted environments | Camera not visible | Fall back to in-app-directory DLL registration or ship MSIX package             |
| NV12 conversion CPU overhead                                  | Performance        | Can use GPU (wgpu already in deps) or SIMD intrinsics; budget is ~2ms for 1080p |

## 12. Comparison: MFCreateVirtualCamera vs DirectShow source filter

| Aspect                    | MF Virtual Camera                 | DirectShow Source Filter                  |
| ------------------------- | --------------------------------- | ----------------------------------------- |
| Min OS                    | Windows 11 (22000)                | Windows 7+                                |
| COM registration          | Per-user or session               | regsvr32 or SxS manifest                  |
| Crash cleanup             | Automatic (session)               | Manual unregister needed                  |
| Consumer compatibility    | Modern apps (Zoom, Teams, Chrome) | Legacy + modern apps                      |
| Implementation complexity | Medium (media source DLL)         | High (IBaseFilter + IPin + IMemAllocator) |
| FrameServer integration   | Native                            | Via compatibility shim                    |
| Admin elevation           | Not needed (CurrentUser)          | Depends on registration method            |
| Frame delivery            | IMFSample via FrameServer         | IMediaSample via filter graph             |

**Verdict**: Given the project requires Windows 11+, MFCreateVirtualCamera is
the better choice. It is simpler to implement correctly, has automatic cleanup,
does not require admin elevation, and integrates natively with the modern camera
stack that Zoom/Teams/Chrome prefer.
