<#
.SYNOPSIS
    Registers the virtual camera MSIX sparse package for local development.

.DESCRIPTION
    Builds vcam_source.dll, stages the AppxManifest.xml and icon into
    the target directory, then registers the sparse package pointing at
    that directory. After this, MFCreateVirtualCamera can resolve the CLSID
    via COM redirection without needing an installed NSIS package.

    Run this once after cloning, or after changing AppxManifest.xml / the CLSID.

.PARAMETER Unregister
    Remove the registered sparse package instead of registering it.

.PARAMETER Release
    Use target/release instead of target/debug.

.EXAMPLE
    .\register-vcam-dev.ps1
    .\register-vcam-dev.ps1 -Release
    .\register-vcam-dev.ps1 -Unregister
#>

param(
    [switch]$Unregister,
    [switch]$Release
)

$ErrorActionPreference = 'Stop'

# HKLM writes require elevation
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
)
if (-not $isAdmin) {
    Write-Host 'Elevating to admin (HKLM registry writes require it)...'
    $argList = @('-ExecutionPolicy', 'Bypass', '-File', $PSCommandPath)
    if ($Unregister) { $argList += '-Unregister' }
    if ($Release) { $argList += '-Release' }
    Start-Process pwsh -ArgumentList $argList -Verb RunAs -Wait
    exit $LASTEXITCODE
}

$ProjectRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$TauriDir = Join-Path $ProjectRoot 'src-tauri'
$Profile = if ($Release) { 'release' } else { 'debug' }
$TargetDir = Join-Path $TauriDir 'target' $Profile
$MsixDir = Join-Path $TauriDir 'msix'
$ManifestSource = Join-Path $MsixDir 'AppxManifest.xml'
$IconSource = Join-Path $TauriDir 'icons' 'icon.png'
$PackageName = 'io.takken.cameras'
$Clsid = '{7B2E3A1F-4D5C-4E8B-9A6F-1C2D3E4F5A6B}'
$ClsidRegPath = "HKLM:\Software\Classes\CLSID\$Clsid"

if ($Unregister) {
    Write-Host 'Removing HKLM COM registration...'
    Remove-Item -Path $ClsidRegPath -Recurse -ErrorAction SilentlyContinue
    Write-Host "Removed: $ClsidRegPath"

    Write-Host 'Removing registered sparse package...'
    $pkg = Get-AppxPackage -Name $PackageName -ErrorAction SilentlyContinue
    if ($pkg) {
        Remove-AppxPackage -Package $pkg.PackageFullName
        Write-Host "Removed: $($pkg.PackageFullName)"
    } else {
        Write-Host 'No registered package found — nothing to remove.'
    }
    return
}

# Step 1: Build vcam_source.dll
$buildArgs = @('build', '-p', 'vcam-source')
if ($Release) { $buildArgs += '--release' }
Write-Host "Building vcam_source.dll ($Profile)..."
Push-Location $TauriDir
try {
    & cargo @buildArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Error 'cargo build -p vcam-source failed'
    }
} finally {
    Pop-Location
}

$DllPath = Join-Path $TargetDir 'vcam_source.dll'
if (-not (Test-Path $DllPath)) {
    Write-Error "vcam_source.dll not found at: $DllPath"
}
Write-Host "DLL: $DllPath"

# Step 2: Stage AppxManifest.xml and icon into target/debug
Write-Host 'Staging manifest and icon...'

Copy-Item $ManifestSource (Join-Path $TargetDir 'AppxManifest.xml') -Force

$IconDir = Join-Path $TargetDir 'icons'
if (-not (Test-Path $IconDir)) {
    New-Item -ItemType Directory -Path $IconDir -Force | Out-Null
}
Copy-Item $IconSource (Join-Path $IconDir 'icon.png') -Force

Write-Host "Staged: AppxManifest.xml, icons/icon.png -> $TargetDir"

# Step 3: Remove any existing registration (re-register cleanly)
$existing = Get-AppxPackage -Name $PackageName -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host 'Removing existing registration...'
    Remove-AppxPackage -Package $existing.PackageFullName
}

# Step 4: Register the sparse package
$ManifestPath = Join-Path $TargetDir 'AppxManifest.xml'
Write-Host "Registering sparse package..."
Write-Host "  Manifest: $ManifestPath"
Write-Host "  ExternalLocation: $TargetDir"

Add-AppxPackage -Register $ManifestPath -ExternalLocation $TargetDir

# Step 5: Register COM in HKLM so FrameServer (LOCAL SERVICE) can resolve the CLSID
Write-Host 'Registering COM in HKLM for FrameServer...'
$InProcPath = "$ClsidRegPath\InProcServer32"
New-Item -Path $InProcPath -Force | Out-Null
Set-ItemProperty -Path $InProcPath -Name '(Default)' -Value (Join-Path $TargetDir 'vcam_source.dll')
Set-ItemProperty -Path $InProcPath -Name 'ThreadingModel' -Value 'Both'
Write-Host "  $InProcPath -> $(Join-Path $TargetDir 'vcam_source.dll')"

# Step 6: Verify
$registered = Get-AppxPackage -Name $PackageName -ErrorAction SilentlyContinue
if ($registered) {
    Write-Host "`nRegistered successfully:"
    Write-Host "  Name: $($registered.Name)"
    Write-Host "  Version: $($registered.Version)"
    Write-Host "  InstallLocation: $($registered.InstallLocation)"
    Write-Host "  HKLM COM: $InProcPath"
    Write-Host "`nVirtual camera is ready for dev testing."
} else {
    Write-Error 'Registration failed — package not found after Add-AppxPackage.'
}
