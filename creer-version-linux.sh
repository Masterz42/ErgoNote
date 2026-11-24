#!/usr/bin/env bash
# Construit la version Linux d'ErgoNote et un paquet facile a installer.
#
# Usage : creer-version-linux.sh [--sans-compilation]
#
# Produit :
#   ErgoNote-v<version>-Linux-<arch>                  le binaire seul (comme avant)
#   dist/linux/ErgoNote-v<version>-Linux-<arch>/      dossier portable (double-clic)
#   dist/linux/ErgoNote-v<version>-Linux-<arch>.tar.gz
#   dist/linux/ergonote_<version>_<arch>.deb          paquet Debian/Ubuntu/Mint
#   dist/linux/SHA256SUMS
#
# Le paquet .deb s'installe en double-cliquant dessus (GDebi, Discover, aptinstall)
# et place ErgoNote dans le menu. L'archive .tar.gz contient install-mon-cartable.sh
# pour une installation sans root dans le dossier de l'utilisateur.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERSION="$(tr -d '\r' < "$ROOT/Cargo.toml" | sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\([^"]*\)"$/\1/p' | head -n 1)"
[ -n "$VERSION" ] || { echo "impossible de lire la version dans Cargo.toml" >&2; exit 1; }

ARCH="$(uname -m)"
case "$ARCH" in
    x86_64) DEB_ARCH=amd64 ;;
    aarch64|arm64) ARCH=aarch64; DEB_ARCH=arm64 ;;
    *) echo "architecture Linux non prise en charge : $ARCH" >&2; exit 2 ;;
esac

BUILD=1
for arg in "$@"; do
    case "$arg" in
        --sans-compilation) BUILD=0 ;;
        -h|--help) sed -n '2,17p' "$0"; exit 0 ;;
        *) echo "option inconnue : $arg" >&2; exit 2 ;;
    esac
done

DIST="$ROOT/dist/linux"
WORK="${CARGO_TARGET_DIR:-$ROOT/target}/linux-easy-install"
# Les fichiers publies portent la version courte, comme les versions precedentes
# (1.5.0 se nomme v1.5) ; le paquet .deb garde la version complete.
FILE_VERSION="$VERSION"
case "$FILE_VERSION" in
    *[0-9].0) FILE_VERSION="${FILE_VERSION%.0}" ;;
esac
BASENAME="ErgoNote-v$FILE_VERSION-Linux-$ARCH"
BIN="$ROOT/target/release/notecraft"
DEMO="$ROOT/docs/demo/DemoNotebook"

echo "== ErgoNote $VERSION pour Linux $ARCH =="
if [ "$BUILD" = 1 ]; then
    (cd "$ROOT" && cargo build --release --locked -p notecraft)
fi
[ -x "$BIN" ] || { echo "binaire release introuvable : $BIN" >&2; exit 1; }
[ -f "$DEMO/html/_node.toml" ] && [ -f "$DEMO/cmgfm/_node.toml" ] || {
    echo "carnet d'accueil manquant : $DEMO (html et cmgfm)" >&2
    exit 1
}

# Le binaire seul, au meme endroit que les versions precedentes.
cp -p "$BIN" "$ROOT/ErgoNote-v$FILE_VERSION-Linux-$ARCH"
echo "binaire : $ROOT/ErgoNote-v$FILE_VERSION-Linux-$ARCH"

rm -rf "$DIST" "$WORK"
mkdir -p "$DIST" "$WORK"

# ---------- dossier portable ----------
PORT="$DIST/$BASENAME"
install -d "$PORT/icons" "$PORT/demo"
install -m755 "$BIN" "$PORT/ergonote"
install -m755 "$ROOT/packaging/linux/install-mon-cartable.sh" "$PORT/install-mon-cartable.sh"
for size in 32 64 128 256 512; do
    install -m644 "$ROOT/assets/brand/notecraft-$size.png" "$PORT/icons/notecraft-$size.png"
done
cp -R "$DEMO" "$PORT/demo/welcome-to-notecraft"
cat > "$PORT/LISEZ-MOI.txt" <<'EOF'
ErgoNote - version portable Linux
=================================

Deux facons de l'installer :

1. Sans mot de passe (recommande) :
   double-cliquez sur install-mon-cartable.sh, ou ouvrez un terminal ici et lancez
       bash install-mon-cartable.sh --lancer
   Le programme va dans ~/.local/bin, le menu et le Bureau recoivent un raccourci.

