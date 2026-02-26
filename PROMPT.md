# PROMPT.md

## Current task

Building a **webcam settings manager** desktop app. All planning/spec work is complete. Ready to start **Phase 1 implementation** (project scaffold, camera discovery, preview, basic controls, app shell).

## Method

Using **OpenSpec** workflow. All artefacts are at `openspec/changes/webcam-settings-manager/`:

- `proposal.md` — 15 capabilities identified
- `design.md` — 14 architectural decisions (monorepo, CameraBackend trait, frame delivery, dynamic controls, wgpu colour pipeline, virtual camera, presets as JSON, OBS via obs-websocket, RTSP via retina, overlay compositing, diagnostics, phased plan, CI/CD, two-tier settings)
- `specs/` — 15 spec files with WHEN/THEN scenarios
- `tasks.md` — ~160 tasks across 27 task groups in 6 phases

Change ID: `webcam-settings-manager`
Branch: `main`
Repo: `https://github.com/webbertakken/cameras` (private)

## Key things to remember

### Stack

- **Tauri v2 + React + TypeScript** (Vite frontend)
- **Rust backend** for all camera access, frame capture, colour pipeline, virtual camera
- **Cross-platform:** Windows + macOS + Linux
- **Yarn** for package management, **Volta** for Node/Yarn versions

### Architecture decisions

- Platform-native camera crates (`windows-rs`, `v4l`, ObjC bindings) — NOT nokhwa (too limited for full UVC control enumeration)
- Dynamic control descriptors from Rust drive the frontend UI — no hardcoded controls
- JPEG-compressed frames via Tauri IPC for preview delivery
- `wgpu` GPU compute for colour grading pipeline (with CPU fallback via `image` crate)
- `retina` crate for RTSP (pure Rust, lighter than ffmpeg-next)
- `obws` crate for OBS integration (typed obs-websocket v5 client)
- Virtual camera via platform-specific drivers (DirectShow/CoreMediaIO/v4l2loopback)
- Presets stored as JSON in OS app data directory, per-camera scoped
- Stream Deck plugin is a separate Node.js project (Elgato SDK), communicates via local WebSocket

### Key Rust crates (~30 deps)

- **Core:** tauri, tokio, serde, serde_json
- **Tauri plugins:** global-shortcut, autostart, single-instance, updater
- **Camera:** windows (windows-rs), v4l, objc2/core-foundation
- **GPU:** wgpu
- **Image:** image, fast_image_resize
- **Network:** retina (RTSP), obws (OBS)
- **Input:** midir + midi-msg (MIDI)
- **Infra:** tracing, thiserror, anyhow, parking_lot, crossbeam-channel, dirs, uuid, chrono, notify

### CI/CD (modelled after webbertakken/snap)

- 6 GitHub Actions workflows: checks, build (5-target matrix), commit-lint, lint-workflows, release-please, release
- `tauri-apps/tauri-action` replaces cargo-dist for bundling (MSI, DMG, AppImage/deb)
- release-please for automated version bumping + changelog
- Conventional commits enforced on PR titles
- Code signing needed for macOS notarisation + Windows Authenticode

### UI/UX decisions

- **Left sidebar with live camera previews** is the HERO feature
- **Two-tier settings:** native hardware controls (zero cost) shown first; software processing (colour grading, LUT, digital zoom, overlays) behind an opt-in "Additional settings" toggle — per-camera persistence
- Accordion sections for progressive disclosure within each tier
- Split before/after preview for comparing changes
- System tray + floating widget with most-used controls
- Greyed-out unsupported controls with tooltip (not hidden)
- Follow OS theme (light/dark)
- Full WCAG 2.2 AA accessibility compliance
- Full overlay system (text, images, borders, watermarks)
- No per-app profiles, no audio controls, no virtual effects in v1

### User preferences (from CLAUDE.md)

- British English, sentence case
- Domain-driven structure, small focused modules
- Always use PRs, never push directly to main
- Always use OpenSpec for planning
- Fix all issues immediately, no suppressions
- Yarn, Volta, pre-commit hooks mirroring CI
- No unnecessary docs/README creation

## Progress

- OpenSpec complete (proposal, design, 15 specs, ~160 tasks) — all verified 38/38 requirements
- Rust crate list researched and documented
- CI/CD pipeline designed (6 workflows)
- Initial commit pushed to `https://github.com/webbertakken/webcam`
- **Next step:** Start Phase 1 implementation — task group 1 (project scaffold)
