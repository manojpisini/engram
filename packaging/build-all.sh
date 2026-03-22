#!/usr/bin/env bash
# Build ENGRAM binaries for all supported platforms
# Requires: rustup, cross (cargo install cross)
# Usage: ./build-all.sh [version]
set -euo pipefail

VERSION="${1:-0.1.0}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/dist"

echo "Building ENGRAM ${VERSION} for all platforms..."
mkdir -p "$OUT"

# Native build (current platform)
echo "==> Building native release..."
cd "$ROOT"
cargo build --release --bin engram

# Cross-compile targets (requires 'cross' tool)
TARGETS=(
    "x86_64-unknown-linux-gnu"
    "aarch64-unknown-linux-gnu"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
    "x86_64-pc-windows-msvc"
)

for target in "${TARGETS[@]}"; do
    echo "==> Building for ${target}..."
    if command -v cross &>/dev/null; then
        cross build --release --bin engram --target "$target" 2>/dev/null && {
            EXT=""
            [[ "$target" == *"windows"* ]] && EXT=".exe"
            cp "$ROOT/target/$target/release/engram${EXT}" "$OUT/engram-${VERSION}-${target}${EXT}"
            echo "    Built: engram-${VERSION}-${target}${EXT}"
        } || echo "    Skipped: $target (cross-compile failed)"
    else
        echo "    Skipped: $target (install 'cross' for cross-compilation)"
    fi
done

# Package for Debian
if command -v dpkg-deb &>/dev/null; then
    echo "==> Building .deb package..."
    bash "$ROOT/packaging/debian/build.sh" "$VERSION"
fi

echo ""
echo "Build artifacts in: $OUT/"
ls -la "$OUT/" 2>/dev/null || true
