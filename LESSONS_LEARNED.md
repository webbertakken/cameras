# Lessons learned

## Canon EVF resolution is model-dependent

- Actual output: 960×640 (crop), 1056×704 (full), 1920×1280 (newer EOS R bodies).
- EDSDK has no API to query EVF dimensions. Our Canon EOS 2000D sends 1056×704.
- Frame pump reads real dimensions from decoded JPEG — resize handles any input.

## FrameServer locks `vcam_source.dll`

- Must `Stop-Service FrameServer -Force` (elevated) before rebuilding the DLL.
- After rebuild: re-register (`register-vcam-dev.ps1`) then `Start-Service FrameServer`.

## Virtual camera requires dual registration

- MSIX sparse package = package identity (for `MFCreateVirtualCamera`).
- HKLM `InProcServer32` = COM visibility (for FrameServer/LOCAL SERVICE).
- Both are required. MSIX alone won't work — FrameServer can't see per-user MSIX COM redirection.
