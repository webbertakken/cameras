# Tasks

All tracked in chainlink. Run `chainlink list` for current status.

## 1. Hotplug fix

- **#57** Fix DirectShow hotplug detection for new USB cameras (parent)
  - **#60** Investigate hotplug root cause — DONE: HWND_MESSAGE doesn't receive WM_DEVICECHANGE
  - **#61** Implement fix: change window parent to HWND::default(), add logging

## 2. Canon EDSDK build wiring

- **#58** Canon EDSDK real DLL wiring and build configuration (parent)
  - **#62** Configure build.rs for EDSDK DLL linking
  - **#63** Wire real EDSDK FFI to sdk.rs wrapper (blocked by #62)
  - **#64** Update CI to handle canon feature flag

## 3. GPU encoding validation

- **#59** GPU encoding validation and PR #57 merge readiness (parent)
  - **#65** Validate MF JPEG encoder end-to-end on real hardware
