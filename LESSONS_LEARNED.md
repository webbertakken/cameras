# Lessons learned

## Windows `Local\` vs `Global\` shared memory namespaces

- `Local\` = session-scoped. Services (Session 0) cannot see objects from user apps (Session 1+).
- `Global\` requires `SeCreateGlobalPrivilege` to **create**, but not to **open**.
- Solution: the privileged side (COM DLL/FrameServer) creates, the app opens and writes.

## `CreateFileMappingW` default DACL excludes services

- Passing `None` for security attributes only grants access to the owner and SYSTEM.
- Must explicitly add ACEs for service accounts (e.g. LOCAL SERVICE via `SECURITY_LOCAL_SERVICE_RID`).

## Windows `MFVideoFormat_RGB24` is physically BGR

- Despite the name, byte order is B-G-R (offset 0 = Blue).
- Using RGB byte order for NV12 conversion produces a heavy red tint that looks like a lighting issue.

## `IMFVirtualCamera::AddProperty` needs elevation

- Setting `DEVPKEY_Device_ClassGuid` fails with ACCESS_DENIED as normal user.
- Virtual cameras default to `SoftwareDevice` PnP class — invisible to DirectShow-based apps (OBS).
- Device node is session-lifetime, so no install-time fix possible.

## Canon EVF resolution is model-dependent

- Actual output: 960×640 (crop), 1056×704 (full), 1920×1280 (newer EOS R bodies).
- EDSDK has no API to query EVF dimensions. Our Canon EOS 2000D sends 1056×704.
- Frame pump reads real dimensions from decoded JPEG — resize handles any input.

## FrameServer locks `vcam_source.dll`

- Must `Stop-Service FrameServer -Force` (elevated) before rebuilding the DLL.
- After rebuild: re-register (`register-vcam-dev.ps1`) then `Start-Service FrameServer`.

## Virtual camera requires dual registration

- MSIX sparse package = package identity (for `MFCreateVirtualCamera`).
- HKLM `InProcServer32` = COM visibility (for FrameServer/LOCAL SERVICE).
- Both are required. MSIX alone won't work — FrameServer can't see per-user MSIX COM redirection.

## FrameServer has isolated kernel object namespace

- FrameServer hosts COM DLLs in a process with its **own kernel object namespace**.
- `Global\` inside FrameServer does NOT map to the same `\BaseNamedObjects` that user-session apps see.
- `CreateFileMappingW` with `Global\` prefix succeeds inside FrameServer (LOCAL SERVICE has `SeCreateGlobalPrivilege`), but `OpenFileMappingW` from the app returns `ERROR_FILE_NOT_FOUND` — the object is invisible across the namespace boundary.
- This is NOT an AppContainer issue (FrameServer is UNRESTRICTED, ServiceSidType 1).
- **Workaround**: Use file-backed shared memory (real file on disk) instead of named kernel objects. File paths work across all namespace boundaries.

## FrameServer creates new COM instances per probe cycle

- FrameServer calls `DllGetClassObject` → `CreateInstance` → `Start()` → `Shutdown()` repeatedly, creating **entirely new** `IMFMediaSource` instances each time.
- Per-instance state (like shared memory owners) lives only 6-22ms per cycle.
- Any persistent state must be DLL-global (e.g. `static GLOBAL_SHM_OWNER: Mutex<Option<Arc<...>>>`) to survive instance cycling.
- First Start() creates, subsequent Start() calls reuse the global.

## FrameServer trace logs require elevation to read

- DLL trace log at `C:\Windows\ServiceProfiles\LocalService\AppData\Local\Temp\vcam_source_trace.log` is owned by LOCAL SERVICE.
- Non-elevated `Test-Path` returns `False` (access denied), making it look like the file doesn't exist.
- Must use `Start-Process pwsh -Verb RunAs` to read, or copy to a user-accessible location.
- User-side trace (`%USERPROFILE%\AppData\Local\Temp\`) only shows in-process probes, not FrameServer calls.
