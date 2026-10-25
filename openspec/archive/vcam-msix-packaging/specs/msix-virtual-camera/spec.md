## ADDED requirements

### Requirement: AppxManifest with COM class declaration

The build system SHALL include an `AppxManifest.xml` declaring the vcam-source CLSID (`7B2E3A1F-4D5C-4E8B-9A6F-1C2D3E4F5A6B`) as a `com:InProcessServer` with `ThreadingModel="Both"`. The manifest SHALL include `DeviceCapability Name="webcam"`, target `Windows.Desktop` min version `10.0.22000.0`, and use `uap10:AllowExternalContent` to mark it as a sparse package.

#### Scenario: Manifest declares correct CLSID

- **GIVEN** the `AppxManifest.xml` in `src-tauri/msix/`
- **WHEN** the manifest is parsed
- **THEN** it SHALL contain a `com:Class` element with `Id="7B2E3A1F-4D5C-4E8B-9A6F-1C2D3E4F5A6B"`
- **AND** the `Path` attribute SHALL be `vcam_source.dll`
- **AND** `ThreadingModel` SHALL be `"Both"`

#### Scenario: Manifest is a valid sparse package

- **GIVEN** the `AppxManifest.xml`
- **WHEN** the manifest is parsed
- **THEN** it SHALL contain `<uap10:AllowExternalContent>true</uap10:AllowExternalContent>` in `<Properties>`
- **AND** it SHALL use `EntryPoint="windows.none"` (not `Windows.FullTrustApplication`)
- **AND** it SHALL NOT declare `runFullTrust` capability

#### Scenario: Manifest version matches app version

- **GIVEN** the app version in `tauri.conf.json` is `X.Y.Z`
- **WHEN** the MSIX build script runs
- **THEN** the manifest Identity Version SHALL be `X.Y.Z.0`

### Requirement: Sparse MSIX package generation

The build pipeline SHALL produce a sparse MSIX package containing the `AppxManifest.xml`, `vcam_source.dll`, and the app icon. The package SHALL be created using `MakeAppx.exe pack` and optionally signed with `SignTool.exe`.

#### Scenario: Build script produces MSIX

- **GIVEN** `vcam_source.dll` exists in the cargo target directory
- **WHEN** `scripts/build-msix.ps1` is run
- **THEN** a `.msix` file SHALL be created in the msix output directory

#### Scenario: Dev build creates self-signed package

- **GIVEN** the `-Dev` flag is passed to `build-msix.ps1`
- **WHEN** the script runs
- **THEN** the package SHALL be signed with the dev certificate

#### Scenario: Production build signs the package

- **GIVEN** a signing certificate is available via `-CertPath`
- **WHEN** `build-msix.ps1` runs
- **THEN** the package SHALL be signed with `SignTool.exe`

#### Scenario: Version substitution only targets Identity element

- **GIVEN** the `AppxManifest.xml` contains `MinVersion="10.0.22000.0"`
- **WHEN** the build script substitutes the app version
- **THEN** only the `<Identity>` element's `Version` attribute SHALL be changed
- **AND** `MinVersion` SHALL remain `"10.0.22000.0"`

### Requirement: Install-time sparse package registration

The NSIS installer SHALL register the sparse MSIX package during installation via a post-install hook. The registration SHALL use `Add-AppxPackage -Register -ExternalLocation` pointing to the install directory. No runtime registration code SHALL exist in the main app.

#### Scenario: NSIS post-install registers the package

- **WHEN** the NSIS installer completes file installation
- **THEN** it SHALL execute `Add-AppxPackage -Register -ExternalLocation "$INSTDIR"` with the manifest path

#### Scenario: No admin required after install

- **GIVEN** the sparse package was registered during install
- **WHEN** the app starts as a standard user
- **THEN** the virtual camera feature SHALL work without elevation

#### Scenario: Registration failure during install is non-fatal

- **WHEN** `Add-AppxPackage` fails during install (e.g. policy restriction)
- **THEN** the NSIS installer SHALL complete without aborting
- **AND** the error SHALL be logged

### Requirement: NSIS uninstaller cleans up sparse package

The NSIS uninstaller SHALL remove the sparse MSIX package registration when the app is uninstalled. This prevents orphaned package registrations.

#### Scenario: Uninstall removes package

- **WHEN** the user uninstalls the app via the NSIS uninstaller
- **THEN** the sparse MSIX package SHALL be deregistered via `Remove-AppxPackage`

### Requirement: FrameServer discovers COM DLL via MSIX redirection

After sparse package registration, FrameServer SHALL be able to resolve the vcam-source CLSID and load `vcam_source.dll` without any HKCU or HKLM registry entries.

#### Scenario: Virtual camera starts successfully

- **GIVEN** the sparse package is registered
- **WHEN** `MFCreateVirtualCamera` is called with the vcam-source CLSID
- **AND** `IMFVirtualCamera::Start()` is called
- **THEN** FrameServer SHALL load `vcam_source.dll` via COM redirection
- **AND** the virtual camera SHALL appear in the system's device list

#### Scenario: No HKCU registration needed

- **GIVEN** the sparse package is registered
- **WHEN** the virtual camera is started
- **THEN** no writes to `HKCU\Software\Classes\CLSID` SHALL occur

### Requirement: Remove all runtime registration code

The `windows.rs` file SHALL contain no runtime MSIX registration code, no HKCU COM registration code, and no `AddRegistryEntry` calls. The CLSID resolution SHALL rely entirely on the install-time MSIX COM redirection.

#### Scenario: No registry writes during virtual camera start

- **WHEN** the virtual camera start flow executes
- **THEN** no `RegCreateKeyExW`, `RegSetValueExW`, or `AddRegistryEntry` calls SHALL be made

#### Scenario: No WinRT imports in main app

- **WHEN** the main app crate is compiled
- **THEN** it SHALL NOT import `windows::Foundation`, `windows::Management::Deployment`, or `windows::ApplicationModel`

### Requirement: vcam_source.dll ships with the app

The `vcam_source.dll` SHALL be copied to the same directory as `cameras.exe` during build. The `build.rs` script SHALL handle this copy step, matching the existing EDSDK DLL copy pattern.

#### Scenario: DLL present after cargo build

- **WHEN** `cargo build` completes for the main app
- **THEN** `vcam_source.dll` SHALL exist in the target output directory alongside `cameras.exe`

### Requirement: Exe.manifest with MSIX identity block

The built `cameras.exe` SHALL have an embedded application manifest containing an `<msix>` identity block in the `<compatibility>` section. The `publisher`, `packageName`, and `applicationId` values SHALL exactly match the `AppxManifest.xml`.

#### Scenario: Exe manifest contains MSIX identity

- **GIVEN** the built `cameras.exe`
- **WHEN** its embedded manifest is inspected
- **THEN** it SHALL contain `<msix publisher="CN=Cameras App Dev" packageName="io.takken.cameras" applicationId="CamerasApp" />`

## MODIFIED requirements

### Requirement: Virtual camera start flow (from sidebar-canon-vcam/specs/mf-virtual-camera)

The virtual camera start flow SHALL be:

1. Call `MFCreateVirtualCamera` with session lifetime and current-user access
2. Call `IMFVirtualCamera::Start()`
3. Start the frame pump thread

No registration steps of any kind (HKCU, MSIX, AddRegistryEntry) SHALL occur at runtime. The sparse package MUST already be registered at install time.
