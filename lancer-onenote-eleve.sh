#!/usr/bin/env bash
set -e
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$DIR"
exec "$DIR/target/debug/notecraft" --notebooks "$DIR/mes-classeurs" --expand-all "$@"
