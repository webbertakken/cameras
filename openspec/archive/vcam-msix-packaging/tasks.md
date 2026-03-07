## Tasks

### 1. Fix AppxManifest.xml for sparse COM-only package

**File:** `src-tauri/msix/AppxManifest.xml`

Replace the current manifest with the corrected sparse package structure. Specific changes:

- **Remove** `StartPage="index.html"` (this is for JS UWP apps, not Win32)
- **Add** `xmlns:uap10="http://schemas.microsoft.com/appx/manifest/uap/windows10/10"` namespace
- **Add** `uap10` to `IgnorableNamespaces`
- **Add** `<uap10:AllowExternalContent>true</uap10:AllowExternalContent>` inside `<Properties>`
- **Replace** `<Application ... StartPage="index.html">` with:
  ```xml
  <Application
    Id="CamerasApp"
    Executable="cameras.exe"
    uap10:TrustLevel="mediumIL"
    uap10:RuntimeBehavior="win32App"
    EntryPoint="windows.none">
  ```
- **Add** `AppListEntry="none"` to `<uap:VisualElements>`
- **Add** the `<Extensions>` block with `com:Extension` for the COM server declaration:
  ```xml
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
  ```
- **Do NOT** include `runFullTrust` capability or `rescap` namespace
- **Remove** `rescap` namespace declaration if present

The full corrected manifest is in the design.md Decision 2.

### 2. Fix build-msix.ps1 version substitution

**File:** `scripts/build-msix.ps1`

The version substitution regex on line 75 is too greedy -- it replaces ALL `Version="X.Y.Z.W"` occurrences, including `MinVersion="10.0.22000.0"` in the `<TargetDeviceFamily>` element.

**Change line 75 from:**

```powershell
$manifestContent = $manifestContent -replace 'Version="[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+"', "Version=`"$MsixVersion`""
```

**To:**

```powershell
$manifestContent = $manifestContent -replace '(<Identity[^>]*\s)Version="[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+"', "`$1Version=`"$MsixVersion`""
```

This ensures only the `<Identity>` element's `Version` attribute is replaced.

### 3. Remove runtime MSIX registration from windows.rs

**File:** `src-tauri/src/virtual_camera/windows.rs`

Remove all runtime MSIX registration code. The sparse package is now registered at install time by the NSIS installer. Specific removals:

**Remove these imports (lines 9, 14-15):**

- `use std::sync::OnceLock;`
- `use windows::Foundation::Uri;`
- `use windows::Management::Deployment::{AddPackageOptions, PackageManager};`

**Remove these constants/statics (lines 26-29):**

- `const SPARSE_PACKAGE_NAME: &str = "io.takken.cameras";`
- `static SPARSE_REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();`

**Remove these methods/functions:**

- `ensure_sparse_package_registered()` method on `MfVirtualCamera` (lines 60-64)
- `find_msix_path()` function (lines 245-266)
- `is_sparse_package_registered()` function (lines 269-301)
- `register_sparse_package()` function (lines 306-375)

**Remove this test:**

- `find_msix_returns_error_when_missing` test (lines 419-429)

**Simplify `create_virtual_camera()` method (line 67-110):**
Remove the `self.ensure_sparse_package_registered()?;` call on line 72. The method should go straight to building the CLSID string and calling `MFCreateVirtualCamera`.

### 4. Remove WinRT features from Cargo.toml

**File:** `src-tauri/Cargo.toml`

Remove these three features from the `[target.'cfg(windows)'.dependencies.windows]` features list (lines 61-63):

- `"Foundation"`
- `"Management_Deployment"`
- `"ApplicationModel"`

Also remove `"Win32_System_Registry"` if no other code in the main app uses it (check for `RegCreateKeyExW`, `RegSetValueExW`, `RegCloseKey`, `RegOpenKeyExW` usage outside vcam code). The registry imports were only used by the now-removed `register_com_server()`.

### 5. Add NSIS post-install hook for sparse package registration

**File:** `src-tauri/nsis-hooks.nsh`

Add a `NSIS_HOOK_POSTINSTALL` macro that registers the sparse package. The manifest and DLL must already be in `$INSTDIR` at this point.

**Add before the existing `NSIS_HOOK_PREUNINSTALL` macro:**

```nsis
; Register the MSIX sparse package for COM redirection.
; The installer runs elevated, so Add-AppxPackage has the required permissions.
; The -ExternalLocation points to the install directory where vcam_source.dll lives.
!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToLog 'powershell -NoProfile -Command "Add-AppxPackage -Register -ExternalLocation \"$INSTDIR\" (Join-Path \"$INSTDIR\" \"AppxManifest.xml\")"'
!macroend
```

The existing `NSIS_HOOK_PREUNINSTALL` macro (already present) handles deregistration.

### 6. Bundle AppxManifest.xml and vcam_source.dll with NSIS installer

**File:** `src-tauri/build.rs`

Extend `build.rs` to copy files to the target directory so the NSIS installer picks them up:

