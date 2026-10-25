; NSIS installer hooks for Cameras app.
; Referenced from tauri.conf.json via bundle.windows.nsis.installerHooks.

; Virtual camera CLSID — must match vcam-shared VCAM_SOURCE_CLSID.
!define VCAM_CLSID "{7B2E3A1F-4D5C-4E8B-9A6F-1C2D3E4F5A6B}"

; Register the MSIX sparse package and HKLM COM entry after install.
; FrameServer (LOCAL SERVICE) needs HKLM to resolve the CLSID because
; per-user MSIX COM redirection is not visible to system services.
!macro NSIS_HOOK_POSTINSTALL
  ; MSIX sparse package registration — use -Register with manifest path
  ; (not a .msix package), matching the dev script pattern.
  nsExec::ExecToLog 'powershell -ExecutionPolicy Bypass -Command "Add-AppxPackage -Register (Join-Path \"$INSTDIR\" \"AppxManifest.xml\") -ExternalLocation \"$INSTDIR\""'

  ; HKLM COM registration for FrameServer
  WriteRegStr HKLM "Software\Classes\CLSID\${VCAM_CLSID}\InProcServer32" "" "$INSTDIR\vcam_source.dll"
  WriteRegStr HKLM "Software\Classes\CLSID\${VCAM_CLSID}\InProcServer32" "ThreadingModel" "Both"
!macroend

; Remove the MSIX sparse package and HKLM COM entry before uninstalling files.
!macro NSIS_HOOK_PREUNINSTALL
  ; Remove HKLM COM registration
  DeleteRegKey HKLM "Software\Classes\CLSID\${VCAM_CLSID}"

  ; Remove MSIX sparse package
  nsExec::ExecToLog 'powershell -NoProfile -Command "Get-AppxPackage -Name io.takken.cameras | Remove-AppxPackage"'
!macroend
