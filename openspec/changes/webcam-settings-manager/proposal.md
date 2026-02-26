## Why

Most cameras ship with terrible or nonexistent control software. Windows Camera, OBS, and video call apps expose only a handful of settings — users with professional or prosumer cameras (GoPro Hero 12, Canon EOS2000D, Logitech Brio, etc.) have no unified way to discover, configure, and persist the full range of hardware controls. Streamers, content creators, and remote workers waste time re-adjusting settings every session, across every app, with no way to save, share, or automate their configurations.

A dedicated camera settings manager that auto-discovers all connected cameras, dynamically enumerates their capabilities, and provides a polished control surface with presets, persistence, and virtual camera output fills this gap.

## What Changes

This is a greenfield application. All capabilities are new.

- Cross-platform desktop app (Windows + macOS + Linux) built with Tauri v2 + React + TypeScript
- Auto-discovery of all camera devices (USB, built-in, capture cards, IP/RTSP network cameras, GoPro, Canon DSLR, virtual cameras) with hot-plug support
- Left sidebar showing all detected cameras with live preview thumbnails — the primary navigation surface
- Dynamic enumeration of ALL hardware UVC controls per camera, with the UI adapting to what each camera supports
- Full auto/semi-auto/manual control modes for white balance and exposure
- Hardware zoom + digital zoom + click-to-focus (region of interest)
- Resolution, frame rate, and pixel format control (MJPEG/YUV/NV12)
- Split before/after preview for comparing changes in real time
- Full preset system: save/load, built-in starters, import/export, per-camera libraries
- Auto-persist and auto-apply last settings per camera across sessions
- Software colour grading pipeline with LUT (lookup table) file import
- Virtual camera output — pass adjusted feed to other apps (OBS, Zoom, Discord, etc.)
- OBS integration via obs-websocket protocol
- Global hotkeys + Stream Deck / MIDI controller support
- System tray with floating widget for quick access, full panel on expand
- Full feed overlay system — text, images, borders, watermarks composited onto the feed and virtual camera output
- Full diagnostic overlay — resolution, fps, format, bandwidth, dropped frames, latency, USB bus info
- Unsupported controls shown greyed out with explanatory tooltips
- Architecture designed for future extensibility (virtual backgrounds, blur, filters)
- Two-tier settings: native hardware controls (zero cost, always visible) first, software processing controls (CPU/GPU cost) behind an opt-in "Additional settings" toggle
- Accordion-based progressive disclosure for settings organisation within each tier
- Full WCAG 2.2 AA compliance with regular accessibility audits
- System theme following (light/dark) with OS detection
- Distribution via GitHub releases + Tauri auto-updater

## Capabilities

### New Capabilities

- `camera-discovery`: Auto-detection of all camera devices (USB, built-in, capture cards, RTSP network cameras, virtual cameras) with hot-plug events, model identification, capability enumeration, and the left sidebar UI with live preview thumbnails
- `camera-controls`: Dynamic UVC control enumeration and rendering — brightness, contrast, saturation, white balance, exposure, sharpness, gain, zoom, focus, pan/tilt, gamma, hue, backlight compensation, powerline frequency, and any other controls the hardware exposes
- `control-modes`: Auto/semi-auto/manual mode system for white balance and exposure, including "auto-set once" lock behaviour
- `zoom-and-focus`: Hardware zoom, digital zoom (software), and click-to-focus region-of-interest selection in the preview
- `resolution-control`: Resolution, frame rate, and pixel format selection per camera with capability-aware dropdowns
- `preview-engine`: Live camera preview rendering, split before/after comparison view, full diagnostic overlay (resolution, fps, format, bandwidth, dropped frames, latency, USB bus info)
- `preset-system`: Save/load named presets, built-in starter presets, import/export as files, per-camera preset libraries, and auto-apply on camera reconnect
- `settings-persistence`: Per-camera settings storage, auto-apply on launch, reset-to-defaults, and recognised camera memory
- `colour-grading`: Software colour correction pipeline (temperature, tint, RGB curves) and LUT file import/application
- `virtual-camera`: Virtual camera device output that passes the adjusted feed to consuming apps
- `obs-integration`: OBS Studio integration via obs-websocket for scene/source coordination
- `hotkeys-and-controllers`: Global keyboard hotkeys, Stream Deck plugin, and MIDI controller mapping
- `app-shell`: Tauri app shell — system tray, floating widget, full settings panel, window management, auto-updater, and OS theme detection
- `feed-overlays`: Full overlay system — text, images, borders, watermarks composited onto the camera feed with layer management, applied after colour grading and included in virtual camera output
- `settings-ui`: Accordion-based control layout, progressive disclosure, dynamic control rendering based on camera capabilities, greyed-out unsupported controls with tooltips, and full WCAG 2.2 AA compliance

### Modified Capabilities

<!-- None — greenfield project -->

## Impact

- **Rust backend (Tauri)**: Camera device enumeration (platform-specific: DirectShow/Media Foundation on Windows, AVFoundation on macOS, V4L2 on Linux), UVC control read/write, video frame capture pipeline, virtual camera driver integration, hotkey registration, OBS websocket client
- **React frontend**: Camera sidebar, preview canvas (WebGL or Canvas2D), settings panel with dynamic controls, preset manager, colour grading UI, system tray menu
- **Native dependencies**: Platform camera APIs, virtual camera frameworks (e.g. OBS Virtual Camera SDK or custom driver), LUT parsing libraries
- **Distribution**: Tauri bundler for Windows (MSI/NSIS), macOS (DMG), Linux (AppImage/deb); GitHub releases with auto-update manifest
- **External integrations**: OBS via obs-websocket v5, Stream Deck SDK, MIDI device access
