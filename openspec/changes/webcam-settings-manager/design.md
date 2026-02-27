## Context

This is a greenfield desktop application for managing camera settings across Windows, macOS, and Linux. There is no existing codebase. The app targets streamers, content creators, and remote workers who need unified control over multiple cameras with professional-grade features (presets, colour grading, virtual camera output).

The technology stack is prescribed: **Tauri v2** (Rust backend) + **React** (TypeScript frontend). Tauri v2 provides cross-platform desktop packaging with native system tray, auto-updater, and IPC between Rust and the webview.

Key constraint: Camera hardware access (UVC controls, device enumeration, video frame capture) MUST happen in Rust via platform-native APIs. The React frontend handles only presentation and user interaction, receiving frames and control metadata via Tauri's IPC bridge.

## Goals / Non-Goals

**Goals:**

- Camera discovery and sidebar with live thumbnails as the primary UX (first-class feature)
- Dynamic UI that adapts to each camera's actual capabilities — no hardcoded control sets
- Real-time preview with < 100ms latency from camera to screen
- Full preset system with persistence, import/export, and per-camera libraries
- Software colour grading pipeline with LUT support applied before virtual camera output
- Cross-platform from day one (Windows, macOS, Linux) with platform abstraction in Rust
- System tray + floating widget for quick access without disrupting workflow
- Full feed overlay system (text, images, borders, watermarks) on the camera output
- Full diagnostic overlay with real-time feed stats
- RTSP/IP camera support alongside local cameras
- Full WCAG 2.2 AA accessibility compliance

**Non-Goals:**

- Per-app camera profiles (auto-switch based on foreground app) — deferred
- Audio/microphone controls — out of scope
- Recording or streaming capabilities — this is a control app, not a capture app
- Virtual backgrounds, blur, or filters in v1 — architecture for extensibility only
- Mobile or web support
- Custom themes beyond OS light/dark following

## Decisions

### Decision 1: Monorepo structure with domain-driven modules

The project uses a monorepo with Tauri's standard layout. The Rust backend is organised into domain modules rather than technical layers:

```
src-tauri/
  src/
    camera/          # Camera discovery, enumeration, platform abstraction
      discovery.rs   # Device detection + hot-plug events
      controls.rs    # UVC control read/write
      capture.rs     # Frame capture pipeline
      platform/      # Platform-specific implementations
        windows.rs   # DirectShow / Media Foundation
        macos.rs     # AVFoundation
        linux.rs     # V4L2
    preview/         # Frame processing + delivery to frontend
    presets/         # Preset save/load/import/export
    colour/          # Software colour pipeline + LUT processing
    overlays/        # Feed overlay compositing (text, images, borders, watermarks)
    virtual_cam/     # Virtual camera output device
    integrations/    # OBS websocket, Stream Deck, MIDI
    hotkeys/         # Global hotkey registration
    diagnostics/     # Feed diagnostic stats (fps, latency, bandwidth, dropped frames)
    rtsp/            # RTSP network camera client
    settings/        # Persistence layer (JSON config files)
src/                 # React frontend
  features/
    camera-sidebar/  # Left sidebar with thumbnails
    preview/         # Main preview + split comparison
    controls/        # Dynamic control rendering + accordion
    presets/         # Preset manager UI
    colour-grading/  # Colour correction + LUT UI
    settings/        # App settings (hotkeys, OBS config, auto-start)
    widget/          # Floating widget mini-view
```

**Rationale:** Domain-driven structure keeps related concerns together. Each Rust module owns its camera API surface. The platform abstraction lives inside the camera module, not as a separate cross-cutting layer.

### Decision 2: Platform camera abstraction via trait

Define a `CameraBackend` trait in Rust that each platform implements:

```rust
trait CameraBackend {
    fn enumerate_devices() -> Vec<CameraDevice>;
    fn watch_hotplug(callback: impl Fn(HotplugEvent));
    fn open_device(id: &DeviceId) -> Result<CameraHandle>;
    fn get_controls(handle: &CameraHandle) -> Vec<ControlDescriptor>;
    fn get_control(handle: &CameraHandle, id: ControlId) -> ControlValue;
    fn set_control(handle: &CameraHandle, id: ControlId, value: ControlValue) -> Result<()>;
    fn get_formats(handle: &CameraHandle) -> Vec<FormatDescriptor>;
    fn start_capture(handle: &CameraHandle, format: Format) -> FrameStream;
}
```

