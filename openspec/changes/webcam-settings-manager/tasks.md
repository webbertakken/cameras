## Phase 1 — Foundation

## 1. Project scaffold

- [x] 1.1 Initialise Tauri v2 project with React + TypeScript frontend (Vite)
- [x] 1.2 Configure Volta for Node/Yarn versions, add `.npmrc` and engine constraints
- [x] 1.3 Set up ESLint, Prettier, and Rust clippy/rustfmt configurations
- [x] 1.4 Add pre-commit hooks (lint, format, typecheck) via Husky + lint-staged
- [x] 1.5 Set up CI workflows (GitHub Actions) — modelled on webbertakken/snap:
  - [x] 1.5.1 Create `checks.yml` — runs on push to main + PRs: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo check`, `cargo test`, `yarn lint`, `yarn typecheck`, `yarn test`
  - [x] 1.5.2 Create `build.yml` — cross-platform build matrix (5 targets: Windows x64, Linux x64, Linux ARM, macOS ARM, macOS Intel) using `tauri build`. Steps: checkout, `dtolnay/rust-toolchain@stable`, `actions/setup-node` (Volta-pinned), `swatinem/rust-cache@v2`, install platform deps (Linux: libwebkit2gtk-4.1-dev, libjavascriptcoregtk-4.1-dev, libsoup-3.0-dev, libayatana-appindicator3-dev, libxcb, libgtk-3-dev, etc.), `yarn install`, `yarn tauri build`
  - [x] 1.5.3 Create `commit-lint.yml` — validate PR titles using `amannn/action-semantic-pull-request@v5` with conventional commit types (feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert)
  - [x] 1.5.4 Create `lint-workflows.yml` — validate workflow files on PRs touching `.github/workflows/**` using `reviewdog/action-actionlint@v1`
  - [x] 1.5.5 Create `release-please.yml` — automated release PRs using `googleapis/release-please-action@v4` with config for both Cargo.toml and package.json version bumping, conventional commit changelog generation, `RELEASE_TOKEN` secret
  - [x] 1.5.6 Create `release.yml` — build + publish releases using `tauri-apps/tauri-action@v0` triggered on version tags. Builds NSIS/MSI (Windows), DMG (macOS), AppImage/deb (Linux). Generates Tauri auto-updater JSON manifest. Uploads all artifacts to GitHub Release.
  - [x] 1.5.7 Add `release-please-config.json` and `.release-please-manifest.json` to repo root
  - [x] 1.5.8 Document required repository secrets: `RELEASE_TOKEN`, `TAURI_SIGNING_PRIVATE_KEY` (auto-updater), and placeholder notes for future code signing (`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`, `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD`)
- [x] 1.6 Create domain-driven directory structure in `src-tauri/src/` (camera, preview, presets, colour, overlays, virtual_cam, integrations, hotkeys, diagnostics, rtsp, settings)
- [x] 1.7 Create domain-driven directory structure in `src/features/` (camera-sidebar, preview, controls, presets, colour-grading, overlays, settings, widget)
- [x] 1.8 Add Tauri v2 permissions for system tray, window management, and IPC commands

## 2. Camera discovery — Rust backend

- [x] 2.1 Define `CameraBackend` trait with `enumerate_devices`, `watch_hotplug`, `open_device`, `get_controls`, `get_control`, `set_control`, `get_formats`, `start_capture`
- [x] 2.2 Define core types: `CameraDevice`, `DeviceId`, `CameraHandle`, `ControlDescriptor`, `ControlValue`, `ControlId`, `FormatDescriptor`, `HotplugEvent`
- [x] 2.3 Implement `WindowsBackend` for camera device enumeration using DirectShow/Media Foundation
- [x] 2.4 Implement hot-plug detection on Windows (device notification listener)
- [x] 2.5 Implement camera capability enumeration — query supported UVC controls with min/max/step/default for each
- [x] 2.6 Implement camera identification persistence — generate stable identifiers from VID:PID + serial number
- [x] 2.7 Expose Tauri IPC commands: `list_cameras`, `watch_cameras`, `get_camera_controls`, `get_camera_formats`

## 3. Camera discovery — Frontend sidebar

- [x] 3.1 Create `CameraSidebar` component with camera list layout
- [x] 3.2 Implement camera device state management (React context or Zustand store) synced with Tauri backend events
- [x] 3.3 Render camera entries with model name and placeholder thumbnails
- [x] 3.4 Implement camera selection — click to select, highlight active, update main panel
- [x] 3.5 Handle empty state — "no cameras found" message when no devices detected
- [x] 3.6 Handle hot-plug events — add/remove cameras from sidebar in real time with toast notifications

## 4. Camera frame capture and preview

- [x] 4.1 Implement frame capture pipeline in Rust — open camera, capture frames into a ring buffer
- [x] 4.2 Implement JPEG compression for frame delivery (main preview at quality 85, thumbnails at reduced resolution)
- [x] 4.3 Expose Tauri IPC command: `start_preview`, `stop_preview`, `get_frame`
- [x] 4.4 Create `PreviewCanvas` component — render JPEG frames via Canvas2D or `<img>` with blob URLs
- [x] 4.5 Implement sidebar thumbnail previews — low-resolution live thumbnails for each camera (5-10 fps)
- [x] 4.6 Implement diagnostic stats collection in Rust — actual fps, drop count, latency, bandwidth, USB bus info
- [x] 4.7 Create toggleable diagnostic overlay on the preview displaying all collected stats in real time
- [x] 4.8 Benchmark frame delivery latency and optimise if > 100ms

## 5. Basic camera controls — Rust backend

- [x] 5.1 Implement `get_control` and `set_control` for UVC controls on Windows (brightness, contrast, saturation, etc.)
- [x] 5.2 Expose Tauri IPC commands: `set_camera_control`, `reset_camera_control`
- [x] 5.3 Implement error handling — return descriptive errors when hardware rejects a control value

## 6. Basic camera controls — Frontend

- [x] 6.1 Create `ControlRenderer` component that maps `ControlDescriptor` to appropriate UI widget (slider, toggle, select)
- [x] 6.2 Create `ControlSlider` component with numeric display, direct numeric input, and reset-to-default button
- [x] 6.3 Create `ControlToggle` and `ControlSelect` components
- [x] 6.4 Implement greyed-out disabled state for unsupported controls with tooltip ("Not supported by [Camera Name]")
- [x] 6.5 Implement accordion section grouping based on `ControlDescriptor.group` field
- [x] 6.6 Wire controls to Tauri IPC — real-time slider changes sent to backend without debounce
- [x] 6.7 Handle control value rejection — revert slider to last valid value with inline error
- [x] 6.8 Ensure all controls meet WCAG 2.2 AA — keyboard nav, ARIA labels, contrast ratios, visible focus indicators

## 7. App shell — System tray and window management

- [x] 7.1 Configure Tauri system tray with icon, context menu (active camera, open panel, quit)
- [x] 7.2 Implement minimise-to-tray behaviour (window close minimises, tray click restores)
- [x] 7.3 Implement OS theme detection and following (light/dark) in the React frontend
- [x] 7.4 Set up base CSS/design tokens for the app's visual design (colours, spacing, typography)

## 8. Visual regression testing

- [x] 8.1 Install dependencies: `@vitest/browser`, `vitest-browser-react`, `playwright`
- [x] 8.2 Create `vitest.workspace.ts` separating unit tests (jsdom) from visual tests (browser/playwright)
- [x] 8.3 Move existing `test` config from `vite.config.ts` into workspace, add `test:visual` script to `package.json`
- [x] 8.4 Create test helper for rendering components in browser mode with app styles loaded
- [x] 8.5 Write visual test: empty state (sidebar + placeholder)
- [x] 8.6 Write visual test: camera sidebar with mocked devices
- [x] 8.7 Write visual test: controls panel (sliders, toggles, accordion sections)
- [x] 8.8 Write visual test: toast notifications (all types)
- [x] 8.9 Write visual test: disabled control state
- [x] 8.10 Generate and commit initial baselines on Linux/Chromium
- [x] 8.11 Add visual regression job to `checks.yml` CI workflow
- [x] 8.12 Update pre-commit hooks lint-staged config if needed (exclude `__screenshots__` from formatting)

---

## Phase 2 — Persistence and presets

## 9. Settings persistence

- [ ] 9.1 Create settings storage module in Rust — read/write JSON files in app data directory
- [ ] 9.2 Implement per-camera settings persistence — save all control values keyed by camera stable ID
- [ ] 9.3 Implement auto-save — persist settings on every control change (debounced write)
- [ ] 9.4 Implement auto-apply — on camera detection (launch or hot-plug), apply saved settings to hardware
- [ ] 9.5 Expose Tauri IPC commands: `reset_to_defaults` (reset all controls to hardware defaults)
- [ ] 9.6 Add toast notifications for settings restored/applied events

## 10. Preset system

- [ ] 10.1 Implement preset storage — save/load named presets as JSON files in `presets/<camera-id>/` directory
- [ ] 10.2 Implement built-in starter presets ("Warm", "Cool", "High Contrast", "Natural", "Low Light")
- [ ] 10.3 Implement preset import/export — export as JSON file, import with conflict resolution (rename/overwrite)
- [ ] 10.4 Create `PresetManager` component — list, load, save, rename, delete presets
- [ ] 10.5 Handle preset loading with unsupported controls — skip unsupported, apply supported, show toast listing skipped
- [ ] 10.6 Implement per-camera preset scoping — each camera sees only its own presets plus built-ins

---

## Phase 3 — Advanced controls

## 11. Control modes (auto/semi-auto/manual)

- [ ] 11.1 Implement auto/semi-auto/manual mode switching for white balance in Rust backend
- [ ] 11.2 Implement auto/semi-auto/manual mode switching for exposure in Rust backend
- [ ] 11.3 Implement "auto-set once" lock — read current auto value and switch to manual with that value
- [ ] 11.4 Create `ControlModeSelector` component — three-way toggle (Auto/Semi/Manual) for WB and exposure
- [ ] 11.5 Update `ControlSlider` to respect mode state — disabled in auto, range-constrained in semi-auto, full in manual

## 12. Zoom and focus

- [ ] 12.1 Implement hardware zoom control read/write in Rust backend
- [ ] 12.2 Implement digital zoom in the frame pipeline — crop and upscale in Rust before frame delivery
- [ ] 12.3 Implement hardware focus control with auto-focus toggle in Rust backend
- [ ] 12.4 Create zoom controls UI — separate hardware zoom slider and digital zoom slider with combined level display
- [ ] 12.5 Implement click-to-focus — capture click coordinates on preview, translate to camera ROI, send to backend
- [ ] 12.6 Add visual indicator on preview for click-to-focus point (animated ring)

## 13. Resolution, frame rate, and format selection

- [ ] 13.1 Implement format enumeration in Rust — list all supported resolution/fps/format combinations
- [ ] 13.2 Expose Tauri IPC command: `set_camera_format` with resolution, fps, and pixel format
- [ ] 13.3 Create cascading dropdowns UI — resolution dropdown updates available fps, fps updates available formats
- [ ] 13.4 Handle format change — restart capture pipeline with new format, update preview

## 14. Split before/after preview

- [ ] 14.1 Implement snapshot capture — freeze a frame as the "before" reference
- [ ] 14.2 Create split preview UI — side-by-side "Before" (static) and "After" (live) panes
- [ ] 14.3 Add toggle to enter/exit comparison mode
- [ ] 14.4 Ensure comparison mode exits cleanly when camera changes or disconnects

---

## Phase 4 — Colour, overlays, and virtual camera

## 15. Software colour grading pipeline

- [ ] 15.1 Integrate `wgpu` into the Rust backend for GPU compute
- [ ] 15.2 Implement colour correction compute shader — temperature, tint, RGB channel adjustments
- [ ] 15.3 Implement LUT application compute shader — sample 3D LUT texture for colour grading
- [ ] 15.4 Implement CPU fallback path for systems without GPU support (using `image` crate)
- [ ] 15.5 Define colour pipeline ordering: hardware controls → software colour correction → LUT → overlays
- [ ] 15.6 Expose Tauri IPC commands: `set_colour_correction`, `load_lut`, `unload_lut`, `toggle_colour_bypass`
- [ ] 15.7 Parse .cube LUT files and load as 3D textures

## 16. Colour grading UI

- [ ] 16.1 Create colour correction controls — temperature, tint, and RGB channel sliders
- [ ] 16.2 Create LUT manager — import .cube files, list loaded LUTs, select/deselect active LUT
- [ ] 16.3 Create colour bypass toggle — single button to enable/disable entire software colour pipeline
- [ ] 16.4 Add colour grading accordion section to the settings panel

## 17. Feed overlay system

- [ ] 17.1 Implement overlay compositing in the wgpu pipeline — render overlays after LUT, before output
- [ ] 17.2 Implement text overlay rendering — configurable content, font, size, colour, position, opacity
- [ ] 17.3 Implement image overlay rendering — PNG/JPEG import with position, scale, opacity, alpha support
- [ ] 17.4 Implement border overlay — configurable colour, thickness, and style
- [ ] 17.5 Implement watermark mode — tiled text or image with configurable opacity and angle
- [ ] 17.6 Implement overlay layer management — multiple overlays with reorderable z-stack and per-layer visibility toggle
- [ ] 17.7 Expose Tauri IPC commands: `add_overlay`, `update_overlay`, `remove_overlay`, `reorder_overlays`
- [ ] 17.8 Create overlay manager UI — add/edit/remove overlays, drag-to-reorder layers, preview in real time
- [ ] 17.9 Implement overlay persistence per camera — save/restore overlay configs alongside camera settings

## 18. Virtual camera output

- [ ] 18.1 Define `VirtualCameraOutput` trait in Rust with `start`, `stop`, `write_frame` methods
- [ ] 18.2 Implement Windows virtual camera output (DirectShow source filter or OBS VirtualCam SDK)
- [ ] 18.3 Wire virtual camera to receive processed frames from the full pipeline (colour + overlays)
- [ ] 18.4 Expose Tauri IPC commands: `start_virtual_camera`, `stop_virtual_camera`, `get_virtual_camera_status`
- [ ] 18.5 Create virtual camera toggle in the UI with status indicator
- [ ] 18.6 Set virtual camera device name to "Cameras - [Camera Name]"

---

## Phase 5 — Integration and polish

## 19. Global hotkeys

- [ ] 19.1 Implement global hotkey registration in Rust (platform-native APIs)
- [ ] 19.2 Expose Tauri IPC commands: `register_hotkey`, `unregister_hotkey`, `list_hotkeys`
- [ ] 19.3 Create hotkey settings UI — record key combinations, assign to actions, detect conflicts
- [ ] 19.4 Implement hotkey actions: toggle auto-exposure, toggle auto-WB, switch preset, toggle virtual camera
- [ ] 19.5 Add toast notifications for hotkey-triggered actions

## 20. Stream Deck integration

- [ ] 20.1 Create Stream Deck plugin project (Node.js SDK)
- [ ] 20.2 Implement plugin actions: load preset, toggle auto-exposure, toggle virtual camera
- [ ] 20.3 Implement bidirectional status sync — button icons reflect current camera state
- [ ] 20.4 Implement communication channel between Stream Deck plugin and Tauri app (local WebSocket or named pipe)

## 21. MIDI controller support

- [ ] 21.1 Integrate MIDI input library in Rust (`midir` crate)
- [ ] 21.2 Implement MIDI CC mapping to camera control sliders
- [ ] 21.3 Implement MIDI note mapping to toggle actions and preset loading
- [ ] 21.4 Create MIDI mapping UI — learn mode (turn a knob to assign), list mappings, delete mappings
- [ ] 21.5 Persist MIDI and hotkey mappings to settings file

## 22. OBS integration

- [ ] 22.1 Implement obs-websocket v5 client in Rust (`tokio-tungstenite`)
- [ ] 22.2 Implement OBS connection management — connect, disconnect, auto-reconnect, save credentials
- [ ] 22.3 Implement scene/source listing — read OBS video capture sources
- [ ] 22.4 Implement camera source coordination — update OBS source to match selected camera (with user confirmation)
- [ ] 22.5 Create OBS integration settings UI — connection config, status indicator, source list

## 23. Floating widget

- [ ] 23.1 Create floating widget Tauri window — small, always-on-top, minimal controls
- [ ] 23.2 Render most-used controls in widget: exposure mode, white balance mode, active preset, virtual camera toggle
- [ ] 23.3 Implement expand button — opens full panel and closes widget
- [ ] 23.4 Add widget activation via tray menu and global hotkey

## 24. RTSP network camera support

- [ ] 24.1 Integrate `ffmpeg-next` crate for RTSP stream decoding
- [ ] 24.2 Implement RTSP client module — connect to URL, decode frames, produce frames in standard pipeline format
- [ ] 24.3 Create "Add network camera" UI — RTSP URL input, connection test, optional friendly name
- [ ] 24.4 Persist RTSP camera configs and show them in sidebar alongside local cameras
- [ ] 24.5 Handle RTSP connection state — show "Offline" indicator when stream drops, auto-reconnect in background
- [ ] 24.6 Expose only software-side controls for RTSP cameras (colour grading, digital zoom, overlays)

## 25. Auto-updater and accessibility

- [ ] 25.1 Configure Tauri auto-updater with GitHub releases endpoint
- [ ] 25.2 Implement update check on launch with non-intrusive notification
- [ ] 25.3 Implement download and install flow within the app
- [ ] 25.4 Add code signing to release workflow — Apple notarisation (certificate + signing identity + Apple ID secrets) and Windows Authenticode signing (certificate + password secrets) integrated into the `release.yml` Tauri action build
- [ ] 25.5 Run full WCAG 2.2 AA accessibility audit — keyboard nav, screen reader, contrast, focus indicators, reduced motion
- [ ] 25.6 Implement configurable auto-start on login (minimised to tray) via OS-native mechanisms

---

## Phase 6 — Cross-platform

## 26. macOS camera backend

- [ ] 26.1 Implement `MacosBackend` — camera enumeration via AVFoundation
- [ ] 26.2 Implement hot-plug detection on macOS
- [ ] 26.3 Implement UVC control read/write on macOS
- [ ] 26.4 Implement frame capture pipeline on macOS
- [ ] 26.5 Implement virtual camera output on macOS (CoreMediaIO DAL plugin)
- [ ] 26.6 Test with multiple camera models on macOS (built-in FaceTime cam, USB webcams)

## 27. Linux camera backend

- [ ] 27.1 Implement `LinuxBackend` — camera enumeration via V4L2
- [ ] 27.2 Implement hot-plug detection on Linux (udev)
- [ ] 27.3 Implement UVC control read/write on Linux via V4L2 ioctl
- [ ] 27.4 Implement frame capture pipeline on Linux
- [ ] 27.5 Implement virtual camera output on Linux (v4l2loopback)
- [ ] 27.6 Test with multiple camera models on Linux

## 28. Cross-platform polish

- [ ] 28.1 Verify system tray integration on all platforms (Windows notification area, macOS menu bar, Linux desktop tray)
- [ ] 28.2 Verify OS theme following on all platforms
- [ ] 28.3 Verify auto-start registration on all platforms
- [ ] 28.4 Verify hotkey registration on all platforms
- [ ] 28.5 Verify RTSP camera support on all platforms
- [ ] 28.6 Performance testing — preview latency, CPU usage, memory footprint on each platform
