## Context

The virtual camera feature uses `MFCreateVirtualCamera` (Windows 11+) to register a session-lifetime virtual camera backed by a custom COM media source DLL (`vcam-source`). The DLL implements `IMFMediaSource`, `IMFMediaStream2`, `IKsControl`, and other interfaces required by Windows Camera FrameServer.

The blocker: FrameServer runs as LOCAL SERVICE and cannot see HKCU COM registrations. The current code registers the CLSID under `HKCU\Software\Classes\CLSID\{...}\InProcServer32` and also calls `IMFVirtualCamera::AddRegistryEntry`, but both approaches fail:

- **HKCU registration**: LOCAL SERVICE has a different HKCU hive -- it never sees the app's registration
- **AddRegistryEntry**: Returns ACCESS_DENIED (0x80070005) when using `MFVirtualCameraAccess_CurrentUser` without elevation

MSIX packaging with COM redirection provides a clean solution: the package manifest declares the COM class with its DLL path, and Windows resolves the CLSID for ALL processes on the system -- including services like FrameServer.

## Goals / non-goals

**Goals:**

- FrameServer can discover and load `vcam_source.dll` via MSIX COM redirection
- No elevation required at runtime -- works with `MFVirtualCameraAccess_CurrentUser`
- Sparse package registration happens at install time (NSIS installer already has UAC)
- Dev builds work with self-signed certificates
- CI produces signed MSIX alongside existing NSIS installer
- Minimal impact on existing build and packaging pipeline

**Non-goals:**

- Full MSIX installer replacing NSIS/MSI -- we use a "sparse package" for identity + COM only
- Runtime sparse package registration -- moved to install time to avoid elevation issues
- Microsoft Store submission (separate concern, not blocked by this change)
- Changing the virtual camera's session lifetime or IPC mechanism

## Decisions

### 1. Sparse MSIX package (not full MSIX replacement)

Tauri v2 does not natively support MSIX as a build target. Rather than replacing the NSIS installer, we create a **sparse MSIX package** -- a minimal MSIX that provides package identity and COM class declarations. The sparse package points to the existing installed binary location.

**Why sparse?** A full MSIX conversion would require abandoning Tauri's NSIS bundler, managing VFS paths, and dealing with the MSIX sandbox (filesystem virtualisation, registry virtualisation). A sparse package avoids all of that -- it is a thin overlay that only provides identity and extension declarations.

**How it works:**

1. The NSIS installer installs the app as usual (exe + DLLs in `Program Files`)
2. The NSIS post-install hook registers the sparse package via `Add-AppxPackage -Register -ExternalLocation`
3. The sparse package's manifest declares the COM class pointing to the installed `vcam_source.dll`
4. FrameServer resolves the CLSID via the package's COM redirection

### 2. AppxManifest.xml with com:InProcessServer (COM-only, no app launch)

The manifest declares the vcam-source CLSID as an in-process COM server. Since this is a COM-only sparse package (the app itself is launched by NSIS, not the MSIX), we use `EntryPoint="windows.none"` and `AppListEntry="none"` to avoid needing `runFullTrust`.

```xml
<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:uap10="http://schemas.microsoft.com/appx/manifest/uap/windows10/10"
  xmlns:com="http://schemas.microsoft.com/appx/manifest/com/windows10"
  IgnorableNamespaces="uap10">

  <Identity
    Name="io.takken.cameras"
    Publisher="CN=Cameras App Dev"
    Version="0.7.0.0"
    ProcessorArchitecture="x64" />

  <Properties>
    <DisplayName>Cameras Virtual Camera</DisplayName>
    <PublisherDisplayName>Webber Takken</PublisherDisplayName>
    <Logo>icons\icon.png</Logo>
    <uap10:AllowExternalContent>true</uap10:AllowExternalContent>
  </Properties>

  <Resources>
    <Resource Language="en-us" />
  </Resources>

  <Dependencies>
    <TargetDeviceFamily
      Name="Windows.Desktop"
      MinVersion="10.0.22000.0"
      MaxVersionTested="10.0.26100.0" />
  </Dependencies>

  <Capabilities>
    <DeviceCapability Name="webcam" />
  </Capabilities>

  <Applications>
    <Application
      Id="CamerasApp"
      Executable="cameras.exe"
      uap10:TrustLevel="mediumIL"
      uap10:RuntimeBehavior="win32App"
      EntryPoint="windows.none">

      <uap:VisualElements
        DisplayName="Cameras Virtual Camera"
        Square150x150Logo="icons\icon.png"
        Square44x44Logo="icons\icon.png"
        Description="Virtual camera COM extension"
        AppListEntry="none"
        BackgroundColor="#FFFFFF" />

      <Extensions>
        <com:Extension Category="windows.comServer">
          <com:ComServer>
            <com:InProcessServer Path="vcam_source.dll">
              <com:Class
                Id="7B2E3A1F-4D5C-4E8B-9A6F-1C2D3E4F5A6B"
                DisplayName="Cameras App Virtual Camera Source"
                ThreadingModel="Both" />
            </com:InProcessServer>
          </com:ComServer>
        </com:Extension>
      </Extensions>

    </Application>
  </Applications>
</Package>
```

