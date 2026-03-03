# Self-testing machine

## Problem

Agents repeatedly claim features are "done" but fail in practice:

- Canon live view was "wired" but `EdsOpenSession` fails with "EDSDK internal error"
- COM init was "fixed" but the error persists
- No automated way to verify the app works end-to-end after changes
- Agent self-testing is manual, ad-hoc, and unreliable

The user must NEVER be asked to test. Agents must verify everything themselves,
end-to-end, before reporting done.

## Solution

Build a self-testing machine: scripts and agent workflows that verify the running
Tauri app works correctly after every change. Three layers:

1. **Dev server harness** — start/stop/restart with log capture to a file
2. **Log verifier** — parse captured logs for success/failure patterns
3. **Agent workflow** — documented procedure in AGENTS.md that agents MUST follow

## Non-goals

- NOT a CI/CD test suite (this is for local agent verification)
- NOT browser automation / Playwright E2E tests (too complex for the immediate need)
- NOT replacing existing unit/visual tests

## Changes

### 1. Dev server harness (`scripts/dev-harness.sh`)

A bash script that wraps `yarn dev` with log capture:

```bash
#!/usr/bin/env bash
# Usage: ./scripts/dev-harness.sh [start|stop|restart|status|logs]
```

**Behaviour:**

- `start` — kills any existing cameras.exe + Vite process, runs `yarn dev` in
  background, pipes all stdout/stderr to `tmp/dev-server.log` (truncated on start)
- `stop` — kills cameras.exe and the Vite dev server (port 5173)
- `restart` — stop + start
- `status` — reports whether cameras.exe and Vite are running
- `logs` — tails the last 100 lines of `tmp/dev-server.log`

**Log file:** `tmp/dev-server.log` (gitignored via existing `tmp/` entry)

**Startup detection:** The script waits up to 30 seconds for the log to contain
`Listening on` (Vite ready) AND either `Auto-started preview` or
`enumerate_devices` (Rust backend ready). Exits 0 on success, 1 on timeout.

### 2. Log verifier (`scripts/verify-logs.sh`)

A bash script that checks `tmp/dev-server.log` for expected patterns:

```bash
#!/usr/bin/env bash
# Usage: ./scripts/verify-logs.sh [--expect PATTERN] [--reject PATTERN] [--wait SECONDS]
```

**Behaviour:**

- `--expect "pattern"` — fail if pattern NOT found in logs (can repeat)
- `--reject "pattern"` — fail if pattern IS found in logs (can repeat)
- `--wait N` — wait up to N seconds for expected patterns to appear
- Exit 0 = all checks pass, exit 1 = failure (prints which check failed)

**Common patterns:**

Success indicators:

- `Auto-started preview session for` — DirectShow camera working
- `Auto-started Canon preview for` — Canon camera working
- `Canon EDSDK backend initialised` — SDK loaded
- `Listening on` — Vite ready

Failure indicators:

- `EDSDK internal error` — SDK session failure
- `camera is not producing frames` — preview failure
- `Failed to start` — any startup failure
- `panic` or `RUST_BACKTRACE` — crash

### 3. Update AGENTS.md

Replace the current "End-to-end verification" section with a complete self-testing
workflow that agents MUST follow after every change:

```
## Self-testing protocol

After ANY code change, agents MUST follow this protocol before reporting done:

### Step 1: Quality checks
- `cargo check --features canon` (must pass)
- `cargo clippy --features canon -- -D warnings` (must pass)
- `cargo test --features canon` (must pass)
- `cargo check` (must pass without canon feature)
- `cargo test` (must pass without canon feature)

### Step 2: Dev server verification
- Run `./scripts/dev-harness.sh restart`
- Wait for startup (the script detects readiness automatically)
- Run `./scripts/verify-logs.sh` with appropriate patterns for the change:
  - Camera changes: expect camera enumeration + preview start
  - Canon changes: expect Canon backend init + session open
  - Preview changes: expect preview sessions started
- Check for ANY errors/warnings in the log that relate to the change
- If errors found: FIX THEM. Do not report done.

### Step 3: Log review
- Read the full `tmp/dev-server.log` after 10-15 seconds of running
- Look for:
  - Error-level log lines (these are bugs, not "expected")
  - Warning-level log lines (investigate, fix if related to the change)
  - Expected success messages for ALL cameras (not just one)
- The dev server MUST be running with ZERO errors related to the change

### Failure protocol
- If any step fails: diagnose, fix, and restart from step 1
- NEVER report "done" with known failures
- NEVER dismiss errors as "expected" or "pre-existing" without user agreement
- NEVER say "it should work" — VERIFY IT WORKS
```

### 4. Gitignore

Ensure `tmp/` is in `.gitignore` (it already is — verify).

## Testing

- Run `./scripts/dev-harness.sh start` and verify it captures logs
- Run `./scripts/verify-logs.sh --expect "Listening on"` against captured logs
- Run `./scripts/dev-harness.sh stop` and verify processes are killed
- Verify the full workflow from AGENTS.md works end-to-end

## Affected files

- `scripts/dev-harness.sh` (new)
- `scripts/verify-logs.sh` (new)
- `AGENTS.md` (updated)
- `.gitignore` (verify `tmp/` entry)
