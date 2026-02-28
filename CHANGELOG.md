# Changelog

## [0.5.0](https://github.com/webbertakken/cameras/compare/v0.4.0...v0.5.0) (2026-02-28)


### Features

* **settings:** add settings persistence with auto-save, auto-apply, and reset ([#43](https://github.com/webbertakken/cameras/issues/43)) ([fd63f7c](https://github.com/webbertakken/cameras/commit/fd63f7c10426df86d2eafa2607feac7b69bacdd9))


### Performance improvements

* **preview:** optimise capture pipeline for real-time preview ([#41](https://github.com/webbertakken/cameras/issues/41)) ([4f32a42](https://github.com/webbertakken/cameras/commit/4f32a420f979ce6e9f5968fdd416cc0ecc0789d2))

## [0.4.0](https://github.com/webbertakken/cameras/compare/v0.3.1...v0.4.0) (2026-02-27)


### Features

* **testing:** add visual regression with baselines ([#32](https://github.com/webbertakken/cameras/issues/32)) ([51ccfea](https://github.com/webbertakken/cameras/commit/51ccfea20fde9e44f5381155b1e8da27ed6b929e))


### Bug fixes

* **ci:** add --repo flag to gh pr view in update-snapshots workflow ([#39](https://github.com/webbertakken/cameras/issues/39)) ([62a0538](https://github.com/webbertakken/cameras/commit/62a0538cf7d7ab8359effe03b6e017730464304a))
* implement DirectShow capture pipeline and wire preview ([b8d2293](https://github.com/webbertakken/cameras/commit/b8d2293c40d8e578acacbb52d1fb4ea1bf62042b))
* **logging:** explicit stdout target and debug println for preview ([#36](https://github.com/webbertakken/cameras/issues/36)) ([9d67695](https://github.com/webbertakken/cameras/commit/9d6769593866f31b194daa6e0428a9b1c3010538))
* **preview:** accept any video format, handle virtual cameras ([#38](https://github.com/webbertakken/cameras/issues/38)) ([6d24637](https://github.com/webbertakken/cameras/commit/6d24637c29178cc4ee44b4f35379fd864d43e433))
* **preview:** decode base64 frame response from Rust backend ([#40](https://github.com/webbertakken/cameras/issues/40)) ([40ababa](https://github.com/webbertakken/cameras/commit/40abababf935a724ace78a81d06cf10d4e26a1b3))
* **ui:** resolve tray icon toggle and dark mode flash ([#34](https://github.com/webbertakken/cameras/issues/34)) ([3140683](https://github.com/webbertakken/cameras/commit/314068318513b96ef0993e27a022c83cfc853f0b))


### Maintenance

* make yarn dev run full Tauri app ([#31](https://github.com/webbertakken/cameras/issues/31)) ([ba5250d](https://github.com/webbertakken/cameras/commit/ba5250d53b4874edcebb3f253a40769b3aa6415c))

## [0.3.1](https://github.com/webbertakken/cameras/compare/v0.3.0...v0.3.1) (2026-02-26)

### Bug fixes

- **build:** fix macOS build and bundle identifier ([#28](https://github.com/webbertakken/cameras/issues/28)) ([#29](https://github.com/webbertakken/cameras/issues/29)) ([75d7846](https://github.com/webbertakken/cameras/commit/75d78469a958c4c8a200b2a17e8b27781913400c))

## [0.3.0](https://github.com/webbertakken/cameras/compare/v0.2.0...v0.3.0) (2026-02-26)

### Features

- **controls:** add camera controls UI with IPC, accordion grouping, and WCAG accessibility ([#13](https://github.com/webbertakken/cameras/issues/13)) ([adb1d6d](https://github.com/webbertakken/cameras/commit/adb1d6d05a94ae077b8f07b267ea19579b46c3b9))
- **notifications:** add toast notifications for camera hotplug events ([#21](https://github.com/webbertakken/cameras/issues/21)) ([3ab125a](https://github.com/webbertakken/cameras/commit/3ab125a0262ed7cd7fcdb1aa2eb18ab0a79d75ce))
- **notifications:** add toast notifications for camera hotplug events ([#22](https://github.com/webbertakken/cameras/issues/22)) ([8903857](https://github.com/webbertakken/cameras/commit/89038578db1e66fa21551cdbf75d1a172194cda5))
- **preview:** add frame capture pipeline and preview components ([#12](https://github.com/webbertakken/cameras/issues/12)) ([ec3b552](https://github.com/webbertakken/cameras/commit/ec3b552d3d6d81e7f7f42b1d43de8f8051873ad3))

### Bug fixes

- add engines field to package.json ([#16](https://github.com/webbertakken/cameras/issues/16)) ([434a0aa](https://github.com/webbertakken/cameras/commit/434a0aa9026be6d5fb964723e6bcc3f4b1321657))
- align rustfmt edition with Cargo.toml ([#25](https://github.com/webbertakken/cameras/issues/25)) ([97ce87c](https://github.com/webbertakken/cameras/commit/97ce87c8b41db8957c245ec71c598a646dd518fb))
- **app:** wire main panel to selected camera ([#17](https://github.com/webbertakken/cameras/issues/17)) ([f1229fa](https://github.com/webbertakken/cameras/commit/f1229fadbd12c14a97a3a42129144b590101cc8a))
- **camera:** scope COM init to per-method calls ([#27](https://github.com/webbertakken/cameras/issues/27)) ([183ad55](https://github.com/webbertakken/cameras/commit/183ad55d8d646064ae8d27ace86bbc56a28b3b66))
- **diagnostics:** add USB bus info to diagnostic snapshot ([#20](https://github.com/webbertakken/cameras/issues/20)) ([38ec5f4](https://github.com/webbertakken/cameras/commit/38ec5f45c93bdaf89cc6a51b973830984942f891))
- **hooks:** add cargo clippy and lib tests to pre-commit ([#26](https://github.com/webbertakken/cameras/issues/26)) ([395c38f](https://github.com/webbertakken/cameras/commit/395c38f5a06f2a7a1ac18db04d1516c600c91a81))
- **hotplug:** wire hotplug events end-to-end ([#18](https://github.com/webbertakken/cameras/issues/18)) ([c829187](https://github.com/webbertakken/cameras/commit/c829187be7599081dd1e3db92f6d698b0808eb7b))
- **sidebar:** wire live thumbnails into camera entries ([#19](https://github.com/webbertakken/cameras/issues/19)) ([ac6c76d](https://github.com/webbertakken/cameras/commit/ac6c76d11ce187513e6711375b2dd92ea5cf2771))

### Maintenance

- **openspec:** add visual regression spec and tasks ([dc24719](https://github.com/webbertakken/cameras/commit/dc24719ebf6259e75eef41f5534fd01e20236aae))
- rename project from Webcam to Cameras ([#24](https://github.com/webbertakken/cameras/issues/24)) ([439c3d1](https://github.com/webbertakken/cameras/commit/439c3d18b0d9897fa4b91ec6fd331014fa44ca31))

## [0.2.0](https://github.com/webbertakken/cameras/compare/v0.1.0...v0.2.0) (2026-02-26)

### Features

- add app shell — system tray, theme, and design tokens ([#6](https://github.com/webbertakken/cameras/issues/6)) ([3326997](https://github.com/webbertakken/cameras/commit/3326997e119683598be48e7a8e62157eed4b013d))
- add linting, formatting, and pre-commit hooks ([#4](https://github.com/webbertakken/cameras/issues/4)) ([3fbb9ff](https://github.com/webbertakken/cameras/commit/3fbb9ff43542ed65a745bfbd4b33bf00786cfbe0))
- add OpenSpec for webcam settings manager ([e3f9644](https://github.com/webbertakken/cameras/commit/e3f9644be309fa5eb1c2cab034c21067bcf1aca7))
- **camera-sidebar:** add camera discovery sidebar with Zustand store ([#10](https://github.com/webbertakken/cameras/issues/10)) ([1e63f7b](https://github.com/webbertakken/cameras/commit/1e63f7b10abef6824be71698d08ff6f1df56803e))
- **camera:** add camera control get/set backend ([#11](https://github.com/webbertakken/cameras/issues/11)) ([fb28e08](https://github.com/webbertakken/cameras/commit/fb28e083d31c7436e0fc00659f03bdbf5a0e0e3e))
- **camera:** add camera discovery Rust backend ([#7](https://github.com/webbertakken/cameras/issues/7)) ([cb4c83f](https://github.com/webbertakken/cameras/commit/cb4c83f6e450c1f08162fbb40b4d5328618b6561))
- **ci:** add CI/CD workflows and release-please ([#2](https://github.com/webbertakken/cameras/issues/2)) ([0a9e16b](https://github.com/webbertakken/cameras/commit/0a9e16bfb7fa2edae50437b6d74f2be59f7691d7))
- domain-driven structure + Tauri permissions ([#3](https://github.com/webbertakken/cameras/issues/3)) ([59ecd95](https://github.com/webbertakken/cameras/commit/59ecd95701a4a684c03e2088f7cd1ec9364f03d4))
- scaffold Tauri v2 project with React + TypeScript + Vite ([#1](https://github.com/webbertakken/cameras/issues/1)) ([2457958](https://github.com/webbertakken/cameras/commit/245795873332b1376e3e5a6c88e2cef479385b64))
- **spec:** add two-tier settings architecture (native vs software processing) ([aabcaf4](https://github.com/webbertakken/cameras/commit/aabcaf478284a29c0d41c5b33517e27e1cc083e7))

### Bug fixes

- add .claude and .husky/\_ to lint/format ignore patterns ([#5](https://github.com/webbertakken/cameras/issues/5)) ([7101b8f](https://github.com/webbertakken/cameras/commit/7101b8f3073393ac945eb28939a5007b83ce3924))
- **ci:** escape regex backslashes in release-please config ([#8](https://github.com/webbertakken/cameras/issues/8)) ([b462214](https://github.com/webbertakken/cameras/commit/b4622148b50a5f8245389d02df52a032e213b349))

### Maintenance

- add project README ([#14](https://github.com/webbertakken/cameras/issues/14)) ([fdee090](https://github.com/webbertakken/cameras/commit/fdee090ba42dbd201c1cb0df97ba41657bd9e2f8))