**Rationale:** This allows the frontend and core logic to be platform-agnostic. The trait is implemented by `WindowsBackend` (DirectShow/Media Foundation), `MacosBackend` (AVFoundation), and `LinuxBackend` (V4L2). The correct implementation is selected at compile time via `cfg(target_os)`.

**Alternatives considered:**

- Using a cross-platform library like `nokhwa`: Considered, but it doesn't expose the full range of UVC controls needed (pan/tilt, powerline freq, etc.) and limits our ability to handle non-standard cameras like GoPro/Canon.
- Abstracting at a higher level (per-feature, not per-operation): Too coarse — we need fine-grained control over individual UVC properties.

### Decision 3: Frame delivery via shared memory + events

Camera frames are captured in Rust and delivered to the React frontend via:

1. **Rust captures frames** into a shared buffer (ring buffer)
2. **Tauri event** notifies the frontend that a new frame is ready
3. **Frontend reads the frame** via a Tauri command that returns the frame data (or a reference to shared memory)
4. **WebGL or Canvas2D** renders the frame in the preview

For sidebar thumbnails, frames are downscaled in Rust before delivery (e.g. 160x120) to reduce IPC overhead.

**Rationale:** Direct IPC of full frames at 30fps 1080p (~6MB/frame uncompressed) is too expensive. Options:

- **SharedArrayBuffer** (not available in Tauri webview)
- **Base64-encoded frames via events**: High overhead from encoding. Acceptable for thumbnails, not main preview.
- **Write frames to a local file/pipe and read from frontend**: Complex, fragile.
- **Offscreen canvas in Rust + compositing**: Not available in Tauri.

The pragmatic approach: Send JPEG-compressed frames via Tauri commands for the main preview (JPEG at quality 85 for 1080p is ~100-200KB, manageable at 30fps). Thumbnails use lower resolution JPEG. If performance is insufficient, we can explore custom native rendering or Tauri v2's `WebviewWindow` raw rendering hooks.

### Decision 4: Dynamic control descriptors drive the UI

Camera controls are described by a `ControlDescriptor` struct sent from Rust to the frontend:

```typescript
interface ControlDescriptor {
  id: string // e.g. "brightness", "white_balance_temperature"
  name: string // Human-readable display name
  type: 'slider' | 'toggle' | 'select' | 'button'
  group: string // Accordion section: "image", "exposure", "focus", "advanced"
  min?: number
  max?: number
  step?: number
  default?: number
  current: number
  flags: {
    supportsAuto: boolean
    isAutoEnabled: boolean
    isReadOnly: boolean
  }
  options?: { value: number; label: string }[] // For 'select' type
}
```

The React frontend renders controls dynamically from this descriptor array — no hardcoded control components. A `ControlRenderer` component maps each descriptor to the appropriate UI widget.

**Rationale:** Cameras vary wildly in which controls they support. Hardcoding creates maintenance burden and breaks for unusual cameras. Dynamic rendering adapts automatically.

### Decision 5: Software colour pipeline in Rust using GPU compute (wgpu)

The colour grading pipeline (temperature, tint, RGB, LUT) runs as a GPU compute shader via `wgpu` in Rust:

1. Camera frame enters pipeline as a texture
2. Colour correction shader applies temperature/tint/RGB adjustments
3. LUT shader samples the 3D LUT texture
4. Output frame is read back for preview delivery and virtual camera output

**Rationale:** CPU-based pixel manipulation at 1080p 30fps is expensive. `wgpu` provides GPU acceleration that works across all platforms (Vulkan on Linux/Windows, Metal on macOS). The same processed frame feeds both the preview and the virtual camera output, avoiding duplicate processing.

**Alternative considered:** Processing in the frontend via WebGL shaders. This would work for preview but wouldn't help the virtual camera output (which lives in Rust). Having the pipeline in Rust ensures a single source of truth for the processed feed.

### Decision 6: Virtual camera via platform-specific drivers

Virtual camera output requires OS-level driver integration:

- **Windows:** Use the OBS Virtual Camera SDK or implement a DirectShow source filter
- **macOS:** Use a CoreMediaIO DAL plugin (similar to OBS Virtual Camera)
- **Linux:** Use v4l2loopback kernel module

