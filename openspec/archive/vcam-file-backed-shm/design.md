# Fix virtual camera shared memory: file-backed IPC

## Problem

The virtual camera's shared memory IPC between the app and the COM DLL
(loaded by Windows FrameServer) is broken. Both `Global\` named kernel objects
and DLL-global statics fail because FrameServer's process has **its own kernel
object namespace** — `Global\` inside FrameServer is NOT the same `Global\` the
app sees.

### Evidence

**Phase 1 (DLL-global static) confirmed the namespace isolation:**

- DLL creates `Global\CamerasApp_VCam_0` successfully (trace: "Created shared
  memory" + "Reusing existing DLL-global shared memory owner")
- DLL-global static keeps it alive across instance cycling (confirmed)
- App's `OpenFileMappingW("Global\\CamerasApp_VCam_0")` returns
  `ERROR_FILE_NOT_FOUND` for 500+ seconds — the object simply does not exist
  in the app's namespace

**FrameServer process details:**

- Runs as `NT AUTHORITY\LocalService`, Session 0
- ServiceSidType 1 (UNRESTRICTED) — NOT an AppContainer
- Despite not being AppContainer, FrameServer svchost has namespace isolation
  (possibly via per-service SID object namespace or undocumented FrameServer
  sandboxing)

### Why kernel object names don't work

Named kernel objects (`CreateFileMappingW`, `CreateEventW` with `Global\` or
`Local\` prefix) are scoped to the object namespace directory of the calling
process. FrameServer's svchost process uses a different object directory than
the interactive user's processes. There is no documented way to bridge this.

## Solution: file-backed shared memory

Replace kernel-named objects with **file-backed** shared memory. File paths are
universal — they work across all sessions, all namespaces, all accounts (given
correct ACLs).

### Architecture

```
App (Session 1, interactive user)          FrameServer (Session 0, LOCAL SERVICE)
┌───────────────────────────────┐          ┌──────────────────────────────────┐
│                               │          │                                  │
│ SharedMemoryOwner             │          │ VCamMediaSource::Start()         │
│   1. Create directory         │          │   1. Open file (read-only)       │
│   2. CreateFileW (rw)         │          │   2. CreateFileMappingW(file,    │
│   3. SetFileInformationByH    │          │      PAGE_READONLY)              │
│      (set file size)          │          │   3. MapViewOfFile(read)         │
│   4. CreateFileMappingW(file, │          │   4. Validate header             │
│      PAGE_READWRITE)          │  file    │   5. Read frames via             │
│   5. MapViewOfFile(rw)        │ ──────>  │      SharedMemoryReader          │
│   6. Write header             │ on disk  │                                  │
│   7. frame_pump writes NV12   │          │ RequestSample()                  │
│      frames into ring buffer  │          │   read latest frame from         │
│                               │          │   mapped memory, wrap in         │
│ Drop: unmap, close, delete    │          │   IMFSample, deliver             │
└───────────────────────────────┘          └──────────────────────────────────┘
```

### Key decisions

#### 1. Who creates the file? **The app.**

- The app creates the file at virtual camera start time, BEFORE calling
  `MFCreateVirtualCamera` / `Start()`
- The file is ready by the time FrameServer loads the DLL and calls `Start()`
- The DLL opens the file in `Start()` — if it doesn't exist yet, deliver black
  frames (race window is near-zero since the app creates the file first)
- On `Shutdown()`, the DLL closes its handles but does NOT delete the file
- The app deletes the file when stopping the virtual camera

This model is simpler and more robust than DLL-creates because:

- The app has a clear, long lifecycle (user session)
- The DLL can be cycled endlessly — each instance just re-opens the same file
- No need for DLL-global statics
- No orphaned kernel objects

#### 2. File path convention

```
C:\ProgramData\CamerasApp\vcam_shm_0.bin
```

Why `%PROGRAMDATA%` (`C:\ProgramData`):

- Both interactive users and LOCAL SERVICE can access it
- Default ACLs: `BUILTIN\Users` gets `ReadAndExecute + Write`
  (LOCAL SERVICE is a member of `BUILTIN\Users`)
- Confirmed by testing: file created by interactive user inherits
  `Users: ReadAndExecute` — LOCAL SERVICE can read it
- Survives user logoff, not tied to any user profile
- Well-known location, no registry or IPC needed to communicate the path

The constant in `vcam-shared/src/lib.rs`:

```rust
/// Well-known file path for shared memory IPC.
/// Both the app and COM DLL use this path to find each other.
pub const SHARED_MEMORY_PATH: &str = r"C:\ProgramData\CamerasApp\vcam_shm_0.bin";
```

Note: hardcoded `C:\ProgramData` is fine because `%PROGRAMDATA%` is always
`C:\ProgramData` on standard Windows installations. If we ever need to support
non-standard installs, resolve at runtime via `SHGetKnownFolderPath(FOLDERID_ProgramData)`.

#### 3. Event signalling: **not needed**

Named events have the same namespace isolation problem as named mappings. But
we don't actually need them:

- The DLL's `RequestSample()` is called by FrameServer at the consumer's frame
  rate (typically 30fps). It is already demand-driven.
- `RequestSample` reads the `sequence` atomic from the shared header. If it has
  advanced since the last delivery, read the new frame. If not, redeliver the
  previous frame (or black).
- The app's frame pump writes at its own pace. No synchronisation needed beyond
  the acquire/release atomics on `write_index` and `sequence`.
- The existing `wait_frame` event was never called by `RequestSample` — it was
  only used in tests. Tests can use `Local\` events (same process, no namespace
  issue) or simply poll.

**Remove event creation and signalling entirely** from the production path.
Keep `SharedMemoryWriter`/`SharedMemoryReader` (which use events) for tests.

#### 4. DACL on the file

The default `C:\ProgramData` ACL is sufficient:

- Creator (interactive user): `FullControl` (inherited from `CREATOR OWNER`)
- `BUILTIN\Users`: `ReadAndExecute, Synchronize` (inherited)
- LOCAL SERVICE is in `BUILTIN\Users` — gets read access automatically

No explicit DACL code needed. Just create the file with default security.

If we want to be explicit (belt and suspenders), set a DACL on the file:

- `SECURITY_INTERACTIVE_RID` (S-1-5-4): `GENERIC_ALL`
- `SECURITY_LOCAL_SERVICE_RID` (S-1-5-19): `GENERIC_READ`

But this is optional — the inherited ACLs already provide the right access.

#### 5. File size

For 1920x1080 NV12 with 3 ring buffer slots:

- Header: 64 bytes
- Frame data: 3 x 3,110,400 = 9,331,200 bytes
- Total: 9,331,264 bytes (~8.9 MB)

The file is pre-allocated to this size. Both sides map the entire file.

#### 6. Cleanup

- **Normal shutdown**: app unmaps, closes handles, deletes file
- **App crash**: file persists (~9 MB, harmless). Next app start creates a new
  file (overwrites). NSIS uninstaller removes `C:\ProgramData\CamerasApp\`.
- **DLL side**: closes mapping and file handles. Never deletes the file.

## Changes to existing types

### SharedMemoryOwner (app side) — `vcam-shared/src/owner.rs`

Rewrite to use file-backed mapping:

```rust
pub struct SharedMemoryOwner {
    file_handle: HANDLE,
    mapping_handle: HANDLE,
    base_ptr: *mut u8,
    file_path: PathBuf,
    _frame_size: u32,
    _slot_count: u32,
}

