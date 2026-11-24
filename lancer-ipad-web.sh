#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/web-dist" && pwd)"
PORT="${1:-8080}"
IP="$(hostname -I | awk '{print $1}')"

echo "============================================================"
echo " Mon Cartable (Version Web / iPad Safari)"
echo "============================================================"
echo " Ouvre Safari sur ton iPad (connecté au même Wi-Fi) et va sur :"
echo ""
echo "   http://${IP}:${PORT}"
echo ""
echo " Astuce iPad : bouton Partager (carré avec flèche) -> Sur l'écran d'accueil"
echo " Pour arrêter le serveur : Ctrl+C"
echo "============================================================"

cd "${DIR}"
exec python3 -m http.server "${PORT}" --bind 0.0.0.0
