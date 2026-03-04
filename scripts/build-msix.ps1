<#
.SYNOPSIS
    Builds a sparse MSIX package for virtual camera COM redirection.

.DESCRIPTION
    Stages AppxManifest.xml, vcam_source.dll, and the app icon into a
    staging directory, then runs MakeAppx.exe pack to create the .msix.
    Optionally signs the package with SignTool.exe.

.PARAMETER Dev
    Use the dev certificate (src-tauri/msix/dev-cert.pfx) for signing.

.PARAMETER CertPath
    Path to a PFX certificate for signing (production builds).

.PARAMETER CertPassword
    Password for the PFX certificate.

.PARAMETER SkipSign
    Skip signing the MSIX package.

.EXAMPLE
    .\build-msix.ps1 -Dev
    .\build-msix.ps1 -CertPath .\cert.pfx -CertPassword "secret"
#>

param(
    [switch]$Dev,
    [string]$CertPath,
    [string]$CertPassword,
    [switch]$SkipSign
)

$ErrorActionPreference = 'Stop'

$ProjectRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$TauriDir = Join-Path $ProjectRoot 'src-tauri'
$MsixDir = Join-Path $TauriDir 'msix'
$StagingDir = Join-Path $MsixDir 'staging'
$ManifestPath = Join-Path $MsixDir 'AppxManifest.xml'
$IconSource = Join-Path $TauriDir 'icons' 'icon.png'
$TargetDir = Join-Path $TauriDir 'target' 'release'
$DllSource = Join-Path $TargetDir 'vcam_source.dll'

# Read version from tauri.conf.json
$TauriConf = Get-Content (Join-Path $TauriDir 'tauri.conf.json') -Raw | ConvertFrom-Json
$Version = $TauriConf.version
$MsixVersion = "$Version.0"  # MSIX requires 4-part version (X.Y.Z.0)

Write-Host "Building sparse MSIX package v$MsixVersion..."

# Validate inputs
if (-not (Test-Path $ManifestPath)) {
    Write-Error "AppxManifest.xml not found at: $ManifestPath"
}
if (-not (Test-Path $DllSource)) {
    Write-Error "vcam_source.dll not found at: $DllSource. Build with: cargo build -p vcam-source --release"
}
if (-not (Test-Path $IconSource)) {
    Write-Error "Icon not found at: $IconSource"
}

# Clean and create staging directory
if (Test-Path $StagingDir) {
    Remove-Item -Recurse -Force $StagingDir
}
New-Item -ItemType Directory -Path $StagingDir -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $StagingDir 'icons') -Force | Out-Null

# Stage files
Write-Host 'Staging files...'

# Copy and patch manifest with current version
$manifestContent = Get-Content $ManifestPath -Raw
$manifestContent = $manifestContent -replace '(<Identity[^>]*\s)Version="[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+"', "`$1Version=`"$MsixVersion`""
Set-Content -Path (Join-Path $StagingDir 'AppxManifest.xml') -Value $manifestContent -Encoding UTF8

Copy-Item $DllSource (Join-Path $StagingDir 'vcam_source.dll')
Copy-Item $IconSource (Join-Path $StagingDir 'icons' 'icon.png')

Write-Host "Staged: AppxManifest.xml, vcam_source.dll, icons/icon.png"

# Find MakeAppx.exe from Windows SDK
$sdkPaths = @(
    "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\MakeAppx.exe"
    "$env:ProgramFiles\Windows Kits\10\bin\*\x64\MakeAppx.exe"
)
$makeAppx = $sdkPaths | ForEach-Object { Get-Item $_ -ErrorAction SilentlyContinue } |
    Sort-Object { $_.Directory.Parent.Name } -Descending |
    Select-Object -First 1

if (-not $makeAppx) {
    Write-Error 'MakeAppx.exe not found. Install the Windows SDK.'
}

Write-Host "Using MakeAppx: $($makeAppx.FullName)"

# Build MSIX
$OutputMsix = Join-Path $MsixDir "cameras-vcam-$Version.msix"
if (Test-Path $OutputMsix) {
    Remove-Item $OutputMsix -Force
}

& $makeAppx.FullName pack /d $StagingDir /p $OutputMsix /o
if ($LASTEXITCODE -ne 0) {
    Write-Error "MakeAppx.exe pack failed with exit code $LASTEXITCODE"
}

Write-Host "Created: $OutputMsix"

# Sign the package
if ($SkipSign) {
    Write-Host 'Skipping signing (--SkipSign specified).'
} else {
    # Resolve certificate path
    if ($Dev) {
        $CertPath = Join-Path $MsixDir 'dev-cert.pfx'
        $CertPassword = 'cameras-dev'
    }

    if (-not $CertPath -or -not (Test-Path $CertPath)) {
        Write-Warning "No certificate found. Use -Dev for dev cert or -CertPath for production. Package is unsigned."
    } else {
        # Find SignTool.exe
        $signToolPaths = @(
            "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe"
            "$env:ProgramFiles\Windows Kits\10\bin\*\x64\signtool.exe"
        )
        $signTool = $signToolPaths | ForEach-Object { Get-Item $_ -ErrorAction SilentlyContinue } |
            Sort-Object { $_.Directory.Parent.Name } -Descending |
            Select-Object -First 1

        if (-not $signTool) {
            Write-Error 'SignTool.exe not found. Install the Windows SDK.'
        }

        Write-Host "Signing with: $($signTool.FullName)"

        $signArgs = @(
            'sign'
            '/fd', 'SHA256'
            '/a'
            '/f', $CertPath
        )
        if ($CertPassword) {
            $signArgs += @('/p', $CertPassword)
        }
        $signArgs += @('/t', 'http://timestamp.digicert.com')
        $signArgs += $OutputMsix

        & $signTool.FullName @signArgs
        if ($LASTEXITCODE -ne 0) {
            Write-Error "SignTool.exe sign failed with exit code $LASTEXITCODE"
        }

        Write-Host 'Package signed successfully.'
    }
}

# Clean up staging
Remove-Item -Recurse -Force $StagingDir
Write-Host "Cleaned up staging directory."

Write-Host "Done. MSIX package: $OutputMsix"