The app writes processed frames to the virtual device. This is the most complex cross-platform feature and may require bundling or installing a driver component.

**Rationale:** There is no cross-platform virtual camera abstraction. Each OS has its own mechanism. We'll abstract behind a `VirtualCameraOutput` trait similar to the camera backend.

### Decision 7: Presets stored as JSON in app data directory

Presets are stored as JSON files in the OS-appropriate app data directory:

- Windows: `%APPDATA%/cameras/presets/`
- macOS: `~/Library/Application Support/cameras/presets/`
- Linux: `~/.config/cameras/presets/`

Structure: `presets/<camera-id>/<preset-name>.json`

**Rationale:** JSON is human-readable, editable, and easy to import/export. Per-camera directories keep presets scoped. The app data directory is the standard location for user configuration.

### Decision 8: OBS integration via obs-websocket v5

OBS integration uses the obs-websocket v5 protocol over a WebSocket connection from Rust. This allows reading OBS scenes/sources and coordinating camera source selection.

**Rationale:** obs-websocket is the official remote control protocol for OBS Studio, widely adopted, and doesn't require building an OBS plugin (which would be a separate C/C++ project). The Rust ecosystem has mature WebSocket client libraries (`tokio-tungstenite`).

### Decision 9: RTSP network camera support via GStreamer or ffmpeg

RTSP cameras are supported through a separate `RtspClient` module that connects to RTSP URLs and produces frames in the same format as local cameras. The RTSP client uses `ffmpeg` bindings (`ffmpeg-next` crate) for RTSP stream decoding.

RTSP cameras are added manually by the user (URL entry) and persisted in settings. They appear in the sidebar alongside local cameras. Since RTSP cameras have no UVC controls, only software-side controls (colour grading, digital zoom, overlays) are available.

**Rationale:** `ffmpeg` has mature, battle-tested RTSP support across all platforms. The `ffmpeg-next` Rust crate provides safe bindings. Using ffmpeg also positions us for future protocol support (RTMP, SRT, NDI) without architectural changes.

**Alternative considered:** GStreamer — more modular but heavier dependency and harder to bundle cross-platform. Pure-Rust RTSP libraries exist but lack maturity for production use.

### Decision 10: Feed overlay compositing pipeline

Overlays (text, images, borders, watermarks) are composited in the same GPU pipeline as colour grading, using `wgpu`:

1. Camera frame → hardware controls
2. → Software colour correction
3. → LUT application
4. → **Overlay compositing** (text, images, borders, watermarks in layer order)
5. → Output to preview + virtual camera

Overlay definitions are stored as JSON alongside camera settings. The overlay engine supports multiple layers with independent visibility toggles and a user-reorderable z-stack.

**Rationale:** Compositing in the GPU pipeline means overlays appear in both the preview and virtual camera output with no extra cost. Doing it after colour grading ensures overlays are not affected by colour adjustments (text stays readable, logos stay on-brand).

### Decision 11: Diagnostic stats collection

The Rust backend collects real-time diagnostic stats per camera:

- Actual frame rate (measured)
- Frame drop count and rate
- Capture-to-display latency
- Bandwidth usage (bytes/sec from camera)
- USB bus info (where available via platform APIs)
- Network latency (for RTSP cameras)

Stats are emitted as Tauri events and displayed in a toggleable overlay on the preview. This data is also valuable for debugging camera issues.

### Decision 12: Phased implementation plan

The project is too large to build all at once. Implementation is phased:

**Phase 1 — Foundation:** Tauri project scaffold, camera discovery (Windows first), sidebar with live thumbnails, basic UVC control rendering (with greyed-out unsupported controls + tooltips), preview with diagnostic overlay. This delivers the core "start the app, see your cameras, adjust settings" experience.

**Phase 2 — Persistence & Presets:** Settings persistence, auto-apply on reconnect, preset save/load/import/export, built-in starters. Makes the app useful for daily use.

**Phase 3 — Advanced Controls:** Control modes (auto/semi/manual), zoom (hardware + digital), click-to-focus, resolution/format selection, split before/after preview.

**Phase 4 — Colour, Overlays & Virtual Camera:** Software colour pipeline (wgpu), LUT import, feed overlay system (text, images, borders, watermarks), virtual camera output.

**Phase 5 — Integration & Polish:** OBS websocket integration, global hotkeys, Stream Deck plugin, MIDI support, floating widget, RTSP camera support, auto-updater, accessibility audit.

