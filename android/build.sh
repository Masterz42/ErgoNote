#!/usr/bin/env bash
# Builds the ErgoNote Android APK: dist/android/ergonote-<version>-android-arm64.apk
#
#   bash android/build.sh [--skip-build]
#
# Needs (SKILLS/nc-devx/scripts/setup.android.sh installs them): the Rust target
# aarch64-linux-android for the toolchain in rust-toolchain.toml, cargo-ndk, JDK 17 or newer, and
# the Android SDK (ANDROID_HOME or ANDROID_SDK_ROOT, default ~/Android/Sdk) with build-tools,
# the android-35 platform and the NDK.
#
# Signing: with ANDROID_KEYSTORE (a .jks or .p12 file) and ANDROID_KEYSTORE_PASSWORD set, and
# optionally ANDROID_KEY_ALIAS and ANDROID_KEY_PASSWORD, the APK is signed with that key. Without
# them a throwaway key is made for this build only and a warning is printed: such an APK
# installs, but a later build signed with another key cannot update it in place (Android asks to
# uninstall first, which removes the app's notebooks). Releases meant to be updated need the
# same key every time; docs/releasing.md says where it is kept.
set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SKIP_BUILD=0
for arg in "$@"; do
    case "$arg" in
        --skip-build) SKIP_BUILD=1 ;;
        *) echo "usage: $0 [--skip-build]" >&2; exit 2 ;;
    esac
done

VERSION="$(tr -d '\r' < Cargo.toml | sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\([^"]*\)"$/\1/p' | head -n 1)"
[[ -n "$VERSION" ]] || { echo "could not read the workspace version from Cargo.toml" >&2; exit 1; }

# Android needs a whole number that grows with every release: MAJOR, MINOR and PATCH in three
# digits each (0.0.10 is 10, 0.1.0 is 1000, 1.2.3 is 1002003). A pre-release tag is not counted.
IFS=. read -r MAJOR MINOR PATCH <<<"${VERSION%%-*}"
VERSION_CODE=$((MAJOR * 1000000 + MINOR * 1000 + PATCH))
(( VERSION_CODE > 0 )) || { echo "version $VERSION gives no Android version code" >&2; exit 1; }

MIN_SDK=26
TARGET_SDK=35
ABI=arm64-v8a
TARGET=aarch64-linux-android
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"
BUILD_TOOLS_VERSION="${ARTCRAFT_BUILD_TOOLS_VERSION:-35.0.0}"
BUILD_TOOLS="$SDK/build-tools/$BUILD_TOOLS_VERSION"
PLATFORM="$SDK/platforms/android-$TARGET_SDK/android.jar"
for tool in aapt2 d8 zipalign apksigner; do
    [[ -x "$BUILD_TOOLS/$tool" ]] || { echo "missing $BUILD_TOOLS/$tool; run SKILLS/nc-devx/scripts/setup.android.sh" >&2; exit 1; }
done
[[ -f "$PLATFORM" ]] || { echo "missing $PLATFORM; run SKILLS/nc-devx/scripts/setup.android.sh" >&2; exit 1; }
for tool in javac python3 keytool; do
    command -v "$tool" >/dev/null || { echo "missing $tool" >&2; exit 1; }
done

DIST="${DIST:-$ROOT/dist/android}"
WORK="$ROOT/target/android-apk"
NAME="ergonote-$VERSION-android-arm64"
rm -rf "$WORK"
mkdir -p "$WORK" "$DIST"

# 1. The library.
if [[ "$SKIP_BUILD" == 0 ]]; then
    command -v cargo-ndk >/dev/null || { echo "missing cargo-ndk; run SKILLS/nc-devx/scripts/setup.android.sh" >&2; exit 1; }
    if [[ -z "${ANDROID_NDK_HOME:-}" ]]; then
        NDK_VERSION="${ARTCRAFT_NDK_VERSION:-28.2.13676358}"
        export ANDROID_NDK_HOME="$SDK/ndk/$NDK_VERSION"
    fi
    cargo ndk --target "$ABI" --platform "$MIN_SDK" build --release --locked -p ergonote-android
fi
LIB="$ROOT/target/$TARGET/release/libergonote_android.so"
[[ -s "$LIB" ]] || { echo "missing $LIB; build without --skip-build" >&2; exit 1; }

# 2. Fetch Google ML Kit Text Recognition (Latin/French bundled offline) & stage its assets.
MLKIT="$WORK/mlkit"
python3 "$ROOT/android/fetch_mlkit.py" "$MLKIT"
mkdir -p "$WORK/assets"
if [[ -d "$MLKIT/assets" ]] && [[ -n "$(ls -A "$MLKIT/assets" 2>/dev/null)" ]]; then
    cp -R "$MLKIT/assets/." "$WORK/assets/"
fi

# 3. Resources: the theme and the launcher icon (the app icon, assets/brand/ergonote-256.png).
mkdir -p "$WORK/res"
cp -R "$ROOT/android/res/." "$WORK/res/"
mkdir -p "$WORK/res/mipmap-xxxhdpi"
cp "$ROOT/assets/brand/ergonote-256.png" "$WORK/res/mipmap-xxxhdpi/ic_launcher.png"
"$BUILD_TOOLS/aapt2" compile --dir "$WORK/res" -o "$WORK/resources.zip"
"$BUILD_TOOLS/aapt2" link \
    -o "$WORK/unaligned.apk" \
    -I "$PLATFORM" \
    --manifest "$ROOT/android/AndroidManifest.xml" \
    --min-sdk-version "$MIN_SDK" \
    --target-sdk-version "$TARGET_SDK" \
    --version-code "$VERSION_CODE" \
    --version-name "$VERSION" \
    -0 fb -0 binarypb -0 pb -0 tflite \
    -A "$WORK/assets" \
    "$WORK/resources.zip"

