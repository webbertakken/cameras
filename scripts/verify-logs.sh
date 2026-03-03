#!/usr/bin/env bash
# Log verifier — check dev server logs for expected/rejected patterns.
#
# Usage:
#   ./scripts/verify-logs.sh --expect "pattern" --reject "pattern" [--wait SECONDS]
#
# Options:
#   --expect "pattern"  Fail if pattern NOT found (repeatable)
#   --reject "pattern"  Fail if pattern IS found (repeatable)
#   --wait N            Wait up to N seconds for expected patterns (default: 0)
#   --log FILE          Log file to check (default: tmp/dev-server.log)
#
# Exit: 0 = all checks pass, 1 = failure

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LOG_FILE="$PROJECT_ROOT/tmp/dev-server.log"
WAIT_SECONDS=0

declare -a EXPECT_PATTERNS=()
declare -a REJECT_PATTERNS=()

# Colours
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --expect)
            EXPECT_PATTERNS+=("$2")
            shift 2
            ;;
        --reject)
            REJECT_PATTERNS+=("$2")
            shift 2
            ;;
        --wait)
            WAIT_SECONDS="$2"
            shift 2
            ;;
        --log)
            LOG_FILE="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 --expect 'pattern' --reject 'pattern' [--wait N] [--log FILE]"
            exit 1
            ;;
    esac
done

if [[ ${#EXPECT_PATTERNS[@]} -eq 0 ]] && [[ ${#REJECT_PATTERNS[@]} -eq 0 ]]; then
    echo "No patterns specified. Use --expect and/or --reject."
    exit 1
fi

if [[ ! -f "$LOG_FILE" ]]; then
    echo -e "${RED}[verify]${NC} Log file not found: $LOG_FILE"
    echo "Start the dev server first: ./scripts/dev-harness.sh start"
    exit 1
fi

FAILED=0

# Wait for expected patterns if --wait specified
if [[ $WAIT_SECONDS -gt 0 ]] && [[ ${#EXPECT_PATTERNS[@]} -gt 0 ]]; then
    echo -e "${YELLOW}[verify]${NC} Waiting up to ${WAIT_SECONDS}s for expected patterns..."
    elapsed=0
    while [[ $elapsed -lt $WAIT_SECONDS ]]; do
        all_found=true
        for pattern in "${EXPECT_PATTERNS[@]}"; do
            if ! grep -qi "$pattern" "$LOG_FILE" 2>/dev/null; then
                all_found=false
                break
            fi
        done
        if $all_found; then
            echo -e "${GREEN}[verify]${NC} All expected patterns found after ${elapsed}s"
            break
        fi
        sleep 1
        elapsed=$((elapsed + 1))
    done
fi

# Check expected patterns
for pattern in "${EXPECT_PATTERNS[@]}"; do
    if grep -qi "$pattern" "$LOG_FILE" 2>/dev/null; then
        echo -e "${GREEN}[verify] PASS${NC} Expected pattern found: \"$pattern\""
    else
        echo -e "${RED}[verify] FAIL${NC} Expected pattern NOT found: \"$pattern\""
        FAILED=1
    fi
done

# Check rejected patterns
for pattern in "${REJECT_PATTERNS[@]}"; do
    if grep -qi "$pattern" "$LOG_FILE" 2>/dev/null; then
        echo -e "${RED}[verify] FAIL${NC} Rejected pattern found: \"$pattern\""
        # Show the matching lines for context
        echo -e "${YELLOW}  Matches:${NC}"
        grep -in "$pattern" "$LOG_FILE" 2>/dev/null | head -5 | sed 's/^/    /'
        FAILED=1
    else
        echo -e "${GREEN}[verify] PASS${NC} Rejected pattern absent: \"$pattern\""
    fi
done

# Summary
echo ""
if [[ $FAILED -eq 0 ]]; then
    echo -e "${GREEN}[verify] All checks passed.${NC}"
else
    echo -e "${RED}[verify] Some checks FAILED. Review logs: $LOG_FILE${NC}"
fi

exit $FAILED
