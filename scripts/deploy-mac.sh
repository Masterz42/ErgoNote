#!/usr/bin/env bash
set -euo pipefail

# Syncs the workspace to a Mac over SSH, builds ErgoNote natively for macOS,
# packages ErgoNote.app, installs it in /Applications (or ~/Applications), and launches it.
#
# Usage:
#   MAC_SSH=user@192.168.1.50 bash scripts/deploy-mac.sh

MAC_SSH="${MAC_SSH:-}"
REMOTE_DIR="${REMOTE_DIR:-ErgoNote-ios-build}"

if [[ -z "$MAC_SSH" ]]; then
    cat >&2 <<'EOF'
Missing required environment variable:
  MAC_SSH     SSH target on the Mac (for example user@192.168.1.50)
Optional:
  REMOTE_DIR  Remote directory under $HOME on the Mac (default: ErgoNote-ios-build)
EOF
    exit 2
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_SHA="$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo local)"

echo "==> Syncing repository to $MAC_SSH:~/$REMOTE_DIR"
rsync -az --delete \
    --exclude='target/' \
    --exclude='dist/' \
    --exclude='.git/' \
    --exclude='*.apk' \
    --exclude='*.exe' \
    --exclude='ErgoNote-v*' \
    --exclude='mes-classeurs/' \
    --exclude='my-notebooks/' \
    --exclude='sauvegardes/' \
    --exclude='build-ios/' \
    "$ROOT/" "$MAC_SSH:$REMOTE_DIR/"

echo "==> Building and installing ErgoNote.app on $MAC_SSH"
ssh -o BatchMode=yes "$MAC_SSH" bash -s -- "$REMOTE_DIR" "$BUILD_SHA" <<'REMOTE'
set -euo pipefail
REMOTE_DIR="$1"
BUILD_SHA="$2"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
export DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer"
export MACOSX_DEPLOYMENT_TARGET=14.0

cd "$HOME/$REMOTE_DIR"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
BUILD_CODE="$(printf '%s' "$VERSION" | tr -cd '0-9')"

CLANG_RT="$(clang --print-libgcc-file-name)"
RUSTFLAGS="-C link-arg=$CLANG_RT" cargo build --release -p ergonote

STAGE="$(mktemp -d /tmp/ergonote-mac-stage.XXXXXX)"
cleanup() { rm -rf "$STAGE"; }
trap cleanup EXIT

APP="$STAGE/ErgoNote.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/ergonote "$APP/Contents/MacOS/ergonote"
chmod 0755 "$APP/Contents/MacOS/ergonote"

ICONSET="$STAGE/ergonote.iconset"
mkdir -p "$ICONSET"
for sz in 16 32 128 256 512; do
    sips -z "$sz" "$sz" assets/brand/ergonote-256.png --out "$ICONSET/icon_${sz}x${sz}.png" >/dev/null
    sz2=$((sz * 2))
    sips -z "$sz2" "$sz2" assets/brand/ergonote-256.png --out "$ICONSET/icon_${sz}x${sz}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/ergonote.icns"

sed \
    -e "s|@VERSION@|$VERSION|g" \
    -e "s|@BUILD@|$BUILD_CODE|g" \
    -e "s|@FULL_VERSION@|$VERSION|g" \
    -e "s|@BUILD_SHA@|$BUILD_SHA|g" \
    packaging/OSX/Info.plist.in > "$APP/Contents/Info.plist"

codesign --force --deep --sign - "$APP"

if [[ -w /Applications ]]; then
    DEST_DIR="/Applications"
else
    DEST_DIR="$HOME/Applications"
    mkdir -p "$DEST_DIR"
fi
DEST_APP="$DEST_DIR/ErgoNote.app"
pkill -x ergonote 2>/dev/null || true
rm -rf "$DEST_APP.new" "$DEST_APP.old"
ditto "$APP" "$DEST_APP.new"
if [[ -d "$DEST_APP" ]]; then
    mv "$DEST_APP" "$DEST_APP.old"
fi
mv "$DEST_APP.new" "$DEST_APP"
rm -rf "$DEST_APP.old"

"$DEST_APP/Contents/MacOS/ergonote" --version
echo "installed: $DEST_APP"
open -a "$DEST_APP" || true
REMOTE
