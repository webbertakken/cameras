# Ambiguities log — Section 9: Settings persistence

Decisions and ambiguities encountered during implementation, for post-review.

---

## DummyBackend activation

**Ambiguity:** The brief mentioned "CLI flag or env var". Which should be used?
**Decision:** Used `DUMMY_CAMERA=1` environment variable only (no CLI flag).
**Rationale:** Env var is simpler to implement, doesn't require clap or argument parsing, and works well for both development (`DUMMY_CAMERA=1 cargo tauri dev`) and CI. CLI flag can be added later if needed.

## DummyBackend test JPEG frame

**Ambiguity:** The brief said "1x1 pixel or small solid colour JPEG". Should it be a real valid JPEG or just bytes with JPEG markers?
**Decision:** Used a pre-built minimal valid JFIF JPEG byte array with proper markers (SOI, JFIF APP0, DQT, SOF, DHT, SOS, EOI).
**Rationale:** A structurally valid JPEG ensures the `test_frame_is_valid_jpeg` test is meaningful and the frame can be processed by real JPEG decoders if needed for integration testing.

## Camera name lookup in set_camera_control

**Ambiguity:** How to get the camera name when persisting a control change? The command receives `device_id` and `control_id` but not the camera name.
**Decision:** Look up the camera name via `enumerate_devices()` after the control is set. Falls back to empty string if lookup fails.
**Rationale:** The device list is cached in the backend, so this is a cheap operation. The name is needed for the settings file's human-readable camera identification.

## SettingsState initialisation order

**Ambiguity:** Should SettingsState be managed before or after CameraState in setup()?
**Decision:** CameraState is managed via `.manage()` before `setup()` runs. SettingsState is created in `setup()` after CameraState is available, using `app.path().app_data_dir()`.
**Rationale:** The settings path requires the Tauri app handle (for platform-specific data directory), which is only available inside `setup()`. CameraState doesn't need the app handle so it can be created earlier.

## Debounce notify race window (VibeCheck finding)

**Ambiguity:** The debounce loop has a gap between `save()` completing and `notified().await` re-registering. A notification during this gap is lost.
**Decision:** Accepted as low-risk for now; will fix before final PR by using `notified()` to create the future before the loop, or adding an `AtomicBool` dirty flag checked after each save.
**Rationale:** Worst case: a setting change isn't persisted until the next change triggers another notify. For a desktop app with frequent slider interactions, this is extremely unlikely to cause data loss. Fix is planned.

## enumerate_devices() called per set_camera_control (VibeCheck finding)

**Ambiguity:** Camera name lookup calls `enumerate_devices()` on every control change. WindowsBackend may not cache this.
**Decision:** Accepted for now. The correct fix is to pass the camera name from the frontend (it already knows it) or cache device names on first discovery. Will address before final PR.
**Rationale:** For slider dragging scenarios, this could be a performance concern on Windows. The debounce means the lookup only happens per change, not per drag event, but it should still be optimised.

## TempDir leak in commands.rs tests (VibeCheck finding)

**Ambiguity:** `temp_store()` uses `Box::leak` to keep TempDir alive, causing a memory leak per test.
**Decision:** Will fix before final PR — return TempDir alongside the store (matching the pattern in `store.rs` tests).
**Rationale:** Test-only issue, not affecting production code, but should be fixed for test hygiene.

## No explicit "save now" command (VibeCheck finding)

**Ambiguity:** Settings only persist via debounce. If the app crashes within 500ms of a change, that change is lost.
**Decision:** Accepted by design. The 500ms crash window is acceptable for a desktop settings app.
**Rationale:** Adding a flush-on-exit hook would require wiring into Tauri's shutdown lifecycle. The debounce approach is simple and sufficient — users won't notice a single lost slider position on crash. Can revisit if needed.
