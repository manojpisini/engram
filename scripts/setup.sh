#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════
#  ENGRAM — Setup Script
#  Checks prerequisites, installs missing tools, sets up env
# ═══════════════════════════════════════════════════════════════
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
ok()   { echo -e "  ${GREEN}[OK]${NC}    $1"; }
fail() { echo -e "  ${RED}[FAIL]${NC}  $1"; }
warn() { echo -e "  ${YELLOW}[WARN]${NC}  $1"; }
info() { echo -e "  ${CYAN}[INFO]${NC}  $1"; }

ERRORS=0
WARNINGS=0

echo ""
echo "  ╔═══════════════════════════════════════╗"
echo "  ║  ENGRAM — Environment Setup            ║"
echo "  ╚═══════════════════════════════════════╝"
echo ""

# ─── Detect OS ───
OS="unknown"
PKG=""
case "$(uname -s)" in
    Linux*)   OS="linux";;
    Darwin*)  OS="macos";;
    MINGW*|MSYS*|CYGWIN*) OS="windows";;
esac
info "Detected OS: $OS"

if [ "$OS" = "macos" ]; then PKG="brew"
elif [ "$OS" = "linux" ]; then
    if command -v apt-get &>/dev/null; then PKG="apt"
    elif command -v dnf &>/dev/null; then PKG="dnf"
    elif command -v pacman &>/dev/null; then PKG="pacman"
    fi
elif [ "$OS" = "windows" ]; then
    if command -v winget &>/dev/null; then PKG="winget"
    elif command -v choco &>/dev/null; then PKG="choco"
    elif command -v scoop &>/dev/null; then PKG="scoop"
    fi
fi
info "Package manager: ${PKG:-none detected}"

# ─── Auto-install flag ───
AUTO_INSTALL=false
if [[ "${1:-}" == "--install" || "${1:-}" == "-i" ]]; then
    AUTO_INSTALL=true
    info "Auto-install mode enabled"
fi

install_tool() {
    local tool="$1"
    if [ "$AUTO_INSTALL" = false ]; then
        fail "$tool not found. Run with --install to auto-install."
        ERRORS=$((ERRORS + 1))
        return 1
    fi
    info "Installing $tool..."
    case "$PKG" in
        brew)   brew install "$tool" 2>/dev/null ;;
        apt)    sudo apt-get install -y "$tool" 2>/dev/null ;;
        dnf)    sudo dnf install -y "$tool" 2>/dev/null ;;
        pacman) sudo pacman -S --noconfirm "$tool" 2>/dev/null ;;
        winget) winget install --accept-source-agreements --accept-package-agreements "$tool" 2>/dev/null ;;
        choco)  choco install -y "$tool" 2>/dev/null ;;
        scoop)  scoop install "$tool" 2>/dev/null ;;
        *)      fail "No package manager found to install $tool"; ERRORS=$((ERRORS + 1)); return 1 ;;
    esac
}

# ═══════════════════════════════════════════════════════════════
echo ""
echo "  Step 1/6 — Required Tools"
echo "  ─────────────────────────"

# Rust
if command -v rustc &>/dev/null && command -v cargo &>/dev/null; then
    RUST_VER=$(rustc --version 2>/dev/null | head -1)
    ok "Rust: $RUST_VER"
else
    if [ "$AUTO_INSTALL" = true ]; then
        info "Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env" 2>/dev/null || true
        ok "Rust installed"
    else
        fail "Rust not found. Install: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        ERRORS=$((ERRORS + 1))
    fi
fi

# Git
if command -v git &>/dev/null; then
    ok "Git: $(git --version 2>/dev/null | head -1)"
else
    install_tool "git"
fi

# curl
if command -v curl &>/dev/null; then
    ok "curl: $(curl --version 2>/dev/null | head -1)"
else
    install_tool "curl"
fi

# pkg-config (Linux only)
if [ "$OS" = "linux" ]; then
    if command -v pkg-config &>/dev/null; then
        ok "pkg-config available"
    else
        install_tool "pkg-config"
    fi
fi

# ═══════════════════════════════════════════════════════════════
echo ""
echo "  Step 2/6 — Optional Tools"
echo "  ─────────────────────────"