Key points:

- **No `runFullTrust` capability** -- not needed for COM-only sparse packages that don't launch an exe
- **`EntryPoint="windows.none"`** -- tells Windows this is not a launchable app, prevents MakeAppx from requiring `cameras.exe` in the staging directory
- **`uap10:TrustLevel="mediumIL"` + `uap10:RuntimeBehavior="win32App"`** -- identifies it as a Win32 companion package
- **`AppListEntry="none"`** -- prevents a Start Menu entry
- **`uap10:AllowExternalContent`** -- marks this as a sparse package pointing to an external location
- The `Id` in `com:Class` matches `VCAM_SOURCE_CLSID` from `vcam-shared`
- `ThreadingModel="Both"` matches what the DLL supports
- `Path="vcam_source.dll"` is relative to the external location (the install directory)
- Version must be `X.Y.Z.0` format (4-part, last must be 0 for Store compat)

### 3. Install-time sparse package registration (not runtime)

Sparse MSIX registration via `PackageManager::AddPackageByUriAsync` requires admin/elevation in production scenarios:

- `AllowExternalContent` is a restricted capability requiring system-level modifications
- Signing certificates must be in `LocalMachine\Trusted People` (admin-only store)
- `AllowUnsigned(true)` requires Developer Mode (admin to enable)

Since the NSIS installer already runs elevated (UAC prompt is standard for any installer), we register the sparse package during install -- not at app runtime. This means:

- **No admin required after install** -- the virtual camera feature works as a standard user
- **No `PackageManager` API calls in the main app** -- simpler code, fewer dependencies
- **No WinRT imports in the main Cargo.toml** -- `Foundation`, `Management_Deployment`, `ApplicationModel` are not needed

**NSIS post-install hook** (`nsis-hooks.nsh`):

```nsis
!macro NSIS_HOOK_POSTINSTALL
  ; Register the sparse MSIX package for COM redirection.
  ; The manifest's ExternalLocation points to the install directory.
  nsExec::ExecToLog 'powershell -NoProfile -Command "Add-AppxPackage -Register -ExternalLocation \"$INSTDIR\" (Join-Path \"$INSTDIR\" \"AppxManifest.xml\")"'
!macroend
```

**NSIS pre-uninstall hook** (already exists):

```nsis
!macro NSIS_HOOK_PREUNINSTALL
  nsExec::ExecToLog 'powershell -NoProfile -Command "Get-AppxPackage -Name io.takken.cameras | Remove-AppxPackage"'
!macroend
```

### 4. Exe.manifest with MSIX identity block

For the sparse package identity to link to the running process, the exe must have an embedded manifest declaring its MSIX identity. The values must exactly match the `AppxManifest.xml`.

The `<msix>` block goes inside the `<compatibility>` section of the exe's application manifest:

```xml
<compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
  <application>
    <msix xmlns="urn:schemas-microsoft-com:msix.v1"
          publisher="CN=Cameras App Dev"
          packageName="io.takken.cameras"
          applicationId="CamerasApp" />
  </application>
</compatibility>
```

Tauri generates the exe manifest via `tauri_build::build()` (resource ID 1, type 24 = RT_MANIFEST). The current manifest only has a Common Controls v6 dependency. We need to add the MSIX identity block.

**Approach: Custom `.manifest` file + `embed-resource` crate** in `build.rs`:

1. Create `src-tauri/cameras.exe.manifest` with the full manifest including both the Common Controls dependency and the MSIX identity block
2. In `build.rs`, use the `embed-resource` crate (or a custom `.rc` file) to embed the manifest, replacing Tauri's generated one
3. Alternatively, if Tauri's `tauri_build::build()` supports custom manifest content, use that

