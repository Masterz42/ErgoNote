#!/usr/bin/env bash
# Installe ErgoNote pour l'utilisateur courant, sans mot de passe et sans root.
#
# Usage : install-mon-cartable.sh [--lancer]
#   --lancer   ouvre ErgoNote une fois l'installation terminee
#
# Le script cherche le binaire ergonote, les icones et le carnet d'accueil :
#   1. dans son propre dossier (archive portable),
#   2. deux dossiers au-dessus (depot de developpement : ErgoNote-*-Linux-x86_64),
#   3. dans target/release/notecraft (build local).
# Il ne touche pas aux carnets : ils restent dans Documents/NoteCraft ou dans le
# dossier deja memorise par l'application.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_ID="com.alientechsoftware.notecraft"
BIN_NAME="ergonote"
LAUNCH=0

for arg in "$@"; do
    case "$arg" in
        --lancer) LAUNCH=1 ;;
        -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
        *) echo "Option inconnue : $arg" >&2; exit 2 ;;
    esac
done

# ---------- trouver le binaire ----------
BIN=""
for base in "$HERE" "$HERE/../.."; do
    if [ -f "$base/$BIN_NAME" ]; then
        BIN="$base/$BIN_NAME"
    elif [ -f "$base/notecraft" ]; then
        BIN="$base/notecraft"
    else
        # Plusieurs versions peuvent cohabiter dans le depot : prendre la plus recente.
        BIN="$(ls -1t "$base"/ErgoNote-v*-Linux-* 2>/dev/null | head -n 1 || true)"
        if [ -z "$BIN" ] && [ -f "$base/target/release/notecraft" ]; then
            BIN="$base/target/release/notecraft"
        fi
    fi
    [ -n "$BIN" ] && break
done
if [ -z "$BIN" ] || [ ! -f "$BIN" ]; then
    echo "Erreur : binaire ErgoNote introuvable." >&2
    echo "Placez ce script dans le dossier qui contient le binaire, puis relancez." >&2
    exit 1
fi

# ---------- dossiers utilisateur ----------
BIN_DIR="$HOME/.local/bin"
APP_DIR="$HOME/.local/share/applications"
ICON_ROOT="$HOME/.local/share/icons/hicolor"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
DEMO_DIR="$DATA_HOME/notecraft/demo"

echo "== Installation d'ErgoNote dans le dossier de l'utilisateur =="
install -d "$BIN_DIR" "$APP_DIR" "$ICON_ROOT"
install -Dm755 "$BIN" "$BIN_DIR/$BIN_NAME"
echo "  programme : $BIN_DIR/$BIN_NAME"

# ---------- icones ----------
for size in 32 64 128 256 512; do
    SRC=""
    for base in "$HERE/icons" "$HERE/../.."; do
        for name in "notecraft-$size.png" "icons/notecraft-$size.png" "assets/brand/notecraft-$size.png"; do
            if [ -f "$base/$name" ]; then
                SRC="$base/$name"
                break
            fi
        done
        [ -n "$SRC" ] && break
    done
    if [ -n "$SRC" ]; then
        install -Dm644 "$SRC" "$ICON_ROOT/${size}x${size}/apps/$APP_ID.png"
    fi
done
echo "  icones    : $ICON_ROOT"

# ---------- carnet d'accueil (Welcome) ----------
DEMO_SRC=""
for base in "$HERE" "$HERE/../.."; do
    if [ -d "$base/demo/welcome-to-notecraft" ]; then
        DEMO_SRC="$base/demo/welcome-to-notecraft"
        break
    fi
    if [ -d "$base/docs/demo/DemoNotebook" ]; then
        DEMO_SRC="$base/docs/demo/DemoNotebook"
        break
    fi
done
if [ -n "$DEMO_SRC" ]; then
    install -d "$DEMO_DIR"
    rm -rf "$DEMO_DIR/.welcome-to-notecraft-new"
    cp -R "$DEMO_SRC" "$DEMO_DIR/.welcome-to-notecraft-new"
    rm -rf "$DEMO_DIR/welcome-to-notecraft"
    mv "$DEMO_DIR/.welcome-to-notecraft-new" "$DEMO_DIR/welcome-to-notecraft"
    echo "  accueil   : $DEMO_DIR/welcome-to-notecraft"
else
    echo "  accueil   : non trouve (le carnet d'accueil sera cree par l'application)"
fi

# ---------- entree de menu ----------
ENTRY="$APP_DIR/$APP_ID.desktop"
cat > "$ENTRY" <<EOF
[Desktop Entry]
Type=Application
Name=ErgoNote
GenericName=Cartable numerique
Comment=Mes cours, mes exercices et mes fiches, dans des dossiers simples
Exec="$BIN_DIR/$BIN_NAME" %U
TryExec=$BIN_DIR/$BIN_NAME
Icon=$APP_ID
Terminal=false
StartupNotify=true
StartupWMClass=$APP_ID
Categories=Office;Education;
MimeType=x-scheme-handler/notecraft;
EOF
chmod 755 "$ENTRY"
echo "  menu      : $ENTRY"

# ---------- rafraichir les caches ----------
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$ICON_ROOT" >/dev/null 2>&1 || true
fi

# ---------- raccourci sur le Bureau ----------
DESKTOP_DIR=""
if command -v xdg-user-dir >/dev/null 2>&1; then
    DESKTOP_DIR="$(xdg-user-dir DESKTOP 2>/dev/null || true)"
fi
if [ -z "$DESKTOP_DIR" ] || [ "$DESKTOP_DIR" = "$HOME" ]; then
    for candidate in "$HOME/Bureau" "$HOME/Desktop" "$HOME/desktop"; do
        if [ -d "$candidate" ]; then
            DESKTOP_DIR="$candidate"
            break
        fi
    done
fi
if [ -n "$DESKTOP_DIR" ]; then
    mkdir -p "$DESKTOP_DIR"
    cp "$ENTRY" "$DESKTOP_DIR/ErgoNote.desktop"
    chmod 755 "$DESKTOP_DIR/ErgoNote.desktop"
    # Cinnamon, GNOME et Nemo demandent ce marqueur avant de lancer un raccourci.
    if command -v gio >/dev/null 2>&1; then
        gio set "$DESKTOP_DIR/ErgoNote.desktop" metadata::trusted true >/dev/null 2>&1 || true
    fi
    echo "  bureau    : $DESKTOP_DIR/ErgoNote.desktop"
fi

echo
echo "Termine. ErgoNote est dans le menu des applications (rubrique Office / Education)."
echo "Si le raccourci du Bureau ne s'ouvre pas, clic droit puis \"Autoriser le lancement\"."
echo "Pour desinstaller : rm -f \"$BIN_DIR/$BIN_NAME\" \"$ENTRY\""
echo

if [ "$LAUNCH" = 1 ]; then
    echo "Ouverture d'ErgoNote..."
    nohup "$BIN_DIR/$BIN_NAME" >/dev/null 2>&1 &
fi