impl SharedMemoryOwner {
    /// Create a file-backed shared memory region.
    ///
    /// Creates (or overwrites) the file at `file_path`, sets its size,
    /// creates a file mapping, and initialises the ring buffer header.
    pub fn new(
        file_path: &Path,
        width: u32,
        height: u32,
        slot_count: u32,
    ) -> Result<Self, Error> { ... }

    /// Write a single NV12 frame into the next ring buffer slot.
    pub fn write_frame(&self, nv12_data: &[u8]) { ... }

    /// Current sequence number.
    pub fn sequence(&self) -> u64 { ... }

    /// Current write index.
    pub fn write_index(&self) -> u32 { ... }
}

impl Drop for SharedMemoryOwner {
    fn drop(&mut self) {
        // Unmap, close mapping handle, close file handle, delete file
    }
}
```

Key implementation details:

1. `CreateFileW` with `GENERIC_READ | GENERIC_WRITE`, `FILE_SHARE_READ`
   (allow DLL to open for reading while app has it open)
2. Set file size via `SetFileInformationByHandle` or `SetEndOfFile`
3. `CreateFileMappingW` with the file handle (NOT `INVALID_HANDLE_VALUE`)
   and `PAGE_READWRITE`
4. `MapViewOfFile` with `FILE_MAP_ALL_ACCESS`
5. Zero the region, write header (same as current)
6. `write_frame` is identical to current `SharedMemoryProducer::write_frame`
7. Drop: `UnmapViewOfFile`, `CloseHandle(mapping)`, `CloseHandle(file)`,
   `DeleteFileW(path)`

### SharedMemoryProducer — `vcam-shared/src/producer.rs`

**Remove.** Its write functionality is merged into `SharedMemoryOwner`.

The current `SharedMemoryProducer` opens an existing named mapping and writes
frames. With file-backed IPC, the owner both creates AND writes. No need for
a separate type.

### SharedMemoryReader (DLL side) — REUSE from `vcam-shared/src/reader.rs`

Add a new constructor that opens by file path instead of kernel object name:

```rust
impl SharedMemoryReader {
    /// Open an existing file-backed shared memory region for reading.
    pub fn open_file(file_path: &Path) -> Result<Self, Error> {
        // 1. CreateFileW with GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE
        // 2. CreateFileMappingW with file handle, PAGE_READONLY
        // 3. MapViewOfFile with FILE_MAP_READ
        // 4. Validate header (magic, version)
        // 5. Return reader
    }
}
```

The existing `SharedMemoryReader::open(name: &str)` stays for tests (uses
`OpenFileMappingW` with kernel object names in `Local\` namespace).

### lib.rs constants

```rust
/// Well-known file path for shared memory IPC between app and COM DLL.
pub const SHARED_MEMORY_FILE_PATH: &str = r"C:\ProgramData\CamerasApp\vcam_shm_0.bin";