### 5. Remove all runtime MSIX registration and HKCU code from windows.rs

With install-time registration, `windows.rs` becomes much simpler. Remove:

- `ensure_sparse_package_registered()` method
- `register_sparse_package()` function
- `is_sparse_package_registered()` function
- `find_msix_path()` function
- `SPARSE_REGISTERED` static `OnceLock`
- `SPARSE_PACKAGE_NAME` constant
- `OnceLock` import
- `windows::Foundation::Uri` import
- `windows::Management::Deployment::{AddPackageOptions, PackageManager}` import

The `create_virtual_camera()` method becomes simply:

```rust
fn create_virtual_camera(&mut self) -> Result<(), String> {
    let clsid_str = clsid_string();
    let friendly_name = format!("Cameras App \u{2014} {}", self.device_name);
    let friendly_name_h = HSTRING::from(&friendly_name);
    let clsid_h = HSTRING::from(&clsid_str);
    let categories = [KSCATEGORY_VIDEO_CAMERA];

    let vcam = unsafe {
        MFCreateVirtualCamera(
            MFVirtualCameraType_SoftwareCameraSource,
            MFVirtualCameraLifetime_Session,
            MFVirtualCameraAccess_CurrentUser,
            &friendly_name_h,
            &clsid_h,
            Some(&categories),
        )
    }.map_err(|e| format!("MFCreateVirtualCamera failed: {e}"))?;

    unsafe { vcam.Start(None) }
        .map_err(|e| format!("IMFVirtualCamera::Start failed: {e}"))?;

    self.vcam = Some(vcam);
    Ok(())
}
```

### 6. Build pipeline changes

**`scripts/build-msix.ps1`** (already exists, needs fixes):

- Fix the version substitution regex to only match the `<Identity>` element's `Version` attribute (currently replaces `MinVersion` too)
- The MSIX no longer needs `cameras.exe` in the staging directory (since `EntryPoint="windows.none"`)

**`build.rs`** (extend existing):

- Copy `vcam_source.dll` from workspace target directory to main app target directory
- Pattern matches the existing EDSDK `copy_sdk_file` approach
- Also copy `AppxManifest.xml` to the target directory (needed by NSIS installer)

**CI changes in `build.yml`:**

1. After `tauri build`, run `build-msix.ps1` to generate the sparse package
2. Include the `.msix` in release artefacts alongside the NSIS installer

### 7. Cargo.toml: remove WinRT features

Remove the features that were only needed for runtime sparse package registration:

- `"Foundation"`
- `"Management_Deployment"`
- `"ApplicationModel"`

These are no longer used by any code in the main app. The `Win32_System_Registry` feature can also be removed since we no longer write HKCU registry entries.

## Risks / trade-offs

- **[MSIX signing for dev builds]** Self-signed certificates require one-time trust installation in `LocalMachine\Trusted People` (admin). Mitigated by `scripts/create-dev-cert.ps1` which handles cert creation and trust store installation.
- **[Sparse package persistence]** The sparse package registration survives app crashes but not uninstall. The NSIS pre-uninstall hook handles cleanup via `Remove-AppxPackage`.
- **[FrameServer COM redirection is underdocumented]** Microsoft's docs don't explicitly confirm that FrameServer resolves CLSIDs via MSIX COM redirection. However, MSIX COM redirection is system-wide (all processes see it, including services). If this fails, the fallback is `MFVirtualCameraAccess_AllUsers` with elevation (via UAC prompt at first use).
- **[Windows SDK tools required]** `MakeAppx.exe` and `SignTool.exe` are needed for MSIX creation. These ship with the Windows SDK which is already required for Rust Windows development.
- **[Version synchronisation]** The MSIX package version must stay in sync with the app version. Mitigated by reading the version from `tauri.conf.json` in the build script.
- **[Exe.manifest embedding]** Tauri generates its own exe manifest. We need to either replace it or extend it with the MSIX identity block. If Tauri's build process conflicts with our custom manifest, we may need to post-process the exe after Tauri's build step.
- **[NSIS install-time registration failure]** If `Add-AppxPackage` fails during install (e.g. policy restrictions), the app installs but virtual camera won't work. Mitigated by logging the error and showing a post-install note. The app itself should detect the missing package registration and show a helpful error when the user tries to enable virtual camera.
