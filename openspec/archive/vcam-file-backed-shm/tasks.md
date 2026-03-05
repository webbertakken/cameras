# Tasks

## Phase 1: DLL-global static SharedMemoryOwner (try first)

### 1. Add DLL-global static for SharedMemoryOwner

**File:** `src-tauri/crates/vcam-source/src/lib.rs` (or new `globals.rs`)

Add a DLL-global static that holds the `SharedMemoryOwner` across instance
lifetimes:

```rust
use std::sync::Mutex;
use vcam_shared::SharedMemoryOwner;

static SHARED_MEMORY: Mutex<Option<SharedMemoryOwner>> = Mutex::new(None);
```

Provide helper functions:

```rust
/// Get or create the global SharedMemoryOwner.
/// Returns an Arc clone for the caller to hold.
pub(crate) fn get_or_create_shared_memory(
    name: &str,
    width: u32,
    height: u32,
    slot_count: u32,
) -> Result<Arc<SharedMemoryOwner>, vcam_shared::Error> {
    // ... check if already exists, create if not ...
}
```

Note: `SharedMemoryOwner` must be `Send + Sync` (already is via unsafe impls).
The `Mutex` provides safe access.

### 2. Update VCamMediaSource to use global static

**File:** `src-tauri/crates/vcam-source/src/media_source.rs`

In `Start()`, instead of creating a new `SharedMemoryOwner` on the instance:

```rust
let owner = crate::get_or_create_shared_memory(
    &shared_mem_name,
    DEFAULT_WIDTH,
    DEFAULT_HEIGHT,
    3,
)?;
```

In `do_shutdown()`, do NOT drop the shared memory. Remove:

```rust
self.shm_owner.lock().unwrap().take();
```

Keep the per-instance `shm_owner` field as a reference/Arc clone for the
stream to use, but the actual mapping lives in the global static.

### 3. Remove shared memory drop from do_shutdown

**File:** `src-tauri/crates/vcam-source/src/media_source.rs`

The `do_shutdown()` method currently drops the `SharedMemoryOwner`:

```rust
// Drop the shared memory owner.
self.shm_owner.lock().unwrap().take();
```

Change to: do NOT drop. The global static owns it. The per-instance field
can be cleared (`.take()`) to release the Arc reference, but the global
static keeps the mapping alive.

### 4. Update VCamMediaStream to use Arc reference

**File:** `src-tauri/crates/vcam-source/src/media_stream.rs`

No change needed if we keep passing `Option<Arc<SharedMemoryOwner>>` to the
stream. The Arc reference ensures the stream can read from the mapping
even if the per-instance field is cleared.

### 5. Verify with manual test

After building:

1. Re-register: `pwsh scripts/register-vcam-dev.ps1`
2. Restart dev server: `./scripts/dev-harness.sh restart`
3. Open Windows Camera app (triggers FrameServer to load DLL)
4. Check app logs for "Frame pump connected to shared memory"
5. Check FrameServer trace for "Created shared memory" (should appear once)
6. Verify frames are delivered (non-black output in Camera app)

## Phase 2: File-backed fallback (if Phase 1 fails)

Only proceed here if `Global\` namespace proves unreliable even with the
global static (i.e., the app still gets `ERROR_FILE_NOT_FOUND` after the
mapping is confirmed to exist).

### 6. Create SharedMemoryFileOwner (app side)

**File:** `src-tauri/crates/vcam-shared/src/file_owner.rs` (new)

- Creates file at `%PROGRAMDATA%\CamerasApp\vcam_shm_0.bin`
- Uses `CreateFileW` + `CreateFileMappingW(file_handle, ...)`
- Writes header and NV12 frames
- Deletes file on drop

### 7. Create SharedMemoryFileReader (DLL side)

**File:** `src-tauri/crates/vcam-shared/src/file_reader.rs` (new)

- Opens existing file at well-known path
- Uses `CreateFileW` + `CreateFileMappingW(file_handle, ...)`
- Reads header and NV12 frames
- Does NOT delete file on drop

### 8. Update all consumers to use file-backed types

- `media_source.rs`: Use `SharedMemoryFileReader`
- `media_stream.rs`: Read from file reader
- `frame_pump.rs`: Use `SharedMemoryFileOwner`
- `windows.rs`: Create file owner, pass to pump

### 9. Remove Global\ shared memory code

Clean up the named-mapping code paths once file-backed is confirmed working.

## Summary

| Phase | Approach          | Effort                           | Risk                                       |
| ----- | ----------------- | -------------------------------- | ------------------------------------------ |
| 1     | DLL-global static | Small (move ownership to static) | Global\ might still not work cross-session |
| 2     | File-backed IPC   | Medium (new file I/O, new types) | Definitive fix, no namespace issues        |
