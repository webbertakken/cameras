## ADDED Requirements

### Requirement: Visual regression test infrastructure

The project SHALL have visual regression testing using Vitest 4 browser mode with Playwright, capturing screenshots of key UI states and comparing them against committed baselines to detect unintended visual changes.

#### Scenario: Empty state baseline

- **WHEN** the app renders with no cameras detected
- **THEN** a screenshot of the empty state (sidebar + placeholder main panel) matches the committed baseline

#### Scenario: Camera sidebar with devices

- **WHEN** the app renders with mocked camera devices in the store
- **THEN** a screenshot of the sidebar entries matches the committed baseline

#### Scenario: Controls panel baseline

- **WHEN** the controls panel renders with mocked control descriptors (sliders, toggles, selects, accordion sections)
- **THEN** a screenshot of the controls panel matches the committed baseline

#### Scenario: Toast notification baseline

- **WHEN** toast notifications of each type (info, success, warning, error) are rendered
- **THEN** a screenshot of the toast container matches the committed baseline

#### Scenario: Disabled controls baseline

- **WHEN** a control slider renders in disabled state with a tooltip
- **THEN** a screenshot of the disabled control matches the committed baseline

### Requirement: CI integration

Visual regression tests SHALL run in CI on every PR using a single consistent platform (Linux + Chromium headless) to avoid cross-platform font rendering differences. Baseline updates SHALL be a manual operation via `vitest --update`.

#### Scenario: PR with unintended visual change

- **WHEN** a PR introduces a CSS change that shifts a component's layout
- **THEN** the visual regression CI job fails with a diff image showing the change

#### Scenario: Intentional visual update

- **WHEN** a developer intentionally changes the UI
- **THEN** they run `yarn test:visual --update` locally, commit the updated baselines, and CI passes

### Requirement: Workspace separation

Unit tests (jsdom) and visual regression tests (browser mode) SHALL run in separate Vitest workspace projects to avoid environment conflicts. The `yarn test` command SHALL run unit tests only. A separate `yarn test:visual` command SHALL run browser-mode visual tests.

#### Scenario: Running unit tests does not start a browser

- **WHEN** the developer runs `yarn test`
- **THEN** only jsdom-based unit tests execute — no browser is launched

#### Scenario: Running visual tests starts a headless browser

- **WHEN** the developer runs `yarn test:visual`
- **THEN** Playwright launches headless Chromium and runs visual regression tests
