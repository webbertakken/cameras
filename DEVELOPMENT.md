# Development setup

## Prerequisites

- **Rust** 1.77.2+ (via [rustup](https://rustup.rs/))
- **Node.js** 22.x (via [Volta](https://volta.sh/))
- **Yarn** 4.x (via Volta)
- **Tauri v2 system deps** — see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
  - Windows: Visual Studio Build Tools, WebView2
  - macOS: Xcode command line tools
  - Linux: `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf`

## Quick start

```bash
yarn install
cd src-tauri && cargo check
yarn tauri dev
```

## Running tests

```bash
# Frontend
yarn test
yarn lint
yarn typecheck

# Rust (all tests)
cd src-tauri && cargo test --lib

# Rust with Canon EDSDK feature
cd src-tauri && cargo test --lib --features canon
```

## Canon EDSDK setup

Canon EOS cameras use the proprietary EDSDK for USB communication. All Canon code is behind the `canon` Cargo feature flag.

### Without EDSDK DLLs (mock-only)

- `cargo check` and `cargo test --lib` work without any DLLs
- `cargo test --lib --features canon` runs mock-based tests
- No real Canon camera access — only `MockEdsSdk` is used

### With EDSDK DLLs (real hardware)

1. Download the EDSDK from the [Canon Developer Portal](https://developercommunity.usa.canon.com/s/camera)
2. Place the DLLs in `src-tauri/lib/edsdk/`:
   - `EDSDK.dll`
   - `EdsImage.dll`
   - `DPPDLL.dll` (if needed)
3. Build with `cargo build --features canon`

> The `src-tauri/lib/` directory is gitignored — DLLs are never committed.

## Future SDK support

- **GoPro HTTP API** — planned, no additional DLLs needed
