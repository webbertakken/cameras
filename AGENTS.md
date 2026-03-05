# Agents

## Platform support

- **Windows**: Windows 11+ only (Build 22000+). No Windows 10 support.
- **Linux**: Supported
- **macOS**: Supported

## Development approach

- **TDD (Test-Driven Development)** for all implementation
  1. Write failing tests first (red)
  2. Write minimal code to make tests pass (green)
  3. Refactor while keeping tests green (refactor)
- Applies to both Rust (`cargo test`) and frontend (`vitest`)
- No implementation code without corresponding tests
- Use `chainlink` cli for tracking issues and tasks (`.chainlink/` is gitignored)

## Virtual camera dev setup

- **Register**: `pwsh scripts/register-vcam-dev.ps1` (one-time, re-run after CLSID or manifest changes)
- **Register (release)**: `pwsh scripts/register-vcam-dev.ps1 -Release`
- **Unregister**: `pwsh scripts/register-vcam-dev.ps1 -Unregister`

## Dev server

- **Start**: `./scripts/dev-harness.sh start` (captures logs to `tmp/dev-server.log`)
- **Stop**: `./scripts/dev-harness.sh stop`
- **Restart**: `./scripts/dev-harness.sh restart`
- **Status**: `./scripts/dev-harness.sh status`
- **Read logs**: `./scripts/dev-harness.sh logs` or `cat tmp/dev-server.log`
- **Manual fallback** (if harness unavailable):
  ```bash
  taskkill //F //IM cameras.exe 2>/dev/null
  pwsh -c "Get-Process -Id (Get-NetTCPConnection -LocalPort 5173).OwningProcess | Stop-Process -Force" 2>/dev/null
  yarn dev > tmp/dev-server.log 2>&1 &
  ```
- Hot reload only works for frontend (Vite). Rust changes require a full restart.

## Self-testing protocol

**MANDATORY** — agents MUST follow this protocol after ANY code change, before
reporting done. No exceptions. No "it should work" guesses.

### Step 1: Quality checks

Run ALL of these (order matters — fail fast):

```bash
cd src-tauri
cargo check --features canon
cargo clippy --features canon -- -D warnings
cargo test --features canon
cargo check                    # must also work without canon
cargo test                     # must also work without canon
cd ..
yarn lint
yarn typecheck
yarn test
```

If ANY command fails: fix it. Do not proceed.

### Step 2: Dev server verification

```bash
./scripts/dev-harness.sh restart
```

The harness automatically:

- Kills existing cameras.exe + Vite processes
- Starts `yarn dev` with log capture to `tmp/dev-server.log`
- Waits for Vite + Rust backend to be ready (45s timeout)
- Reports success/failure

If startup fails: read the logs, diagnose, fix, restart from step 1.

### Step 3: Log verification

After the dev server is running for 10-15 seconds, verify the logs:

```bash
# Check for expected success patterns
./scripts/verify-logs.sh \
  --expect "Canon EDSDK backend initialised" \
  --expect "Auto-started preview" \
  --reject "EDSDK internal error" \
  --reject "panic" \
  --reject "Failed to start" \
  --wait 15
```

Adapt `--expect` and `--reject` patterns to match your change:

| Change type      | Expected patterns                            | Rejected patterns         |
| ---------------- | -------------------------------------------- | ------------------------- |
| Camera backend   | `Enumerated.*camera`, `Auto-started preview` | `DeviceNotFound`, `panic` |
| Canon SDK        | `Auto-started Canon preview`                 | `EDSDK internal error`    |
| Canon live view  | `Auto-started Canon preview`                 | `not producing frames`    |
| Preview pipeline | `Auto-started preview session`               | `Failed to start preview` |
| Any change       | (no crashes or errors in logs)               | `panic`, `RUST_BACKTRACE` |

> **Note**: `create_camera_state()` runs before the tracing logger is initialised,
> so early SDK init messages (e.g. "Canon EDSDK backend initialised") won't appear
> in logs. Rely on downstream messages (e.g. "Auto-started Canon preview") instead.

### Step 4: Manual log review

Read the full log and look for:

```bash
cat tmp/dev-server.log
```

- **Error-level lines** — these are bugs. Fix them.
- **Warning-level lines** — investigate. Fix if related to the change.
- **Expected success messages for ALL cameras** — not just one.
- The dev server MUST run with ZERO errors related to the change.

### Step 5: E2E verification via Tauri

Use Tauri's WebDriver or manual UI interaction to verify that user-facing features
work end-to-end in the running dev app. Quality checks and log review are not
sufficient — the actual UI buttons and flows must be exercised.

- Confirm the feature's primary action completes without error toasts or console errors
- Check that the UI state updates correctly (e.g., toggles, status indicators)
- Verify no regressions in adjacent features visible in the same view

If the button or feature produces an error: diagnose, fix, restart from step 1.

**Important**: Agents MUST interact with the running app themselves (click buttons,
trigger IPC commands, grab errors) rather than asking the user to test. If a UI
action produces an error toast, capture it from the logs — never rely on the user
to report it.

### Failure protocol

- If any step fails: diagnose, fix, restart from step 1
- **NEVER** report "done" with known failures
- **NEVER** dismiss errors as "expected" or "pre-existing" without explicit user agreement
- **NEVER** say "it should work" — VERIFY IT WORKS by reading the actual logs
- **NEVER** ask the user to test — you are the tester

### Log file reference

- **Primary**: `tmp/dev-server.log` (stdout/stderr from `yarn dev`)
- **Secondary**: `C:\Users\Webber\AppData\Local\com.cameras.app\logs\cameras.log`
- Frontend console output is forwarded to Rust stdout via `tauri-plugin-log`
