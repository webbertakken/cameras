; NSIS installer hooks for Cameras app.
; Referenced from tauri.conf.json via bundle.windows.nsis.installerHooks.

; Virtual camera CLSID — must match vcam-shared VCAM_SOURCE_CLSID.
!define VCAM_CLSID "{7B2E3A1F-4D5C-4E8B-9A6F-1C2D3E4F5A6B}"

; Device interface category GUIDs for OBS / DirectShow compatibility.
; MFCreateVirtualCamera only takes one category (KSCATEGORY_VIDEO_CAMERA),
; so we register additional categories at install time so FrameServer's
; DirectShow compatibility bridge exposes the device to OBS and other DS apps.
!define KSCATEGORY_VIDEO "{6994AD04-93E7-11D0-A3CC-00A0C9223196}"
!define KSCATEGORY_CAPTURE "{65E8773D-8F56-11D0-A3B9-00A0C9223196}"

; Register the MSIX sparse package, HKLM COM entry, and device interface
; categories after install.
; FrameServer (LOCAL SERVICE) needs HKLM to resolve the CLSID because
; per-user MSIX COM redirection is not visible to system services.
!macro NSIS_HOOK_POSTINSTALL
  ; MSIX sparse package registration — use -Register with manifest path
  ; (not a .msix package), matching the dev script pattern.
  nsExec::ExecToLog 'powershell -ExecutionPolicy Bypass -Command "Add-AppxPackage -Register (Join-Path \"$INSTDIR\" \"AppxManifest.xml\") -ExternalLocation \"$INSTDIR\""'

  ; HKLM COM registration for FrameServer
  WriteRegStr HKLM "Software\Classes\CLSID\${VCAM_CLSID}\InProcServer32" "" "$INSTDIR\vcam_source.dll"
  WriteRegStr HKLM "Software\Classes\CLSID\${VCAM_CLSID}\InProcServer32" "ThreadingModel" "Both"

  ; Register device interface categories so OBS (DirectShow) discovers the
  ; virtual camera. FrameServer's DS bridge checks these categories.
  ; KSCATEGORY_VIDEO
  WriteRegStr HKLM "Software\Classes\CLSID\${VCAM_CLSID}\Capabilities" "KSCATEGORY_VIDEO" "${KSCATEGORY_VIDEO}"
  ; KSCATEGORY_CAPTURE
  WriteRegStr HKLM "Software\Classes\CLSID\${VCAM_CLSID}\Capabilities" "KSCATEGORY_CAPTURE" "${KSCATEGORY_CAPTURE}"
!macroend

; Remove the MSIX sparse package, HKLM COM entry, and device interface
; categories before uninstalling files.
!macro NSIS_HOOK_PREUNINSTALL
  ; Remove HKLM COM registration (includes Capabilities subkey)
  DeleteRegKey HKLM "Software\Classes\CLSID\${VCAM_CLSID}"

  ; Remove MSIX sparse package
  nsExec::ExecToLog 'powershell -NoProfile -Command "Get-AppxPackage -Name io.takken.cameras | Remove-AppxPackage"'
!macroend