2. Sans rien installer :
   double-cliquez sur ergonote. Le programme et le carnet d'accueil restent dans
   ce dossier. Pour le deplacer, deplacez le dossier entier.

Les carnets sont enregistres dans Documents/NoteCraft. Ils ne sont jamais
supprimes par une mise a jour.
EOF

tar -C "$DIST" -czf "$DIST/$BASENAME.tar.gz" "$BASENAME"
echo "archive : $DIST/$BASENAME.tar.gz"

# ---------- paquet .deb ----------
if command -v dpkg-deb >/dev/null 2>&1; then
    DEB_ROOT="$WORK/deb"
    install -d "$DEB_ROOT/DEBIAN" \
        "$DEB_ROOT/usr/bin" \
        "$DEB_ROOT/usr/share/applications" \
        "$DEB_ROOT/usr/share/doc/ergonote" \
        "$DEB_ROOT/usr/share/notecraft/demo"
    install -m755 "$BIN" "$DEB_ROOT/usr/bin/ergonote"
    for size in 32 64 128 256 512; do
        install -Dm644 "$ROOT/assets/brand/notecraft-$size.png" \
            "$DEB_ROOT/usr/share/icons/hicolor/${size}x${size}/apps/com.alientechsoftware.notecraft.png"
    done
    cp -R "$DEMO/." "$DEB_ROOT/usr/share/notecraft/demo/welcome-to-notecraft/"
    install -m644 "$ROOT/LICENSE" "$DEB_ROOT/usr/share/doc/ergonote/LICENSE"
    install -m644 "$ROOT/NOTICE" "$DEB_ROOT/usr/share/doc/ergonote/NOTICE"

    cat > "$DEB_ROOT/usr/share/applications/com.alientechsoftware.notecraft.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=ErgoNote
GenericName=Cartable numerique
Comment=Mes cours, mes exercices et mes fiches, dans des dossiers simples
Exec=ergonote %U
TryExec=ergonote
Icon=com.alientechsoftware.notecraft
Terminal=false
StartupNotify=true
StartupWMClass=com.alientechsoftware.notecraft
Categories=Office;Education;
MimeType=x-scheme-handler/notecraft;
EOF
    chmod 644 "$DEB_ROOT/usr/share/applications/com.alientechsoftware.notecraft.desktop"

    SIZE_KB="$(du -sk "$DEB_ROOT/usr" | awk '{print $1}')"
    cat > "$DEB_ROOT/DEBIAN/control" <<EOF
Package: ergonote
Version: $VERSION
Architecture: $DEB_ARCH
Maintainer: ErgoLib <ergolib@example.invalid>
Installed-Size: $SIZE_KB
Depends: libc6 (>= 2.35), libgcc-s1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libx11-6, libxcursor1, libxi6, libvulkan1 | libegl1
Recommends: mesa-vulkan-drivers, libegl-mesa0, xdg-desktop-portal
Section: education
Priority: optional
Homepage: https://github.com/Masterz42/ErgoNote
Description: ErgoNote - cartable numerique adapte
 Notes, cours et exercices en fichiers simples, dans des dossiers.
 .
 Le paquet installe le programme, l'icone, l'entree de menu et le carnet
 d'accueil. Les carnets de l'utilisateur restent dans Documents/NoteCraft.
EOF
    chmod 644 "$DEB_ROOT/DEBIAN/control"

    if dpkg-deb --help 2>&1 | grep -q -- '--root-owner-group'; then
        dpkg-deb --root-owner-group --build "$DEB_ROOT" "$DIST/ergonote_${VERSION}_${DEB_ARCH}.deb" >/dev/null
    else
        dpkg-deb --build "$DEB_ROOT" "$DIST/ergonote_${VERSION}_${DEB_ARCH}.deb" >/dev/null
    fi
    echo "paquet  : $DIST/ergonote_${VERSION}_${DEB_ARCH}.deb"
else
    echo "dpkg-deb absent : paquet .deb non cree (archive portable disponible)" >&2
fi

# ---------- empreintes ----------
(
    cd "$DIST"
    find . -maxdepth 2 -type f \( -name '*.tar.gz' -o -name '*.deb' \) -print0 \
        | sort -z | xargs -0 -r sha256sum > SHA256SUMS
)
echo "empreintes : $DIST/SHA256SUMS"

"$BIN" --version
echo "== termine : $DIST =="