1. **Copy `vcam_source.dll`** from the workspace target directory to the main app's target directory. The DLL is at `target/{profile}/vcam_source.dll` after `cargo build -p vcam-source`. Use the same `resolve_target_dir()` + `copy_sdk_file()` pattern as EDSDK.

2. **Copy `AppxManifest.xml`** from `src-tauri/msix/AppxManifest.xml` to the target directory. The NSIS installer needs this file alongside the exe so the post-install hook can register it.

Add a new function (e.g. `configure_vcam_source()`) called from `main()`:

```rust
fn configure_vcam_source() {
    let target_dir = resolve_target_dir();
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));

    // Copy vcam_source.dll from workspace target
    let dll_src = target_dir.join("vcam_source.dll");
    // DLL may not exist yet if vcam-source hasn't been built
    if dll_src.exists() {
        // Already in target dir, nothing to do
    }

    // Copy AppxManifest.xml to target dir for NSIS bundling
    let manifest_src = manifest_dir.join("msix").join("AppxManifest.xml");
    copy_sdk_file(&manifest_src, &target_dir.join("AppxManifest.xml"));

    println!("cargo:rerun-if-changed=msix/AppxManifest.xml");
}
```

### 7. Create exe.manifest with MSIX identity block

**File:** `src-tauri/cameras.exe.manifest` (new)

Create an application manifest that includes both the Common Controls v6 dependency (currently generated by Tauri) and the MSIX sparse package identity block:

```xml
<?xml version="1.0" encoding="utf-8"?>
<assembly manifestVersion="1.0" xmlns="urn:schemas-microsoft-com:asm.v1">
  <assemblyIdentity version="1.0.0.0" name="cameras.exe" />
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*" />
    </dependentAssembly>
  </dependency>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <msix xmlns="urn:schemas-microsoft-com:msix.v1"
            publisher="CN=Cameras App Dev"
            packageName="io.takken.cameras"
            applicationId="CamerasApp" />
    </application>
  </compatibility>
</assembly>
```

### 8. Embed custom exe.manifest in build.rs

**File:** `src-tauri/build.rs`

Embed the custom `cameras.exe.manifest` into the built exe, replacing Tauri's auto-generated manifest. Two approaches to investigate:

**Approach A: Override Tauri's resource.rc**
Tauri generates `resource.rc` with `1 24 { ... }` (manifest as inline data). If we provide our own `.rc` file with the manifest, it may conflict. Check if `tauri_build::build()` supports a custom manifest path.

**Approach B: Post-build manifest replacement**
Use `mt.exe` (from Windows SDK) as a post-build step to merge or replace the manifest:

```powershell
mt.exe -manifest cameras.exe.manifest -outputresource:cameras.exe;#1
```

This could be done in a cargo build script or as a separate step.

**Approach C: `embed-resource` crate**
Add `embed-resource` to `build-dependencies` and use it to compile a custom `.rc` file that includes the manifest. Call this BEFORE `tauri_build::build()` to take precedence.

The coding agent should investigate which approach works with Tauri v2's build pipeline and implement the simplest one that doesn't conflict with Tauri's own manifest generation.

### 9. Simplify vcam-test diagnostic

**File:** `src-tauri/crates/vcam-test/src/main.rs`

Remove the runtime MSIX registration capability since registration now happens at install time. The test tool should only CHECK whether the package is registered, not register it.

**Remove:**

- `--register-msix` flag and its handling (lines 42, 60-74)
- `register_sparse_package()` function (lines 228-271)
- `find_msix_path()` function (lines 274-299)
- `use windows::Foundation::Uri;` import
- `use windows::Management::Deployment::{AddPackageOptions, PackageManager};` import

**Keep:**

- `check_sparse_package_registered()` function -- still useful for diagnostics
- `check_msix_com_redirection()` function -- still useful for diagnostics
- `SPARSE_PACKAGE_NAME` constant -- used by the check function

**Update help text** to remove `--register-msix` option and add guidance:

```
Usage: vcam-test [OPTIONS]

Options:
  --help, -h  Show this help message

Note: The sparse package must be registered by the installer.
For dev testing, register manually:
  Add-AppxPackage -Register -ExternalLocation "path\to\target\debug" "path\to\msix\AppxManifest.xml"
```

**Update vcam-test Cargo.toml** (`src-tauri/crates/vcam-test/Cargo.toml`):
Remove WinRT features if they were added:

- `"Foundation"`
- `"Management_Deployment"`
- `"ApplicationModel"`

Keep `"Win32_System_Registry"` since `check_registry_key()` uses `RegOpenKeyExW`.

### 10. Update CI build workflow

**File:** `.github/workflows/build.yml`

After the Tauri build step (Windows only), add:

1. Build vcam-source: `cargo build -p vcam-source --release`
2. Run `scripts/build-msix.ps1` to generate the sparse MSIX package
3. Include the `.msix` in release artefacts alongside the NSIS installer

### 11. Gitignore MSIX artefacts

**File:** `.gitignore`

Add:

```
src-tauri/msix/dev-cert.pfx
src-tauri/msix/*.msix
src-tauri/msix/staging/
```