// Keep the old constant for tests that use kernel object names
pub const SHARED_MEMORY_NAME: &str = r"Global\CamerasApp_VCam_0";
```

### VCamMediaSource (DLL side) — `vcam-source/src/media_source.rs`

Change from `SharedMemoryOwner` to `SharedMemoryReader`:

```rust
pub(crate) struct VCamMediaSource {
    // ...
    shm_reader: Mutex<Option<Arc<SharedMemoryReader>>>,  // was: shm_owner
}
```

In `Start()`:

```rust
let reader = SharedMemoryReader::open_file(Path::new(SHARED_MEMORY_FILE_PATH))
    .map_err(|e| {
        crate::trace::trace(&format!("SHM file not ready: {e}"));
        // Not fatal — deliver black frames until file appears
    })
    .ok();
```

If the file doesn't exist, `reader` is `None` and the stream delivers black
frames (existing `read_frame_or_black` fallback).

**Remove the DLL-global static** from Phase 1. Per-instance `SharedMemoryReader`
is fine because:

- Opening a file is cheap
- No state needs to persist across instances
- Each instance gets a fresh read mapping

### VCamMediaStream (DLL side) — `vcam-source/src/media_stream.rs`

Change `shm_owner: Option<Arc<SharedMemoryOwner>>` to
`shm_reader: Option<Arc<SharedMemoryReader>>`.

`read_frame_or_black` uses `reader.read_frame()` instead of `owner.read_frame()`.
The interface is identical — both have `read_frame() -> Option<&[u8]>` and
`header() -> &SharedFrameHeader`.

### frame_pump.rs (app side)

Simplify — no more retry loop:

```rust
pub fn run_frame_pump(
    jpeg_buffer: Arc<JpegFrameBuffer>,
    shm_owner: Arc<SharedMemoryOwner>,  // pre-created by caller
    running: Arc<AtomicBool>,
) {
    info!("Frame pump started");

    let mut last_seq = 0u64;
    let mut frames_delivered = 0u64;

    while running.load(Ordering::Relaxed) {
        // ... same decode/resize/nv12 logic ...
        shm_owner.write_frame(&nv12_data);
        // ... same logging ...
    }
}
```

The `SharedMemoryOwner` is passed in pre-created. No retry needed because the
app creates the file before starting the pump.

### windows.rs (app side)

In `start_pump()`:

```rust
fn start_pump(&mut self) -> Result<(), String> {
    let file_path = Path::new(vcam_shared::SHARED_MEMORY_FILE_PATH);

    // Create parent directory if needed
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }

    let shm_owner = Arc::new(
        SharedMemoryOwner::new(file_path, DEFAULT_WIDTH, DEFAULT_HEIGHT, 3)
            .map_err(|e| format!("shared memory: {e}"))?
    );

    // ... spawn pump thread with shm_owner ...
}
```

Create the shared memory BEFORE calling `MFCreateVirtualCamera` / `Start()`.
The file exists by the time FrameServer loads the DLL.

In `stop()`:

```rust
// SharedMemoryOwner::drop() handles unmap + close + delete
```

## Testing strategy

### Unit tests (same-process, `Local\` names)

Existing `SharedMemoryWriter`/`SharedMemoryReader` tests continue to work
unchanged — they use `Local\` kernel object names within the same process.

### New integration test for file-backed path

```rust
#[cfg(windows)]
#[test]
fn file_backed_owner_reader_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test_shm.bin");

    let owner = SharedMemoryOwner::new(&path, 4, 2, 3).unwrap();
    let reader = SharedMemoryReader::open_file(&path).unwrap();

    // Initially no frame
    assert!(reader.read_frame().is_none());

    // Write via owner
    let frame_size = SharedFrameHeader::nv12_frame_size(4, 2) as usize;
    let frame: Vec<u8> = (0..frame_size).map(|i| (i % 256) as u8).collect();
    owner.write_frame(&frame);

    // Read back
    let read = reader.read_frame().expect("frame should be available");
    assert_eq!(read, &frame[..]);
}
```

### frame_pump test update

Update `frame_pump_writes_nv12_to_shared_memory` to create a
`SharedMemoryOwner` with a temp file path and pass it to the pump.

## Cargo.toml changes

### vcam-shared/Cargo.toml

Add `Win32_Storage_FileSystem` for `CreateFileW`, `DeleteFileW`,
`SetFileInformationByHandle`:

```toml
[target.'cfg(windows)'.dependencies.windows]
version = "0.62"
features = [
    "Win32_Foundation",
    "Win32_System_Threading",
    "Win32_System_Memory",
    "Win32_Security",
    "Win32_Storage_FileSystem",  # NEW
]
```

Add `tempfile` as a dev dependency for tests:

```toml
[dev-dependencies]
tempfile = "3"
```

## Summary of changes

| File                              | Action                                                                            |
| --------------------------------- | --------------------------------------------------------------------------------- |
| `vcam-shared/src/owner.rs`        | Rewrite: file-backed, creates+writes                                              |
| `vcam-shared/src/producer.rs`     | Delete (merged into owner)                                                        |
| `vcam-shared/src/reader.rs`       | Add `open_file()` constructor                                                     |
| `vcam-shared/src/lib.rs`          | Add `SHARED_MEMORY_FILE_PATH`, remove `producer` module, keep old types for tests |
| `vcam-shared/Cargo.toml`          | Add `Win32_Storage_FileSystem`, `tempfile` dev-dep                                |
| `vcam-source/src/media_source.rs` | Use `SharedMemoryReader::open_file()`, remove DLL-global static                   |
| `vcam-source/src/media_stream.rs` | `shm_owner` → `shm_reader` (same interface)                                       |
| `vcam-source/src/lib.rs`          | Remove DLL-global static from Phase 1                                             |
| `virtual_camera/frame_pump.rs`    | Accept `Arc<SharedMemoryOwner>`, no retry loop                                    |
| `virtual_camera/windows.rs`       | Create `SharedMemoryOwner` in `start_pump()`, pass to pump                        |

## Risks

- **File I/O overhead**: Negligible. Memory-mapped file access goes through the
  OS page cache — same performance as anonymous mappings after initial fault.
- **Stale file after crash**: ~9 MB orphan. Harmless. Next start overwrites.
- **Antivirus interference**: Some AV may flag rapid writes to a file in
  ProgramData. Unlikely for a known app, but worth monitoring.
- **Disk space**: 9 MB per virtual camera instance. Trivial.
