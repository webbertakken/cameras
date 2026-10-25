## Context

The virtual camera uses shared memory for IPC between the main app (producer, writes NV12 frames) and the COM DLL media source (`vcam-source`, loaded by Windows FrameServer, reads frames). Currently the **app creates** the shared memory mapping and the **COM DLL opens** it.

This fails due to three compounding Windows session/privilege issues:

1. **`Local\` namespace is session-scoped** — the app runs in Session 1 (interactive), FrameServer runs in Session 0 (services). `Local\CamerasApp_VCam_0` created in Session 1 is invisible to Session 0.
2. **`Global\` namespace requires `SeCreateGlobalPrivilege`** — normal user processes (medium IL) do not have this privilege. The app cannot call `CreateFileMappingW` with a `Global\` name.
3. **Default DACL excludes LOCAL SERVICE** — even if the naming issue were solved, the default security descriptor on a mapping created by an interactive user would deny access to LOCAL SERVICE.

### Why inverting the ownership solves all three problems

- **LOCAL SERVICE (Session 0) has `SeCreateGlobalPrivilege`** — it can create `Global\` named objects natively.
- **The COM DLL runs inside FrameServer** — it can create the mapping with a DACL that explicitly grants `GENERIC_ALL` to `SECURITY_INTERACTIVE_RID` (all interactive users).
- **Opening an existing `Global\` mapping does NOT require the privilege** — only creation does. The app (Session 1) can open and map the memory for writing without any special privileges.

## Goals / non-goals

**Goals:**

- COM DLL creates the `Global\CamerasApp_VCam_0` shared memory with an explicit DACL
- App opens the existing mapping and writes NV12 frames into it
- App retries opening if the COM DLL hasn't created it yet (startup race)
- Existing tests continue to pass (they use `Local\` names, same-process)
- Add error logging to `media_stream.rs` when shared memory operations fail
- NSIS installer sets Camera class GUID in HKLM for OBS DirectShow enumeration

**Non-goals:**

- Changing the ring buffer protocol, header layout, or slot mechanics
- Dynamic resolution negotiation (stays 1920x1080 fixed)
- Multi-instance support (single shared memory region)

## Decisions

### 1. Invert SharedMemoryWriter/Reader roles

**Before:** App uses `SharedMemoryWriter` (creates mapping), COM DLL uses `SharedMemoryReader` (opens mapping).

**After:** COM DLL uses a new **`SharedMemoryOwner`** (creates `Global\` mapping with DACL), app uses a new **`SharedMemoryProducer`** (opens existing mapping for writing).

The current `SharedMemoryReader` in the COM DLL opens the mapping read-only (`FILE_MAP_READ`). The new `SharedMemoryOwner` will create the mapping and the COM DLL reads from it locally (no cross-process read needed — it owns the memory). The app needs write access, so it opens with `FILE_MAP_WRITE`.

**New types:**

| Type                   | Created by            | Role                                              | Access             | Lifetime            |
| ---------------------- | --------------------- | ------------------------------------------------- | ------------------ | ------------------- |
| `SharedMemoryOwner`    | COM DLL (FrameServer) | Creates `Global\` mapping with DACL, reads frames | Owner (read+write) | FrameServer process |
| `SharedMemoryProducer` | App                   | Opens existing mapping, writes frames             | `FILE_MAP_WRITE`   | App process         |

The old `SharedMemoryWriter` and `SharedMemoryReader` types stay for backwards compatibility (tests use them with `Local\` names in the same process), but the production code paths use the new types.

### 2. DACL grants interactive users write access

The `SharedMemoryOwner` creates a security descriptor with a DACL containing two ACEs:

1. **`SECURITY_LOCAL_SERVICE_RID`** — `GENERIC_ALL` (FrameServer's own identity, full access)
2. **`SECURITY_INTERACTIVE_RID`** — `GENERIC_READ | GENERIC_WRITE` (interactive users can open and write)

This is set via `SECURITY_ATTRIBUTES` passed to `CreateFileMappingW` and `CreateEventW`.

The SDDL equivalent: `D:(A;;GA;;;LS)(A;;GRGW;;;IU)`.

### 3. SharedMemoryOwner initialises the header

The owner creates the mapping, zeroes it, and writes the header (magic, version, width, height, format, frame_size, slot_count). This is identical to what `SharedMemoryWriter::new()` does today.

The `SharedMemoryProducer` validates the header when it opens the mapping (magic + version check), then writes frames into the ring buffer slots.

### 4. Global namespace for shared memory name

Change `SHARED_MEMORY_NAME` from `Local\CamerasApp_VCam_0` to `Global\CamerasApp_VCam_0`.

**Impact:** This affects the constant in `vcam-shared/src/lib.rs`. Both the app and the COM DLL import this constant.

Tests that create `Local\` shared memory names are unaffected — they use dynamic names via `test_name()` helper.

### 5. App retries opening with backoff

The COM DLL is loaded by FrameServer when `IMFVirtualCamera::Start()` is called. The app calls `Start()` first, then starts the frame pump. The COM DLL's `VCamMediaSource::Start()` creates the `SharedMemoryOwner`.

**Race condition:** The app's frame pump starts immediately after `vcam.Start()` returns, but FrameServer loads the DLL asynchronously. The shared memory may not exist yet when the pump first tries to open it.

**Solution:** `SharedMemoryProducer::open()` is called in a retry loop inside `run_frame_pump()`:

```
loop (up to 5 seconds, 100ms intervals):
    try open SharedMemoryProducer
    if ok: break
    if not: sleep 100ms
