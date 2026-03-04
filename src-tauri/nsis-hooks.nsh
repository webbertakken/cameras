; NSIS installer hooks for Cameras app.
; Referenced from tauri.conf.json via bundle.windows.nsis.installerHooks.

; Register the MSIX sparse package after install so FrameServer can resolve
; the vcam-source CLSID via COM redirection.
!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToLog 'powershell -ExecutionPolicy Bypass -Command "Add-AppxPackage -Register -ExternalLocation \"$INSTDIR\" (Join-Path \"$INSTDIR\" \"cameras-vcam.msix\")"'
!macroend

; Remove the MSIX sparse package before uninstalling files.
; The sparse package provides COM redirection for the virtual camera DLL —
; it must be deregistered before the DLL is removed from disk.
!macro NSIS_HOOK_PREUNINSTALL
  nsExec::ExecToLog 'powershell -NoProfile -Command "Get-AppxPackage -Name io.takken.cameras | Remove-AppxPackage"'
!macroend
