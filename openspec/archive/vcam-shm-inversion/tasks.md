## Tasks

### 1. Change SHARED_MEMORY_NAME to Global namespace

**File:** `src-tauri/crates/vcam-shared/src/lib.rs`

Change the constant from:

```rust
pub const SHARED_MEMORY_NAME: &str = r"Local\CamerasApp_VCam_0";
```

To:

```rust
pub const SHARED_MEMORY_NAME: &str = r"Global\CamerasApp_VCam_0";
```

Both the app and COM DLL import this constant, so changing it once updates both sides.

Existing tests use dynamic `Local\` names via the `test_name()` helper — they are unaffected.

### 2. Add SharedMemoryOwner to vcam-shared (COM DLL side)

**File:** `src-tauri/crates/vcam-shared/src/owner.rs` (new)

Create `SharedMemoryOwner` — the COM DLL uses this to create the `Global\` shared memory mapping with an explicit DACL.

**Public API:**

```rust
pub struct SharedMemoryOwner {
    mapping_handle: HANDLE,
    event_handle: HANDLE,
    base_ptr: *mut u8,
    frame_size: u32,
    slot_count: u32,
}

impl SharedMemoryOwner {
    /// Create a new named shared memory region in the Global namespace.
    ///
    /// Sets a DACL granting:
    /// - LOCAL SERVICE: GENERIC_ALL
    /// - Interactive users: GENERIC_READ | GENERIC_WRITE
    pub fn new(name: &str, width: u32, height: u32, slot_count: u32) -> Result<Self, Error>;

    /// Read the latest frame from the ring buffer.
    /// Returns None if no frames have been written (sequence == 0).
    pub fn read_frame(&self) -> Option<&[u8]>;

    /// Access the header.
    pub fn header(&self) -> &SharedFrameHeader;

    /// Wait for new frame signal with timeout (millis).
    /// Returns true if signalled, false on timeout.
    pub fn wait_frame(&self, timeout_ms: u32) -> bool;
}
```

**Implementation details:**

1. Build a `SECURITY_DESCRIPTOR` with `InitializeSecurityDescriptor` + `SetSecurityDescriptorDacl`
2. The DACL has two `ACCESS_ALLOWED_ACE` entries:
   - SID for `SECURITY_LOCAL_SERVICE_RID` (`S-1-5-19`) with `GENERIC_ALL`
   - SID for `SECURITY_INTERACTIVE_RID` (`S-1-5-4`) with `GENERIC_READ | GENERIC_WRITE`
3. Pass the `SECURITY_ATTRIBUTES` to `CreateFileMappingW`
4. Do the same for `CreateEventW` (the frame signal event)
5. Zero the mapped region, write the header (magic, version, width, height, format, frame_size, slot_count)

**Cargo.toml changes for `vcam-shared`:** Add `Win32_Security` feature to `windows` dependency (already present — verify). May also need `Win32_Security_Authorization` for `SetEntriesInAclW` or build ACL manually with `AddAccessAllowedAce`.

**Required Windows features (check/add to `vcam-shared/Cargo.toml`):**

```toml
[target.'cfg(windows)'.dependencies.windows]
features = [
    "Win32_Foundation",
    "Win32_System_Threading",
    "Win32_System_Memory",
    "Win32_Security",
    "Win32_Security_Authorization",  # for ACL/ACE helpers
]
```

Register the new module in `lib.rs`:

```rust
#[cfg(windows)]
pub mod owner;

#[cfg(windows)]
pub use owner::SharedMemoryOwner;
```

### 3. Add SharedMemoryProducer to vcam-shared (app side)

**File:** `src-tauri/crates/vcam-shared/src/producer.rs` (new)

Create `SharedMemoryProducer` — the app uses this to open an existing `Global\` mapping and write NV12 frames.

**Public API:**

```rust
pub struct SharedMemoryProducer {
    mapping_handle: HANDLE,
    event_handle: HANDLE,
    base_ptr: *mut u8,
    frame_size: u32,
    slot_count: u32,
}

impl SharedMemoryProducer {
    /// Open an existing named shared memory region for writing.
    ///
    /// The mapping must already exist (created by SharedMemoryOwner in the COM DLL).
    /// Validates the header (magic + version) before returning.
    pub fn open(name: &str) -> Result<Self, Error>;

    /// Write a single NV12 frame into the next ring buffer slot.
    ///
    /// Panics if nv12_data.len() doesn't match the expected frame size.
    pub fn write_frame(&self, nv12_data: &[u8]);

