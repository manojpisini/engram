#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════
#  ENGRAM — Build Script
#  Builds all binaries in debug or release mode
# ═══════════════════════════════════════════════════════════════
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
ok()   { echo -e "  ${GREEN}[OK]${NC}    $1"; }
fail() { echo -e "  ${RED}[FAIL]${NC}  $1"; }
info() { echo -e "  ${CYAN}[INFO]${NC}  $1"; }

MODE="${1:-debug}"

echo ""
echo "  ╔═══════════════════════════════════════╗"
echo "  ║  ENGRAM — Build                        ║"
echo "  ╚═══════════════════════════════════════╝"
echo ""

# ─── Parse mode ───
CARGO_FLAGS=""
TARGET_DIR="debug"
if [[ "$MODE" == "release" || "$MODE" == "--release" ]]; then
    CARGO_FLAGS="--release"
    TARGET_DIR="release"
    info "Building in RELEASE mode"
else
    info "Building in DEBUG mode (use: bash scripts/build.sh release)"
fi

# ─── Pre-build: kill old engram if running (Windows) ───
if [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* ]]; then
    if tasklist 2>/dev/null | grep -qi "engram.exe"; then
        info "Killing running engram.exe..."
        taskkill //F //IM engram.exe 2>/dev/null || true
        sleep 1
    fi
fi

# ═══════════════════════════════════════════════════════════════
echo ""
echo "  Step 1 — Build workspace"
echo "  ────────────────────────"

info "cargo build --workspace $CARGO_FLAGS"
if cargo build --workspace $CARGO_FLAGS 2>&1; then
    ok "Workspace build succeeded"
else
    fail "Build failed"
    exit 1
fi

# ═══════════════════════════════════════════════════════════════
echo ""
echo "  Step 2 — Verify binaries"
echo "  ────────────────────────"

check_binary() {
    local name="$1"
    local ext=""
    if [[ "$(uname -s)" == MINGW* || "$(uname -s)" == MSYS* ]]; then
        ext=".exe"
    fi
    local path="target/$TARGET_DIR/${name}${ext}"
    if [ -f "$path" ]; then
        local size=$(du -sh "$path" 2>/dev/null | cut -f1)
        ok "$name ($size)"
    else
        fail "$name not found at $path"
    fi
}

check_binary "engram"

# ═══════════════════════════════════════════════════════════════
echo ""
echo "  Step 3 — Quick test run"
echo "  ───────────────────────"

info "cargo test --workspace $CARGO_FLAGS (no-fail-fast)"
if cargo test --workspace $CARGO_FLAGS 2>&1; then
    ok "All tests passed"
else
    echo -e "  ${YELLOW}[WARN]${NC}  Some tests failed"
fi

# ═══════════════════════════════════════════════════════════════
echo ""
echo "  ═══════════════════════════════════════"
echo -e "  ${GREEN}Build complete!${NC}"
echo ""
echo "  Binary:"
echo "    target/$TARGET_DIR/engram      — Main daemon"
echo ""
echo "  Run:"
echo "    cargo run --bin engram          — Start ENGRAM daemon"
echo "    http://localhost:3000          — Open dashboard (setup wizard on first launch)"
echo ""
echo "  Usage:"
echo "    bash scripts/build.sh          # Debug build"
echo "    bash scripts/build.sh release  # Release build"
