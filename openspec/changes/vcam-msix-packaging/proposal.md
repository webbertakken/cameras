## Why

The virtual camera COM DLL (`vcam-source`) cannot be discovered by Windows Camera FrameServer. FrameServer runs as LOCAL SERVICE, which cannot read HKCU COM registrations. `IMFVirtualCamera::AddRegistryEntry` returns ACCESS_DENIED (0x80070005) when running non-elevated with `MFVirtualCameraAccess_CurrentUser`. Without a valid COM registration visible to FrameServer, it completes init in ~1.6us without ever loading the DLL, resulting in ERROR_PATH_NOT_FOUND.

MSIX packaging with COM redirection solves this: the manifest declares the COM class, and Windows resolves the CLSID for any process (including LOCAL SERVICE) without traditional registry entries.

## What changes

- **Sparse MSIX package**: A minimal MSIX package that provides package identity and COM redirection for the virtual camera DLL, alongside the existing NSIS installer
- **AppxManifest.xml**: Declares `com:InProcessServer` for the vcam-source CLSID, `webcam` device capability, `AllowExternalContent` for sparse mode, `EntryPoint="windows.none"` (COM-only, no app launch)
- **Install-time registration**: NSIS post-install hook registers the sparse package via `Add-AppxPackage -Register -ExternalLocation` (installer already has UAC -- no extra admin prompt)
- **Exe.manifest**: MSIX identity block embedded in `cameras.exe` so Windows links the running process to the sparse package
- **Remove runtime registration**: All HKCU COM registration code, `AddRegistryEntry` calls, and runtime `PackageManager` API calls removed from `windows.rs`
- **Remove WinRT dependencies**: `Foundation`, `Management_Deployment`, `ApplicationModel` features removed from Cargo.toml

### Modified capabilities

- `mf-virtual-camera` (sidebar-canon-vcam): Remove HKCU COM registration and runtime MSIX registration; rely on install-time MSIX COM redirection

## Impact

- **Build**: Fix `scripts/build-msix.ps1` (version substitution regex), extend `build.rs` to copy DLL + manifest
- **Installer**: Add NSIS post-install hook for `Add-AppxPackage`, existing pre-uninstall hook already handles cleanup
- **Rust backend**: `src-tauri/src/virtual_camera/windows.rs` heavily simplified -- remove ~120 lines of registration code and all WinRT imports
- **Cargo.toml**: Remove 3 WinRT features (`Foundation`, `Management_Deployment`, `ApplicationModel`)
- **Exe manifest**: New `cameras.exe.manifest` with MSIX identity block, embedded via build.rs
- **vcam-test**: Remove `--register-msix` flag and runtime registration code; keep diagnostic checks
- **CI**: Updated `build.yml` to include MSIX generation step
- **New files**: `src-tauri/cameras.exe.manifest`
- **Modified files**: `src-tauri/msix/AppxManifest.xml`, `scripts/build-msix.ps1`, `src-tauri/build.rs`, `src-tauri/nsis-hooks.nsh`, `src-tauri/Cargo.toml`, `src-tauri/src/virtual_camera/windows.rs`, `src-tauri/crates/vcam-test/src/main.rs`