    /// Current frame sequence number.
    pub fn sequence(&self) -> u64;

    /// Current write index (raw, not wrapped).
    pub fn write_index(&self) -> u32;
}
```

**Implementation details:**

1. `OpenFileMappingW` with `FILE_MAP_WRITE` (not `FILE_MAP_READ`)
2. `MapViewOfFile` with `FILE_MAP_WRITE`
3. Validate header magic + version (same as current `SharedMemoryReader`)
4. `OpenEventW` for frame signalling
5. `write_frame` is identical logic to current `SharedMemoryWriter::write_frame`

Register the new module in `lib.rs`:

```rust
#[cfg(windows)]
pub mod producer;

#[cfg(windows)]
pub use producer::SharedMemoryProducer;
```

### 4. Update VCamMediaSource to create SharedMemoryOwner

**File:** `src-tauri/crates/vcam-source/src/media_source.rs`

Add a `SharedMemoryOwner` field to `VCamMediaSource`. Create it during `Start()`.

**Changes:**

1. Add field to `VCamMediaSource`:

   ```rust
   use vcam_shared::SharedMemoryOwner;

   pub(crate) struct VCamMediaSource {
       // ... existing fields ...
       shm_owner: Mutex<Option<SharedMemoryOwner>>,
   }
   ```

2. In `new()` and `new_with_attributes()`, initialise `shm_owner: Mutex::new(None)`.

3. In `IMFMediaSource_Impl::Start()`, create the owner:

   ```rust
   let shared_mem_name = self.shared_mem_name.lock().unwrap().clone();
   let owner = SharedMemoryOwner::new(
       &shared_mem_name,
       DEFAULT_WIDTH,
       DEFAULT_HEIGHT,
       3, // slot_count
   ).map_err(|e| {
       crate::trace::trace(&format!("Failed to create shared memory: {e}"));
       windows_core::Error::from(E_UNEXPECTED)
   })?;
   *self.shm_owner.lock().unwrap() = Some(owner);
   ```

4. In `Stop()` and `do_shutdown()`, drop the owner:

   ```rust
   self.shm_owner.lock().unwrap().take();
   ```

5. Pass a reference/pointer to the owner's buffer to `VCamMediaStream` so it can read frames directly, rather than each `RequestSample` call opening the mapping fresh.

**Cargo.toml change for `vcam-source`:** No change needed — it already depends on `vcam-shared`.

### 5. Update VCamMediaStream to read from SharedMemoryOwner

**File:** `src-tauri/crates/vcam-source/src/media_stream.rs`

Instead of opening `SharedMemoryReader` on every `RequestSample`, the stream receives a reference to the `SharedMemoryOwner` at construction time.

**Changes:**

1. Change the `shared_mem_name: String` field to a reference to the owner's read capability. Since we can't hold a reference (lifetime issues with COM), pass a clone/arc of the owner or pass the read pointer directly.

   **Approach:** Add a `SharedMemoryOwner` wrapped in `Arc<Mutex<...>>` or pass the raw base pointer + dimensions. The simplest correct approach: pass the `SharedMemoryOwner` behind an `Arc` so the stream can read from it.

   ```rust
   pub(crate) struct VCamMediaStream {
       // ... existing fields ...
       shm_owner: Option<Arc<SharedMemoryOwner>>,  // replaces shared_mem_name
   }
   ```

2. Update `VCamMediaStream::new()` to accept `Option<Arc<SharedMemoryOwner>>` instead of `&str`.

3. Update `read_frame_or_black()`:

   ```rust
   fn read_frame_or_black(&self) -> (Vec<u8>, u32, u32) {
       if let Some(ref owner) = self.shm_owner {
           let header = owner.header();
           let width = header.width;
           let height = header.height;
           let seq = header.sequence.load(Ordering::Acquire);

           let mut last_seq = self.last_sequence.lock().unwrap();
           if seq > *last_seq {
               *last_seq = seq;
               if let Some(data) = owner.read_frame() {
                   return (data.to_vec(), width, height);
               }
           }
       }
       generate_black_nv12(DEFAULT_WIDTH, DEFAULT_HEIGHT)
   }
   ```

4. Add `unsafe impl Send for SharedMemoryOwner {}` in `owner.rs` (the owner is sole creator, reads are synchronised via atomics).

5. Update `VCamMediaSource::Start()` to pass the `Arc<SharedMemoryOwner>` to the stream constructor.

### 6. Update frame_pump to use SharedMemoryProducer with retry

**File:** `src-tauri/src/virtual_camera/frame_pump.rs`

Change `run_frame_pump` to accept a connection function instead of a pre-built writer.

**Changes:**

1. Change the function signature from:

   ```rust
   pub fn run_frame_pump(
       jpeg_buffer: Arc<JpegFrameBuffer>,
       shm_writer: vcam_shared::SharedMemoryWriter,
       running: Arc<AtomicBool>,
   )
   ```

   To:

   ```rust
   pub fn run_frame_pump(
       jpeg_buffer: Arc<JpegFrameBuffer>,
       shm_name: String,
       running: Arc<AtomicBool>,
   )
   ```

2. At the start of the function, retry opening the shared memory:

   ```rust
   let shm_producer = loop {
       match vcam_shared::SharedMemoryProducer::open(&shm_name) {
           Ok(p) => break p,
           Err(e) => {
               if !running.load(Ordering::Relaxed) {
                   info!("Frame pump cancelled during shared memory connect");
                   return;
               }
               trace!("Shared memory not ready, retrying: {e}");
               std::thread::sleep(Duration::from_millis(100));
           }
       }
   };
   info!("Frame pump connected to shared memory '{shm_name}'");
   ```

   Add a 5-second timeout (50 iterations at 100ms):

   ```rust
   let mut retries = 0;
   let shm_producer = loop {
       match vcam_shared::SharedMemoryProducer::open(&shm_name) {
           Ok(p) => break p,
           Err(e) => {
               retries += 1;
               if retries > 50 || !running.load(Ordering::Relaxed) {
                   error!("Failed to connect to shared memory after {retries} retries: {e}");
                   return;
               }
               if retries == 1 {
                   info!("Waiting for shared memory '{shm_name}' (COM DLL must create it)...");
               }
               std::thread::sleep(Duration::from_millis(100));
           }
       }
   };
   ```

3. Replace `shm_writer.write_frame(...)` with `shm_producer.write_frame(...)`.

### 7. Update MfVirtualCamera::start_pump to pass name instead of writer

**File:** `src-tauri/src/virtual_camera/windows.rs`

Change `start_pump()` to no longer create a `SharedMemoryWriter`. Instead, pass the shared memory name to the pump thread, which will open it via `SharedMemoryProducer`.

**Changes in `start_pump()`:**

Remove:

```rust
let shm_writer =
    vcam_shared::SharedMemoryWriter::new(vcam_shared::SHARED_MEMORY_NAME, width, height, 3)
        .map_err(...)?;
