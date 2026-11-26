#!/usr/bin/env bash
set -euo pipefail

# Syncs this workspace to a Mac on the local network over SSH, compiles ErgoNote for
# iOS/iPadOS (`aarch64-apple-ios`), signs the app bundle with Xcode automatic signing,
# and installs + launches it on a connected iPad or iPhone.
#
# Usage:
#   MAC_SSH=user@192.168.1.50 \
#   APPLE_TEAM_ID=XXXXXXXXXX \
#   IOS_DEVICE_ID=00008103-... \
#   bash ios/deploy-from-linux.sh

MAC_SSH="${MAC_SSH:-}"
APPLE_TEAM_ID="${APPLE_TEAM_ID:-}"
IOS_DEVICE_ID="${IOS_DEVICE_ID:-}"
BUNDLE_ID="${BUNDLE_ID:-com.ergonote.app.dev}"
REMOTE_DIR="${REMOTE_DIR:-ErgoNote-ios-build}"

if [[ -z "$MAC_SSH" || -z "$APPLE_TEAM_ID" || -z "$IOS_DEVICE_ID" ]]; then
    cat >&2 <<'EOF'
Missing required environment variables:
  MAC_SSH        SSH target on the Mac (for example user@192.168.1.50)
  APPLE_TEAM_ID  10-character Apple Development Team ID
  IOS_DEVICE_ID  iPad/iPhone UDID from `xcrun devicectl list devices`
Optional:
  BUNDLE_ID      Bundle identifier (default: com.ergonote.app.dev)
  REMOTE_DIR     Remote directory under $HOME on the Mac (default: ErgoNote-ios-build)
EOF
    exit 2
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
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

echo "==> Building staticlib, Xcode bundle, and deploying onto iPad ($IOS_DEVICE_ID)"
ssh -o BatchMode=yes "$MAC_SSH" bash -s -- "$REMOTE_DIR" "$APPLE_TEAM_ID" "$IOS_DEVICE_ID" "$BUNDLE_ID" <<'REMOTE'
set -euo pipefail
REMOTE_DIR="$1"
APPLE_TEAM_ID="$2"
IOS_DEVICE_ID="$3"
BUNDLE_ID="$4"

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
export DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer"

cd "$HOME/$REMOTE_DIR"

mkdir -p ios/App/Assets.xcassets/AppIcon.appiconset
sips -z 1024 1024 assets/brand/ergonote-256.png --out ios/App/Assets.xcassets/AppIcon.appiconset/AppIcon.png >/dev/null

export IPHONEOS_DEPLOYMENT_TARGET=16.0
cargo build --release -p ergonote-ios --target aarch64-apple-ios

DERIVED="$HOME/$REMOTE_DIR/build-ios"
xcodebuild \
    -project ios/ErgoNote.xcodeproj \
    -scheme ErgoNote \
    -configuration Release \
    -sdk iphoneos \
    -derivedDataPath "$DERIVED" \
    -allowProvisioningUpdates \
    DEVELOPMENT_TEAM="$APPLE_TEAM_ID" \
    PRODUCT_BUNDLE_IDENTIFIER="$BUNDLE_ID" \
    build

APP_PATH="$DERIVED/Build/Products/Release-iphoneos/ErgoNote.app"
xcrun devicectl device install app --device "$IOS_DEVICE_ID" "$APP_PATH"
xcrun devicectl device process launch --device "$IOS_DEVICE_ID" "$BUNDLE_ID"
REMOTE