# 3b. Java NativeActivity subclass (SAF, camera EXIF, TTS, Google ML Kit OCR, Gboard IME).
mkdir -p "$WORK/java-classes" "$WORK/dex"
MLKIT_CP="$(printf '%s:' "$MLKIT/jars"/*.jar)"
javac -Xlint:-options -source 8 -target 8 -bootclasspath "$PLATFORM" -cp "$MLKIT_CP" -d "$WORK/java-classes" \
    "$ROOT/android/java/com/ergonote/app/ErgoNoteActivity.java"
"$BUILD_TOOLS/d8" --release --min-api "$MIN_SDK" --lib "$PLATFORM" \
    --output "$WORK/dex" "$WORK/java-classes"/com/ergonote/app/*.class "$MLKIT/jars"/*.jar

# 4. Native shared libraries (`libergonote_android.so` + `libmlkit_google_ocr_pipeline.so`) + dex files.
NDK_HOME="${ANDROID_NDK_HOME:-$SDK/ndk/${ARTCRAFT_NDK_VERSION:-28.2.13676358}}"
STRIP="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip"
[[ -x "$STRIP" ]] || { echo "missing $STRIP; set ANDROID_NDK_HOME" >&2; exit 1; }
mkdir -p "$WORK/apk/lib/$ABI"
"$STRIP" --strip-all -o "$WORK/apk/lib/$ABI/libergonote_android.so" "$LIB"
for extra_so in "$MLKIT/jni/$ABI"/*.so; do
    [[ -f "$extra_so" ]] && cp "$extra_so" "$WORK/apk/lib/$ABI/"
done
(cd "$WORK/apk" && python3 - "$WORK/unaligned.apk" "$WORK/apk/lib/$ABI" "$WORK/dex" <<'PY'
import glob, os, sys, zipfile
apk, lib_dir, dex_dir = sys.argv[1], sys.argv[2], sys.argv[3]
abi = os.path.basename(lib_dir)
with zipfile.ZipFile(apk, "a") as archive:
    for dex in sorted(glob.glob(os.path.join(dex_dir, "classes*.dex"))):
        archive.write(dex, os.path.basename(dex), compress_type=zipfile.ZIP_STORED)
    for so in sorted(glob.glob(os.path.join(lib_dir, "*.so"))):
        archive.write(so, f"lib/{abi}/{os.path.basename(so)}", compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
PY
)

# 5. Align and sign (v1 to v3 signatures; no separate v4 .idsig file).
"$BUILD_TOOLS/zipalign" -P 16 -f 4 "$WORK/unaligned.apk" "$WORK/aligned.apk"
KEYSTORE="${ANDROID_KEYSTORE:-}"
if [[ -n "$KEYSTORE" ]]; then
    [[ -f "$KEYSTORE" ]] || { echo "ANDROID_KEYSTORE is set but $KEYSTORE does not exist" >&2; exit 1; }
    [[ -n "${ANDROID_KEYSTORE_PASSWORD:-}" ]] || { echo "ANDROID_KEYSTORE is set but ANDROID_KEYSTORE_PASSWORD is not" >&2; exit 1; }
    STORE_PASS="$ANDROID_KEYSTORE_PASSWORD"
    KEY_PASS="${ANDROID_KEY_PASSWORD:-$ANDROID_KEYSTORE_PASSWORD}"
    ALIAS_ARGS=()
    [[ -n "${ANDROID_KEY_ALIAS:-}" ]] && ALIAS_ARGS=(--ks-key-alias "$ANDROID_KEY_ALIAS")
    echo "signing with the release key in ANDROID_KEYSTORE"
else
    KEYSTORE="$WORK/throwaway.p12"
    STORE_PASS="$(python3 -c 'import secrets; print(secrets.token_hex(16))')"
    KEY_PASS="$STORE_PASS"
    ALIAS_ARGS=(--ks-key-alias throwaway)
    keytool -genkeypair -keystore "$KEYSTORE" -storetype PKCS12 -storepass "$STORE_PASS" -keypass "$KEY_PASS" \
        -alias throwaway -keyalg RSA -keysize 3072 -validity 10000 -dname "CN=ErgoNote build $VERSION" >/dev/null 2>&1
    echo "warning: no ANDROID_KEYSTORE; signed with a throwaway key made for this build only." >&2
    echo "warning: a build signed with any other key cannot update this one in place." >&2
fi
# Passwords go through the environment, never the command line.
ERGONOTE_STORE_PASS="$STORE_PASS" ERGONOTE_KEY_PASS="$KEY_PASS" "$BUILD_TOOLS/apksigner" sign \
    --ks "$KEYSTORE" --ks-pass env:ERGONOTE_STORE_PASS --key-pass env:ERGONOTE_KEY_PASS "${ALIAS_ARGS[@]}" \
    --min-sdk-version "$MIN_SDK" --v1-signing-enabled true --v2-signing-enabled true --v3-signing-enabled true --v4-signing-enabled false --out "$DIST/$NAME.apk" "$WORK/aligned.apk"
rm -f "$WORK/throwaway.p12"
"$BUILD_TOOLS/apksigner" verify --min-sdk-version "$MIN_SDK" "$DIST/$NAME.apk"
echo "wrote $DIST/$NAME.apk (version $VERSION, code $VERSION_CODE)"