```

Replace with:

```rust
let shm_name = vcam_shared::SHARED_MEMORY_NAME.to_string();
```

Update the thread spawn:

```rust
let handle = std::thread::Builder::new()
    .name("vcam-pump".to_string())
    .spawn(move || {
        super::frame_pump::run_frame_pump(jpeg_buffer, shm_name, running);
    })
    ...
```

Also update ordering: currently `start()` calls `create_virtual_camera()` then `start_pump()`. This is correct — `create_virtual_camera()` calls `vcam.Start()` which triggers FrameServer to load the DLL and call `VCamMediaSource::Start()`, which creates the `SharedMemoryOwner`. By the time `start_pump()` runs, the shared memory should exist (or will exist within ~100ms, handled by the retry loop).

### 8. Add error logging to media_stream.rs

**File:** `src-tauri/crates/vcam-source/src/media_stream.rs`

Add logging via the existing `crate::trace` module when shared memory operations encounter issues.

**Changes:**

1. In `read_frame_or_black()`, when the owner is `None` (shouldn't happen in production), log:

   ```rust
   crate::trace::trace("WARN: read_frame_or_black called but no SharedMemoryOwner set");
   ```

2. In `deliver_sample()`, if `create_nv12_sample` fails, log the error before returning it:

   ```rust
   fn deliver_sample(&self) -> windows_core::Result<()> {
       let (frame_data, width, height) = self.read_frame_or_black();
       let sample = create_nv12_sample(&frame_data, width, height).map_err(|e| {
           crate::trace::trace(&format!("ERROR: create_nv12_sample failed: {e}"));
           e
       })?;
       // ... rest unchanged
   }
   ```

### 9. Update frame_pump tests

**File:** `src-tauri/src/virtual_camera/frame_pump.rs`

Update the test `frame_pump_writes_nv12_to_shared_memory` to use the new signature. Since tests run as an interactive user (no `Global\` privilege), they should:

1. Create a `SharedMemoryWriter` (using `Local\` name) as the "owner" stand-in
2. Pass the `Local\` name to `run_frame_pump`
3. `SharedMemoryProducer::open()` works with `Local\` names too (it's just `OpenFileMappingW`)

```rust
#[cfg(windows)]
#[test]
fn frame_pump_writes_nv12_to_shared_memory() {
    // ... existing setup ...

    // Create the shared memory (simulates what SharedMemoryOwner does)
    let shm_writer =
        vcam_shared::SharedMemoryWriter::new(&shm_name, target_w, target_h, 3).unwrap();
    let shm_reader = vcam_shared::SharedMemoryReader::open(&shm_name).unwrap();

    let running_clone = Arc::clone(&running);
    let jpeg_buffer_clone = Arc::clone(&jpeg_buffer);
    let shm_name_clone = shm_name.clone();

    let pump_thread = std::thread::spawn(move || {
        run_frame_pump(jpeg_buffer_clone, shm_name_clone, running_clone);
    });

    // ... rest of test unchanged ...
}
```

The `SharedMemoryWriter` stays alive for the duration of the test, keeping the mapping open. The `SharedMemoryProducer` inside the pump opens the same mapping and writes to it.

Also update `frame_pump_skips_duplicate_frames` test similarly.

### 10. Update vcam-shared integration tests

**File:** `src-tauri/crates/vcam-shared/src/lib.rs` (test module)

Add a test that verifies `SharedMemoryProducer` can open a mapping created by `SharedMemoryWriter` (or `SharedMemoryOwner` with `Local\` name) and write/read frames correctly.

```rust
#[test]
fn producer_writes_to_writer_created_mapping() {
    let name = test_name("producer");
    let width = 4;
    let height = 2;
    let frame_size = SharedFrameHeader::nv12_frame_size(width, height) as usize;

    // SharedMemoryWriter acts as owner (creates the mapping)
    let _owner = SharedMemoryWriter::new(&name, width, height, 3).unwrap();

    // SharedMemoryProducer opens the existing mapping for writing
    let producer = SharedMemoryProducer::open(&name).unwrap();

    let reader = SharedMemoryReader::open(&name).unwrap();

    // Initially no frame
    assert!(reader.read_frame().is_none());

    // Write via producer
    let frame: Vec<u8> = (0..frame_size).map(|i| (i % 256) as u8).collect();
    producer.write_frame(&frame);

    // Read back
    let read = reader.read_frame().expect("frame should be available");
    assert_eq!(read, &frame[..]);
}
```

### 11. Remove the runtime AddProperty for Camera class GUID

**File:** `src-tauri/src/virtual_camera/windows.rs`

The `set_camera_class_guid()` function call in `create_virtual_camera()` currently warns on failure (requires elevation). This is a known limitation of session-lifetime virtual cameras.

**Keep the call as-is** — it's already non-fatal (`warn!` on failure). Document in a code comment that this requires elevation and will fail for non-admin users, meaning OBS won't discover the device via DirectShow.

No NSIS change for this — the device node doesn't exist at install time (it's session-lifetime).

### Summary of file changes

**New files:**

- `src-tauri/crates/vcam-shared/src/owner.rs` — `SharedMemoryOwner`
- `src-tauri/crates/vcam-shared/src/producer.rs` — `SharedMemoryProducer`

**Modified files:**

- `src-tauri/crates/vcam-shared/src/lib.rs` — new module registrations, `SHARED_MEMORY_NAME` → `Global\`, new integration test
- `src-tauri/crates/vcam-shared/Cargo.toml` — add `Win32_Security_Authorization` feature if needed
- `src-tauri/crates/vcam-source/src/media_source.rs` — `SharedMemoryOwner` field, create in `Start()`, drop in `Stop()`
- `src-tauri/crates/vcam-source/src/media_stream.rs` — read from `Arc<SharedMemoryOwner>` instead of opening `SharedMemoryReader`, add error logging
- `src-tauri/src/virtual_camera/frame_pump.rs` — new signature (name instead of writer), retry loop, use `SharedMemoryProducer`
- `src-tauri/src/virtual_camera/windows.rs` — `start_pump()` passes name not writer

**Unchanged files:**

- `src-tauri/crates/vcam-shared/src/writer.rs` — kept for tests
- `src-tauri/crates/vcam-shared/src/reader.rs` — kept for tests
- `src-tauri/crates/vcam-shared/src/ring_buffer.rs` — no changes
- `src-tauri/crates/vcam-shared/src/error.rs` — no changes
- `src-tauri/nsis-hooks.nsh` — no changes (HKLM COM already registered)