**Phase 6 — Cross-Platform:** macOS backend (AVFoundation), Linux backend (V4L2), platform-specific testing and polish.

**Rationale:** Phase 1 delivers the user's stated primary use case ("start it and see cameras in the sidebar"). Each subsequent phase adds a complete feature vertical. Cross-platform is last because it's mechanical (same patterns, different APIs) and the Windows implementation validates the architecture.

### Decision 13: CI/CD architecture (modelled on webbertakken/snap)

The project uses a 6-workflow GitHub Actions setup, adapted from the proven pattern in webbertakken/snap (a cross-platform Rust desktop app):

**CI workflows (run on every push to main and on PRs):**

1. **`checks.yml`** — Single-runner quality gate on ubuntu-latest. Runs both Rust checks (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo check`, `cargo test`) and frontend checks (`yarn lint`, `yarn typecheck`, `yarn test`). This is the fast feedback loop — must pass before merge.

2. **`build.yml`** — Cross-platform build verification using a 5-target matrix: Windows x64 (`windows-latest`), Linux x64 (`ubuntu-latest`), Linux ARM (`ubuntu-24.04-arm`), macOS ARM (`macos-latest`, `aarch64-apple-darwin`), macOS Intel (`macos-latest`, `x86_64-apple-darwin`). Each target installs platform-specific dependencies, sets up Rust (`dtolnay/rust-toolchain@stable`) with caching (`swatinem/rust-cache@v2`), sets up Node.js (Volta-pinned via `actions/setup-node`), runs `yarn install`, and executes `yarn tauri build`. This catches platform-specific compilation failures early.

3. **`commit-lint.yml`** — Enforces conventional commit format on PR titles using `amannn/action-semantic-pull-request@v5`. Accepted types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert. This feeds into automated changelog generation by release-please.

4. **`lint-workflows.yml`** — Validates GitHub Actions workflow YAML syntax on PRs that touch `.github/workflows/**`, using `reviewdog/action-actionlint@v1` with inline PR review comments.

**Release workflows:**

5. **`release-please.yml`** — Automated release PR creation using `googleapis/release-please-action@v4`. Monitors conventional commits on main, generates changelogs, and bumps versions in both `Cargo.toml` and `package.json`. Uses a dedicated `RELEASE_TOKEN` secret (not `GITHUB_TOKEN`) to allow the release PR merge to trigger downstream workflows.

6. **`release.yml`** — Build and publish releases using `tauri-apps/tauri-action@v0`, triggered on version tags (`v*.*.*`). The Tauri action handles platform-specific bundling: NSIS/MSI on Windows, DMG on macOS, AppImage/deb on Linux. It also generates the Tauri auto-updater JSON manifest and uploads all artifacts to a GitHub Release. On PRs, runs in plan-only mode (build but don't publish) to validate the release pipeline.

**Release flow:** Push to main → release-please creates/updates a release PR with changelog → merge release PR → release-please creates a version tag → tag triggers release.yml → Tauri action builds all 5 platform targets → artifacts uploaded to GitHub Release with auto-updater manifest.

**Configuration files:**

- `release-please-config.json` — Release type configuration for Rust + Node.js dual versioning
- `.release-please-manifest.json` — Tracks current version for release-please

**Required secrets:**

- `RELEASE_TOKEN` — GitHub PAT for release-please (allows tag creation to trigger release workflow)
- `TAURI_SIGNING_PRIVATE_KEY` — For Tauri auto-updater signature verification
- Code signing secrets added in Phase 5: Apple notarisation (`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`) and Windows Authenticode (`WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD`)

**Rationale:** This pattern is proven in production on webbertakken/snap. The key adaptation is replacing `cargo-dist` (used by snap for pure-Rust builds) with `tauri-apps/tauri-action` (which understands Tauri's frontend build step and platform-specific bundling). Release-please provides fully automated version management and changelogs from conventional commits, removing manual release coordination.

**Alternatives considered:**

- **cargo-dist:** Used by snap, but designed for pure-Rust binaries. Does not understand Tauri's frontend build or platform-specific bundlers (NSIS, DMG, AppImage). Would require extensive custom scripting.
- **Manual release process:** Error-prone, doesn't scale across 5 platform targets, and misses auto-updater manifest generation.
- **Single monolithic CI workflow:** Harder to maintain, slower feedback (quality checks blocked by full cross-platform builds), and poor separation of concerns.

### Decision 14: Two-tier settings — native hardware vs software processing

The settings UI is split into two distinct tiers:

1. **Native hardware controls** (always visible, zero processing cost) — brightness, contrast, saturation, white balance, exposure, gain, sharpness, zoom, focus, pan/tilt, etc. These are applied directly by the camera hardware via UVC commands. They appear first in the settings panel.

2. **Additional settings / software processing** (opt-in, CPU/GPU cost) — colour grading, LUT application, digital zoom, overlays. These require the app's processing pipeline (wgpu compute shaders or CPU fallback) to be active. They are behind an explicit "Additional settings" toggle that the user must enable.

**UI layout order:**

```
[Camera sidebar] | [Preview] | [Settings panel]
                                ├── Native controls (accordion sections)
                                │   ├── Image (brightness, contrast, saturation, ...)
                                │   ├── Exposure & White Balance
                                │   ├── Focus & Zoom (hardware only)
                                │   └── Advanced (pan/tilt, gamma, ...)
                                └── Additional settings [toggle: OFF by default]
                                    ├── Colour Grading (temperature, tint, RGB)
                                    ├── LUT
                                    ├── Digital Zoom
                                    └── Overlays (text, images, borders, watermarks)
```

When additional settings are disabled, the processing pipeline is completely bypassed — frames go directly from capture to preview/virtual camera with zero software processing overhead. The toggle state persists per camera.

**Rationale:** This gives users a clear mental model: native controls are "free" and always available; software processing is a conscious opt-in with a visible cost indicator. It also means users who just want to tweak their camera's built-in settings get a snappy, lightweight experience without unnecessary GPU pipeline initialisation.

## Risks / Trade-offs

- **Frame delivery performance** — Sending JPEG frames via IPC at 30fps may introduce latency or CPU overhead. Mitigation: benchmark early in Phase 1; if inadequate, explore native rendering via Tauri window hooks or a dedicated Rust-side rendering surface.

- **Virtual camera driver complexity** — Virtual camera output requires OS-level driver installation on Windows/macOS, which adds installer complexity and may require code signing. Mitigation: use existing open-source virtual camera implementations (OBS VirtualCam) where possible; make virtual camera an optional feature that the user explicitly enables.

- **Cross-platform UVC control parity** — Different platforms expose different subsets of UVC controls, and behaviour may vary between DirectShow, AVFoundation, and V4L2. Mitigation: the `CameraBackend` trait provides a uniform interface; platform-specific quirks are handled in each implementation. Comprehensive per-platform testing with multiple camera models is essential.

- **Non-UVC cameras (GoPro, Canon DSLR)** — Cameras that connect via USB but use proprietary protocols (GoPro uses its own USB protocol, Canon uses PTP/MTP) may not expose standard UVC controls. Mitigation: PTP/MTP cameras are now planned for native support via `libgphoto2` (see `specs/ptp-camera-support/spec.md` and tasks 29-31). GoPro USB protocol support remains a future investigation. Canon's "EOS Webcam Utility" is now pro-only (paid), making native PTP support a higher priority.

- **wgpu availability** — GPU compute via wgpu requires appropriate GPU drivers. On systems without GPU support (some VMs, older hardware), the colour pipeline must fall back to CPU processing. Mitigation: implement CPU fallback path using `image` crate transformations; detect GPU availability at startup.

- **Tauri v2 maturity** — Tauri v2 is relatively new compared to Electron. Some APIs may have rough edges or missing features. Mitigation: stay on stable releases; contribute upstream fixes if needed; the Tauri team is active and responsive.

## Open Questions

- **Camera identification stability:** USB device paths can change between ports/reboots. Is VID:PID + serial number reliable enough for persistent camera identification across all target platforms? Need to test with multiple camera models.

- **Virtual camera driver bundling:** Should the virtual camera driver be bundled with the installer (heavier but simpler for users) or installed separately (lighter but requires user action)? This may depend on code signing requirements.

- **LUT format support:** Start with .cube only, or also support .3dl, .lut, and other formats from day one? .cube is the most common; others can be added incrementally.

- **Stream Deck SDK licensing:** The Elgato Stream Deck SDK has specific licensing terms. Need to verify compatibility with an open-source project.