```

After 5 seconds without success, the pump logs an error and falls back to black frames (same as current behaviour when shared memory is unavailable).

### 6. SharedMemoryOwner lives on VCamMediaSource, not VCamMediaStream

The media source is created when FrameServer calls `ActivateObject` → `VCamMediaSource::Start()`. The media stream is created inside `Start()`. The shared memory should be owned by the source (longer lifetime) so it persists across stream restarts.

The `VCamMediaStream` reads from the owner's mapped memory region by receiving a pointer/reference to the ring buffer at construction time, rather than opening the mapping independently on each `RequestSample`.

### 7. Error logging in media_stream.rs

Currently `read_frame_or_black()` silently falls back to black when `SharedMemoryReader::open()` fails. After the inversion, the COM DLL owns the memory, so a read failure is unexpected. Add logging:

- Log at `warn` level when the frame sequence hasn't advanced (duplicate frame)
- Log at `error` level when the ring buffer header is invalid

Since the COM DLL runs inside FrameServer (SESSION 0), these logs go to the DLL trace file (`%TEMP%\vcam_source_trace.log` — but note this is LOCAL SERVICE's temp, typically `C:\Windows\ServiceProfiles\LocalService\AppData\Local\Temp\`).

### 8. NSIS registers Camera class GUID in HKLM

Currently the app calls `IMFVirtualCamera::AddProperty(DEVPKEY_Device_ClassGuid, Camera)` at runtime, which requires elevation and fails silently without it. OBS (DirectShow) only sees devices in the Camera class.

Move this to the NSIS installer. Add a registry key at install time:

```nsis
; Register Camera class GUID for OBS enumeration
; KSCATEGORY_VIDEO_CAMERA = {e5323777-f976-4f5b-9b55-b94699c46e44}
WriteRegStr HKLM "Software\Classes\CLSID\${VCAM_CLSID}" "DeviceClassGuid" "{ca3e7ab9-b4c3-4ae6-8251-579ef933890f}"
```

**Note:** This is separate from the COM CLSID registration. The Camera class GUID goes on the device node, not the COM class. The `AddProperty` approach is the correct way to set `DEVPKEY_Device_ClassGuid` on a virtual camera device — but it requires the device to already exist (created by `MFCreateVirtualCamera`). Since the device is session-lifetime, we cannot set this at install time via registry.

**Revised approach:** Keep the runtime `AddProperty` call but make it non-fatal (already is). Document that OBS visibility requires elevation for now. This is a known limitation of session-lifetime virtual cameras — the device node is created transiently.

## Architecture summary

```
                 Session 1 (Interactive user)              Session 0 (LOCAL SERVICE)
                ┌─────────────────────────┐               ┌──────────────────────────┐
                │     Main App (Tauri)     │               │    FrameServer           │
                │                          │               │                          │
                │  MfVirtualCamera         │               │  vcam_source.dll         │
                │    ├─ MFCreateVirtualCam │──Start()──>   │    ├─ VCamMediaSource     │
                │    │                     │               │    │   └─ SharedMemOwner  │
                │    └─ frame_pump thread  │               │    │       (creates       │
                │        ├─ decode JPEG    │               │    │        Global\ SHM   │
                │        ├─ BGR→NV12       │               │    │        with DACL)    │
                │        └─ SharedMemProd  │──writes──>    │    │                      │
                │            (opens        │   NV12        │    └─ VCamMediaStream     │
                │             Global\ SHM) │   frames      │        └─ reads from      │
                │                          │               │            owner's buffer  │
                └─────────────────────────┘               └──────────────────────────┘
```

## Risks / trade-offs

- **FrameServer DLL can't use `tracing` crate** — the COM DLL is a `cdylib` loaded by FrameServer, not a Tauri app. Logging must use the existing `trace.rs` file-based logger, not `tracing::info!()` macros. Verify that the trace logger works in Session 0 (LOCAL SERVICE's temp dir).
- **DACL must be correct first time** — if the DACL is wrong, the app silently falls back to black frames. Add a specific error message when `OpenFileMappingW` fails with ACCESS_DENIED.
- **Startup race** — the retry loop adds up to 5 seconds of latency on first frame. In practice, FrameServer loads the DLL within ~100ms, so this is just a safety net.
- **Version bump on protocol** — changing from `Local\` to `Global\` means old DLLs and new apps (or vice versa) won't find each other. Bump `SHARED_MEMORY_NAME` constant to make the mismatch fail fast rather than silently.
- **Testing** — `SharedMemoryOwner` uses `Global\` which requires `SeCreateGlobalPrivilege`. Unit tests running as interactive user don't have this. Tests should continue using `Local\` names via the existing `SharedMemoryWriter`/`SharedMemoryReader` (same-process, no cross-session needed).
