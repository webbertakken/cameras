#!/usr/bin/env bash
# Dev server harness — start/stop/restart with log capture.
#
# Usage: ./scripts/dev-harness.sh [start|stop|restart|status|logs]
#
# Captures all Rust + Vite output to tmp/dev-server.log.
# Used by agents to verify the app works end-to-end after changes.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LOG_DIR="$PROJECT_ROOT/tmp"
LOG_FILE="$LOG_DIR/dev-server.log"
PID_FILE="$LOG_DIR/dev-server.pid"
STARTUP_TIMEOUT=45

# Colours for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No colour

log_info()  { echo -e "${GREEN}[harness]${NC} $*"; }
log_warn()  { echo -e "${YELLOW}[harness]${NC} $*"; }
log_error() { echo -e "${RED}[harness]${NC} $*"; }

ensure_log_dir() {
    mkdir -p "$LOG_DIR"
}

# Kill cameras.exe if running
kill_rust() {
    taskkill //F //IM cameras.exe 2>/dev/null && log_info "Killed cameras.exe" || true
}

# Kill the Vite dev server on port 5173
kill_vite() {
    pwsh -c 'try { Get-Process -Id (Get-NetTCPConnection -LocalPort 5173 -ErrorAction Stop).OwningProcess | Stop-Process -Force } catch {}' 2>/dev/null && log_info "Killed Vite (port 5173)" || true
}

# Kill all dev processes
kill_all() {
    kill_rust
    kill_vite
    # Also kill by PID file if it exists
    if [[ -f "$PID_FILE" ]]; then
        local pid
        pid=$(cat "$PID_FILE")
        kill "$pid" 2>/dev/null || true
        rm -f "$PID_FILE"
    fi
}

# Check if the dev server is running
is_running() {
    # Check for cameras.exe process
    tasklist //FI "IMAGENAME eq cameras.exe" 2>/dev/null | grep -qi "cameras.exe"
}

# Wait for startup patterns in the log
wait_for_startup() {
    local timeout=$STARTUP_TIMEOUT
    local elapsed=0
    local vite_ready=false
    local backend_ready=false

    log_info "Waiting for dev server startup (timeout: ${timeout}s)..."

    while [[ $elapsed -lt $timeout ]]; do
        if [[ -f "$LOG_FILE" ]]; then
            # Check Vite readiness
            if grep -q "Listening on\|Local:.*http://localhost:5173\|VITE.*ready\|ready in" "$LOG_FILE" 2>/dev/null; then
                vite_ready=true
            fi

            # Check Rust backend readiness
            if grep -q "Auto-started preview\|enumerate_devices\|Canon EDSDK backend\|Running on\|tauri" "$LOG_FILE" 2>/dev/null; then
                backend_ready=true
            fi

            if $vite_ready && $backend_ready; then
                log_info "Dev server ready (${elapsed}s)"
                return 0
            fi

            # Check for fatal errors
            if grep -qi "panic\|RUST_BACKTRACE\|fatal error\|could not compile" "$LOG_FILE" 2>/dev/null; then
                log_error "Fatal error detected during startup!"
                tail -20 "$LOG_FILE"
                return 1
            fi
        fi

        sleep 1
        elapsed=$((elapsed + 1))

        # Progress indicator every 5 seconds
        if [[ $((elapsed % 5)) -eq 0 ]]; then
            log_info "  Still waiting... (${elapsed}s) vite=$vite_ready rust=$backend_ready"
        fi
    done

    log_error "Startup timeout after ${timeout}s (vite=$vite_ready, rust=$backend_ready)"
    if [[ -f "$LOG_FILE" ]]; then
        log_error "Last 20 lines of log:"
        tail -20 "$LOG_FILE"
    fi
    return 1
}

cmd_start() {
    ensure_log_dir

    # Kill existing processes
    kill_all
    sleep 1

    # Truncate log file
    : > "$LOG_FILE"

    log_info "Starting dev server..."
    cd "$PROJECT_ROOT"

    # Start yarn dev in background, capturing all output
    yarn dev > "$LOG_FILE" 2>&1 &
    local pid=$!
    echo "$pid" > "$PID_FILE"
    log_info "Dev server PID: $pid"

    # Wait for startup
    if wait_for_startup; then
        log_info "Dev server is running. Logs: $LOG_FILE"
        return 0
    else
        log_error "Dev server failed to start."
        return 1
    fi
}

cmd_stop() {
    log_info "Stopping dev server..."
    kill_all
    sleep 1

    if is_running; then
        log_warn "cameras.exe still running after stop"
        return 1
    else
        log_info "Dev server stopped."
        return 0
    fi
}

cmd_restart() {
    cmd_stop
    cmd_start
}

cmd_status() {
    if is_running; then
        log_info "Dev server is RUNNING"
        if [[ -f "$LOG_FILE" ]]; then
            local lines
            lines=$(wc -l < "$LOG_FILE")
            log_info "Log file: $LOG_FILE ($lines lines)"
            local errors
            errors=$(grep -ci "error\|failed\|panic" "$LOG_FILE" 2>/dev/null || echo "0")
            local warnings
            warnings=$(grep -ci "warn" "$LOG_FILE" 2>/dev/null || echo "0")
            log_info "Errors: $errors, Warnings: $warnings"
        fi
        return 0
    else
        log_warn "Dev server is NOT running"
        return 1
    fi
}

cmd_logs() {
    if [[ -f "$LOG_FILE" ]]; then
        tail -100 "$LOG_FILE"
    else
        log_warn "No log file found at $LOG_FILE"
        return 1
    fi
}

# Main
case "${1:-help}" in
    start)   cmd_start ;;
    stop)    cmd_stop ;;
    restart) cmd_restart ;;
    status)  cmd_status ;;
    logs)    cmd_logs ;;
    *)
        echo "Usage: $0 {start|stop|restart|status|logs}"
        exit 1
        ;;
esac