# cargo-audit
if command -v cargo-audit &>/dev/null || cargo audit --version &>/dev/null 2>&1; then
    ok "cargo-audit available"
else
    if [ "$AUTO_INSTALL" = true ]; then
        info "Installing cargo-audit..."
        cargo install cargo-audit 2>/dev/null && ok "cargo-audit installed" || warn "cargo-audit install failed"
    else
        warn "cargo-audit not found (optional: cargo install cargo-audit)"
        WARNINGS=$((WARNINGS + 1))
    fi
fi

# cargo-watch
if command -v cargo-watch &>/dev/null; then
    ok "cargo-watch available"
else
    if [ "$AUTO_INSTALL" = true ]; then
        info "Installing cargo-watch..."
        cargo install cargo-watch 2>/dev/null && ok "cargo-watch installed" || warn "cargo-watch install failed"
    else
        warn "cargo-watch not found (optional: cargo install cargo-watch)"
        WARNINGS=$((WARNINGS + 1))
    fi
fi

# ═══════════════════════════════════════════════════════════════
echo ""
echo "  Step 3/6 — Configuration"
echo "  ────────────────────────"

if [ -f engram.toml ]; then
    ok "engram.toml exists"
    info "All tokens are configured from the dashboard setup wizard (http://localhost:3000)"
    info "No .env file required"
else
    warn "engram.toml not found — it will be created on first run"
    WARNINGS=$((WARNINGS + 1))
fi

# ═══════════════════════════════════════════════════════════════
echo ""
echo "  Step 4/6 — engram.toml Configuration"
echo "  ────────────────────────────────────"

if [ -f engram.toml ]; then
    ok "engram.toml exists"
    # Check if databases are initialized
    DB_COUNT=$(grep -c '=' engram.toml 2>/dev/null || echo 0)
    EMPTY_DBS=$(grep '= ""' engram.toml 2>/dev/null | wc -l || echo 0)
    if [ "$EMPTY_DBS" -gt 5 ]; then
        warn "Many empty database IDs — run ENGRAM and use the dashboard setup wizard"
        WARNINGS=$((WARNINGS + 1))
    else
        ok "Database IDs appear configured"
    fi
else
    fail "engram.toml not found — ENGRAM cannot start without it"
    ERRORS=$((ERRORS + 1))
fi

# ═══════════════════════════════════════════════════════════════
echo ""
echo "  Step 5/6 — Cargo Check (compile validation)"
echo "  ─────────────────────────────────────────────"

if [ -f Cargo.toml ]; then
    info "Running cargo check..."
    if cargo check 2>/dev/null; then
        ok "Cargo check passed"
    else
        fail "Cargo check failed — fix compilation errors"
        ERRORS=$((ERRORS + 1))
    fi
else
    fail "Cargo.toml not found — not in ENGRAM project root"
    ERRORS=$((ERRORS + 1))
fi

# ═══════════════════════════════════════════════════════════════
echo ""
echo "  Step 6/6 — Cargo Test (unit tests)"
echo "  ───────────────────────────────────"

info "Running cargo test..."
if cargo test 2>/dev/null; then
    ok "All tests passed"
else
    warn "Some tests failed — check output above"
    WARNINGS=$((WARNINGS + 1))
fi

# ═══════════════════════════════════════════════════════════════
echo ""
echo "  ═══════════════════════════════════════"
if [ "$ERRORS" -gt 0 ]; then
    echo -e "  ${RED}Setup incomplete: $ERRORS error(s), $WARNINGS warning(s)${NC}"
    echo "  Fix errors above, then re-run: bash scripts/setup.sh"
    exit 1
elif [ "$WARNINGS" -gt 0 ]; then
    echo -e "  ${YELLOW}Setup complete with $WARNINGS warning(s)${NC}"
    echo "  Warnings are non-blocking but should be addressed."
    exit 0
else
    echo -e "  ${GREEN}Setup complete! No issues found.${NC}"
    echo ""
    echo "  Next steps:"
    echo "    1. cargo run --bin engram        (start the daemon)"
    echo "    2. Open http://localhost:3000   (setup wizard on first launch)"
    exit 0
fi
