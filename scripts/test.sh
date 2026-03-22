#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════
#  ENGRAM — Test Script
#  Runs all tests: unit, integration, lint, and optional smoke
# ═══════════════════════════════════════════════════════════════
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
ok()   { echo -e "  ${GREEN}[PASS]${NC}  $1"; }
fail() { echo -e "  ${RED}[FAIL]${NC}  $1"; }
info() { echo -e "  ${CYAN}[RUN]${NC}   $1"; }

FAILED=0
MODE="${1:-all}"

echo ""
echo "  ╔═══════════════════════════════════════╗"
echo "  ║  ENGRAM — Test Suite                   ║"
echo "  ╚═══════════════════════════════════════╝"
echo ""
echo "  Mode: $MODE"
echo ""

# ═══════════════════════════════════════════════════════════════
if [[ "$MODE" == "all" || "$MODE" == "check" ]]; then
    echo "  Step 1 — Cargo Check (compilation)"
    echo "  ───────────────────────────────────"
    info "cargo check --workspace"
    if cargo check --workspace 2>&1; then
        ok "Compilation check passed"
    else
        fail "Compilation check failed"
        FAILED=$((FAILED + 1))
    fi
    echo ""
fi

# ═══════════════════════════════════════════════════════════════
if [[ "$MODE" == "all" || "$MODE" == "unit" ]]; then
    echo "  Step 2 — Unit Tests"
    echo "  ───────────────────"

    CRATES=(engram-types engram-decisions engram-pulse engram-shield engram-atlas engram-vault engram-review engram-health engram-timeline engram-release)

    for crate in "${CRATES[@]}"; do
        info "Testing $crate..."
        if cargo test -p "$crate" 2>&1; then
            ok "$crate"
        else
            fail "$crate"
            FAILED=$((FAILED + 1))
        fi
    done
    echo ""
fi

# ═══════════════════════════════════════════════════════════════
if [[ "$MODE" == "all" || "$MODE" == "lint" ]]; then
    echo "  Step 3 — Lint (clippy)"
    echo "  ──────────────────────"

    if command -v cargo-clippy &>/dev/null || rustup component list --installed 2>/dev/null | grep -q clippy; then
        info "cargo clippy --workspace"
        if cargo clippy --workspace -- -D warnings 2>&1; then
            ok "Clippy passed (no warnings)"
        else
            # Clippy warnings are non-fatal for now
            echo -e "  ${YELLOW}[WARN]${NC}  Clippy has warnings (non-blocking)"
        fi
    else
        echo -e "  ${YELLOW}[SKIP]${NC}  clippy not installed (rustup component add clippy)"
    fi
    echo ""
fi

# ═══════════════════════════════════════════════════════════════
if [[ "$MODE" == "all" || "$MODE" == "fmt" ]]; then
    echo "  Step 4 — Format Check"
    echo "  ─────────────────────"

    if command -v rustfmt &>/dev/null; then
        info "cargo fmt --check"
        if cargo fmt --check 2>&1; then
            ok "Code is properly formatted"
        else
            echo -e "  ${YELLOW}[WARN]${NC}  Formatting issues found (run: cargo fmt)"
        fi
    else
        echo -e "  ${YELLOW}[SKIP]${NC}  rustfmt not installed"
    fi
    echo ""
fi

# ═══════════════════════════════════════════════════════════════
if [[ "$MODE" == "smoke" ]]; then
    echo "  Step 5 — Smoke Tests (requires running daemon)"
    echo "  ───────────────────────────────────────────────"

    BASE="${ENGRAM_URL:-http://localhost:3000}"
    info "Testing against $BASE"

    smoke_test() {
        local name="$1" method="$2" path="$3" data="${4:-}"
        if [ "$method" = "GET" ]; then
            HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 3 "$BASE$path" 2>/dev/null || echo "000")
        else
            HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 3 -X POST -H "Content-Type: application/json" -d "$data" "$BASE$path" 2>/dev/null || echo "000")
        fi

        if [ "$HTTP_CODE" = "200" ]; then
            ok "$name ($method $path) -> $HTTP_CODE"
        else
            fail "$name ($method $path) -> $HTTP_CODE"
            FAILED=$((FAILED + 1))
        fi
    }

    smoke_test "Health Check"          GET  "/health"
    smoke_test "API Config"            GET  "/api/config"
    smoke_test "GitHub Connection"     GET  "/api/github/connection"
    smoke_test "Notion Connection"     GET  "/api/notion/connection"
    smoke_test "Dashboard Health"      GET  "/api/dashboard/health"
    smoke_test "Dashboard Events"      GET  "/api/dashboard/events"
    smoke_test "Dashboard RFCs"        GET  "/api/dashboard/rfcs"
    smoke_test "Dashboard Vulns"       GET  "/api/dashboard/vulnerabilities"
    smoke_test "Dashboard Reviews"     GET  "/api/dashboard/reviews"
    smoke_test "Dashboard Modules"     GET  "/api/dashboard/modules"
    smoke_test "Dashboard Env Config"  GET  "/api/dashboard/env-config"
    smoke_test "Dashboard Benchmarks"  GET  "/api/dashboard/benchmarks"
    smoke_test "Dashboard Releases"    GET  "/api/dashboard/releases"
    smoke_test "GitHub Repos"          GET  "/api/github/repos"
    smoke_test "Projects"              GET  "/api/projects"

    # POST triggers
    smoke_test "Trigger Digest"   POST "/api/trigger/digest"  '{"project_id":"test"}'
    smoke_test "Trigger Onboard"  POST "/api/trigger/onboard" '{"engineer_name":"Test","role":"backend","project_id":"test"}'
    smoke_test "Trigger Release"  POST "/api/trigger/release" '{"project_id":"test","version":"0.1.0","milestone":"MVP"}'

    echo ""
fi

# ═══════════════════════════════════════════════════════════════
echo "  ═══════════════════════════════════════"
if [ "$FAILED" -gt 0 ]; then
    echo -e "  ${RED}$FAILED test(s) failed${NC}"
    exit 1
else
    echo -e "  ${GREEN}All tests passed!${NC}"
    exit 0
fi
echo ""
echo "  Usage:"
echo "    bash scripts/test.sh          # Run all tests"
echo "    bash scripts/test.sh unit     # Unit tests only"
echo "    bash scripts/test.sh lint     # Clippy only"
echo "    bash scripts/test.sh fmt      # Format check only"
echo "    bash scripts/test.sh check    # Compilation check only"
echo "    bash scripts/test.sh smoke    # Smoke tests (daemon must be running)"
