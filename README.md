# Webcam Settings

Desktop app for managing webcam settings — brightness, contrast, white balance and more — with a live preview.

## Features

- Live camera preview with real-time control adjustments
- Dynamic controls driven by hardware capabilities
- Platform-native camera access (DirectShow/Media Foundation, V4L2, AVFoundation)
- Camera discovery with hot-plug detection
- System tray with global shortcuts

## Tech stack

- **Frontend:** React 19, TypeScript, Vite, Zustand
- **Backend:** Rust, Tauri v2
- **Platform APIs:** windows-rs, v4l2 (Linux), ObjC bindings (macOS)

## Development

### Prerequisites

- [Rust](https://rustup.rs/) 1.77.2+
- [Volta](https://volta.sh/) (manages Node 22 and Yarn 4.9.2 automatically)
- Platform dependencies for [Tauri v2](https://v2.tauri.app/start/prerequisites/)

### Setup

```sh
yarn install
yarn tauri dev
```

### Quality checks

```sh
yarn lint          # ESLint
yarn format        # Prettier
yarn typecheck     # TypeScript
yarn test          # Vitest
cargo clippy       # Rust lints
cargo test         # Rust tests
```

## Licence

MIT
