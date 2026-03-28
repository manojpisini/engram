#!/usr/bin/env bash
# Build .deb package for ENGRAM
# Usage: ./build.sh [version]
set -euo pipefail

VERSION="${1:-0.1.0}"
ARCH="amd64"
PKG="engram_${VERSION}_${ARCH}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

echo "Building ENGRAM ${VERSION} .deb package..."

# Build release binary
cd "$ROOT"
cargo build --release --bin engram

# Create package structure
rm -rf "/tmp/${PKG}"
mkdir -p "/tmp/${PKG}/DEBIAN"
mkdir -p "/tmp/${PKG}/usr/bin"
mkdir -p "/tmp/${PKG}/etc/engram"
mkdir -p "/tmp/${PKG}/usr/lib/systemd/system"
mkdir -p "/tmp/${PKG}/usr/share/engram/dashboard"

# Control file
cat > "/tmp/${PKG}/DEBIAN/control" <<EOF
Package: engram
Version: ${VERSION}
Architecture: ${ARCH}
Maintainer: ENGRAM Team <engram@manojpisini.com>
Description: ENGRAM — Engineering Intelligence, etched in Notion
 AI-powered engineering intelligence that connects GitHub, Notion,
 and Claude to provide automated code review, security audits,
 performance tracking, and team health monitoring.
Depends: ca-certificates
Section: devel
Priority: optional
Homepage: https://github.com/manojpisini/engram
EOF

# Post-install script
cat > "/tmp/${PKG}/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
# Create engram user if it doesn't exist
if ! getent passwd engram >/dev/null 2>&1; then
    useradd --system --no-create-home --shell /usr/sbin/nologin engram
fi
mkdir -p /var/log/engram
chown engram:engram /var/log/engram
chown -R engram:engram /etc/engram
# Enable and start service
systemctl daemon-reload
systemctl enable engram.service
systemctl start engram.service || true
echo ""
echo "ENGRAM installed! Open http://localhost:3000 to configure."
echo ""
EOF
chmod 755 "/tmp/${PKG}/DEBIAN/postinst"

# Pre-remove script
cat > "/tmp/${PKG}/DEBIAN/prerm" <<'EOF'
#!/bin/sh
set -e
systemctl stop engram.service || true
systemctl disable engram.service || true
EOF
chmod 755 "/tmp/${PKG}/DEBIAN/prerm"

# Copy files
cp "$ROOT/target/release/engram" "/tmp/${PKG}/usr/bin/engram"
cp "$ROOT/engram.toml.example" "/tmp/${PKG}/etc/engram/engram.toml"
cp "$ROOT/packaging/systemd/engram.service" "/tmp/${PKG}/usr/lib/systemd/system/engram.service"
cp -r "$ROOT/dashboard/"* "/tmp/${PKG}/usr/share/engram/dashboard/"

# Build package
dpkg-deb --build "/tmp/${PKG}"
mv "/tmp/${PKG}.deb" "$ROOT/packaging/debian/"

echo "Built: packaging/debian/${PKG}.deb"
