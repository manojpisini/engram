#!/usr/bin/env bash
# ENGRAM Installer for macOS (Darwin)
# Usage: sudo ./install.sh
set -euo pipefail

INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/usr/local/etc/engram"
DASHBOARD_DIR="/usr/local/share/engram/dashboard"
LOG_DIR="/usr/local/var/log/engram"
PLIST_SRC="$(cd "$(dirname "$0")/../launchd" && pwd)/com.engram.daemon.plist"
PLIST_DST="/Library/LaunchDaemons/com.engram.daemon.plist"

echo "Installing ENGRAM — Engineering Intelligence, etched in Notion"
echo ""

# Check root
if [ "$(id -u)" -ne 0 ]; then
    echo "Error: Run with sudo"
    exit 1
fi

# Create directories
mkdir -p "$CONFIG_DIR" "$DASHBOARD_DIR" "$LOG_DIR"

# Copy binary
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

if [ -f "$ROOT/target/release/engram" ]; then
    cp "$ROOT/target/release/engram" "$INSTALL_DIR/engram"
    chmod 755 "$INSTALL_DIR/engram"
    echo "  Binary: $INSTALL_DIR/engram"
elif [ -f "$SCRIPT_DIR/engram" ]; then
    cp "$SCRIPT_DIR/engram" "$INSTALL_DIR/engram"
    chmod 755 "$INSTALL_DIR/engram"
    echo "  Binary: $INSTALL_DIR/engram"
else
    echo "Error: engram binary not found. Build first: cargo build --release --bin engram"
    exit 1
fi

# Copy config if not present
if [ ! -f "$CONFIG_DIR/engram.toml" ]; then
    cp "$ROOT/engram.toml.example" "$CONFIG_DIR/engram.toml"
    echo "  Config: $CONFIG_DIR/engram.toml"
else
    echo "  Config: $CONFIG_DIR/engram.toml (existing, kept)"
fi

# Copy dashboard
cp -r "$ROOT/dashboard/"* "$DASHBOARD_DIR/"
echo "  Dashboard: $DASHBOARD_DIR"

# Install launchd plist
if [ -f "$PLIST_SRC" ]; then
    cp "$PLIST_SRC" "$PLIST_DST"
    chmod 644 "$PLIST_DST"
    launchctl unload "$PLIST_DST" 2>/dev/null || true
    launchctl load "$PLIST_DST"
    echo "  Service: launchd daemon loaded"
fi

echo ""
echo "ENGRAM installed! Open http://localhost:3000 to configure."
echo ""
