#!/usr/bin/env bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/web-dist" && pwd)"
PORT="${1:-8080}"

if ! command -v cloudflared >/dev/null 2>&1 && [[ -x "${HOME}/.local/bin/cloudflared" ]]; then
  CLOUDFLARED="${HOME}/.local/bin/cloudflared"
else
  CLOUDFLARED="cloudflared"
fi

python3 -m http.server "${PORT}" --bind 127.0.0.1 --directory "${DIR}" >/tmp/notecraft-web.log 2>&1 &
HTTP_PID=$!
trap 'kill "${HTTP_PID}" 2>/dev/null || true' EXIT

echo "Serveur local démarré sur 127.0.0.1:${PORT}, ouverture du tunnel public HTTPS..."
exec "${CLOUDFLARED}" tunnel --url "http://127.0.0.1:${PORT}" --no-autoupdate
