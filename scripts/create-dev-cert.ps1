#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Creates a self-signed certificate for signed MSIX packages (optional).

.DESCRIPTION
    Creates a self-signed code signing certificate (CN=Cameras App Dev),
    exports it to src-tauri/msix/dev-cert.pfx, and installs it in the
    trusted root store. Skips if the certificate already exists.

    THIS SCRIPT IS OPTIONAL FOR DEVELOPMENT. Unsigned MSIX packages work
    fine for dev builds because the Rust code sets AllowUnsigned(true) in
    debug mode. Use `build-msix.ps1 -Dev -SkipSign` to produce an unsigned
    package without any certificate. Only run this script if you want to
    test signed packages locally.
#>

$ErrorActionPreference = 'Stop'

$CertSubject = 'CN=Cameras App Dev'
$PfxPath = Join-Path $PSScriptRoot '..\src-tauri\msix\dev-cert.pfx'
$PfxPath = [System.IO.Path]::GetFullPath($PfxPath)
$PfxPassword = ConvertTo-SecureString -String 'cameras-dev' -Force -AsPlainText

# Check if cert already exists in the personal store
$existing = Get-ChildItem -Path Cert:\CurrentUser\My |
    Where-Object { $_.Subject -eq $CertSubject -and $_.NotAfter -gt (Get-Date) }

if ($existing) {
    Write-Host "Certificate '$CertSubject' already exists (thumbprint: $($existing.Thumbprint)). Skipping creation."
} else {
    Write-Host "Creating self-signed certificate '$CertSubject'..."
    $cert = New-SelfSignedCertificate `
        -Type Custom `
        -Subject $CertSubject `
        -KeyUsage DigitalSignature `
        -CertStoreLocation 'Cert:\CurrentUser\My' `
        -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3') `
        -NotAfter (Get-Date).AddYears(3)

    $existing = $cert
    Write-Host "Created certificate with thumbprint: $($cert.Thumbprint)"
}

# Export to PFX
$pfxDir = Split-Path $PfxPath -Parent
if (-not (Test-Path $pfxDir)) {
    New-Item -ItemType Directory -Path $pfxDir -Force | Out-Null
}

Export-PfxCertificate -Cert "Cert:\CurrentUser\My\$($existing.Thumbprint)" `
    -FilePath $PfxPath `
    -Password $PfxPassword | Out-Null

Write-Host "Exported PFX to: $PfxPath"

# Install in trusted root (so MSIX validation passes)
$rootStore = Get-ChildItem -Path Cert:\LocalMachine\Root |
    Where-Object { $_.Thumbprint -eq $existing.Thumbprint }

if ($rootStore) {
    Write-Host 'Certificate already in trusted root store. Skipping install.'
} else {
    Write-Host 'Installing certificate in trusted root store...'
    $rootCert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($PfxPath, 'cameras-dev')
    $store = New-Object System.Security.Cryptography.X509Certificates.X509Store('Root', 'LocalMachine')
    $store.Open('ReadWrite')
    $store.Add($rootCert)
    $store.Close()
    Write-Host 'Certificate installed in trusted root store.'
}

Write-Host 'Done. Dev certificate is ready for MSIX signing.'
