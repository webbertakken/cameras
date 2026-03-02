# Design

## Decision 1: Hotplug window — top-level invisible window

Replace `HWND_MESSAGE` with `HWND::default()` (NULL parent = top-level desktop window) in `run_hotplug_loop`. The window remains invisible (0x0 size, no style). `RegisterDeviceNotificationW` delivers targeted `WM_DEVICECHANGE` to top-level windows but not message-only windows.

**Rationale:** Message-only windows don't receive broadcast messages including `WM_DEVICECHANGE`. A top-level invisible window receives all targeted device notifications without any UI footprint.

## Decision 2: Add WM_DEVICECHANGE debug logging

Log all `wparam` values in `hotplug_wnd_proc`, not just `DBT_DEVICEARRIVAL` (0x8000) and `DBT_DEVICEREMOVECOMPLETE` (0x8004). This aids future debugging. Consider also handling `DBT_DEVNODES_CHANGED` (0x0007) as a fallback trigger for re-enumeration.

## Decision 3: EDSDK build.rs linking via .proprietary/

Add conditional build.rs logic behind `#[cfg(feature = "canon")]`:

- Locate EDSDK.lib at `.proprietary/Canon/EDSDKv132010W/EDSDKv132010W/Windows/EDSDK_64/Library/`
- Emit `cargo:rustc-link-search=native=<path>` and `cargo:rustc-link-lib=dylib=EDSDK`
- Copy EDSDK.dll to `target/<profile>/` for runtime availability
- Print `cargo:warning=` if DLLs not found (don't fail — mock-only still works)

**Rationale:** `.proprietary/` is gitignored, so DLLs are local-only. Build.rs handles the linking transparently. Developers without DLLs can still build and test with mocks.

## Decision 4: EDSDK delay-loading

Use `/DELAYLOAD:EDSDK.dll` linker flag so the app starts without EDSDK.dll present. The DLL is loaded on first FFI call. This means:

- App works normally without Canon SDK installed
- Canon features activate only when DLL is available at runtime
- Missing DLL produces a clear error, not a crash

## Decision 5: CI canon feature — compile-check only

Add `cargo check --features canon` to CI. Since EDSDK DLLs aren't in CI, linking will fail — use `continue-on-error: true`. Mock-based tests (`cargo test --features canon`) should pass since they don't call real FFI. Add this as a separate job in `checks.yml`.

## Decision 6: GPU encoding — validate via stats IPC

The `get_encoding_stats` IPC command already exposes encoder kind, timing, and frame counts. Validation = start preview, confirm `encoder_kind` is `MfHardware` or `MfSoftware` (not `CpuFallback`), confirm encode times are lower than CPU baseline.

## Risks

- EDSDK delay-load requires MSVC linker flags — may need `build.rs` to emit `cargo:rustc-cdylib-link-arg=/DELAYLOAD:EDSDK.dll`
- Canon feature in CI may have false negatives if compile-check doesn't cover all code paths
- GPU encoding validation is manual (requires real camera hardware)
