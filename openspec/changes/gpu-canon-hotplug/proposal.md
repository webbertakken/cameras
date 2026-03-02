# GPU encoding, Canon EDSDK wiring, and hotplug fix

## Problem

Three independent issues blocking the next release:

1. **Hotplug detection broken** — new USB cameras connected after launch are not detected. Root cause: `HWND_MESSAGE` windows don't receive `WM_DEVICECHANGE` broadcasts.
2. **Canon EDSDK not linked** — mock-based Canon backend merged (PRs #52, #54) but real EDSDK DLLs (now in `.proprietary/Canon/`) are not wired into the build.
3. **GPU encoding unvalidated** — Media Foundation JPEG encoder and async encode worker on `feat/gpu-processing` (PR #57) need CI green and end-to-end validation.

## Scope

- Fix hotplug window parent (one-liner + logging)
- Configure build.rs for EDSDK DLL linking behind `canon` feature flag
- Wire real FFI in sdk.rs to EDSDK.dll
- Update CI for canon feature
- Validate GPU encoding pipeline and merge PR #57

## Out of scope

- Canon real-hardware testing (no Canon camera available in CI)
- GPU shader changes (WGPU code is already correct)
- Frontend changes (none needed)
