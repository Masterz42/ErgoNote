#!/usr/bin/env bash
# Enregistre une sauvegarde de version dans sauvegardes/v<version>/.
# Usage : scripts/sauvegarde-version.sh <version> [options]
# Ne jamais ecraser une sauvegarde existante sans le vouloir.
set -euo pipefail

usage() {
  printf 'Usage : %s <version> [options]\n' "$(basename "$0")" >&2
  printf '  <version>        numero de version, par exemple 1.5 ou 1.5.0\n' >&2
  printf '  --depuis <dir>   dossier des livrables (defaut : racine du depot)\n' >&2
  printf '  --sans-journal   ne pas ecrire dans sauvegardes/JOURNAL.md\n' >&2
  exit "${1:-2}"
}

fail() {
  printf 'Erreur : %s\n' "$1" >&2
  exit 1
}

main() {
  local version=""
  local source_dir=""
  local journal="oui"

  while [ "$#" -gt 0 ]; do
    case "${1:-}" in
      -h|--help)
        usage 0
        ;;
      --depuis)
        [ "$#" -ge 2 ] || fail "l'option --depuis demande un repertoire"
        source_dir="${2:-}"
        shift 2
        ;;
      --sans-journal)
        journal="non"
        shift
        ;;
      -*)
        printf 'Option inconnue : %s\n' "${1:-}" >&2
        usage
        ;;
      *)
        if [ -n "$version" ]; then
          fail "une seule version est acceptee (recu : $version et ${1:-})"
        fi
        version="${1:-}"
        shift
        ;;
    esac
  done

  [ -n "$version" ] || usage
  case "$version" in
    [0-9]*.[0-9]*) ;;
    *) fail "version invalide : $version" ;;
  esac

  local repo_root
  repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  [ -n "$source_dir" ] || source_dir="$repo_root"
  [ -d "$source_dir" ] || fail "repertoire introuvable : $source_dir"

  local dest="$repo_root/sauvegardes/v$version"
  mkdir -p "$dest/android" "$dest/windows" "$dest/linux" "$dest/sources"

  if [ -n "$(find "$dest" -type f ! -name CHECKSUMS.sha256 -print -quit)" ]; then
    printf 'Attention : %s contient deja des fichiers, ils seront remplaces.\n' \
      "$dest" >&2
  fi

  local copied=0
  copy_glob() {
    local pattern="$1"
    local target="$2"
    local file
    while IFS= read -r file; do
      [ -n "$file" ] || continue
      cp -p "$file" "$target/"
      copied=$((copied + 1))
    done < <(compgen -G "$pattern" || true)
  }

  copy_glob "$source_dir/ErgoNote-v$version-Android.apk" "$dest/android"
  copy_glob "$source_dir/ErgoNote-v$version-Windows-x86_64.exe" "$dest/windows"
  copy_glob "$source_dir/ErgoNote-v$version-Linux-x86_64" "$dest/linux"
  copy_glob "$repo_root/dist/android/ergonote-$version*-android-arm64.apk" \
    "$dest/android"

  if [ "$copied" -eq 0 ]; then
    fail "aucun livrable trouve pour la version $version dans $source_dir"
  fi

  (
    cd "$dest"
    find . -type f ! -name CHECKSUMS.sha256 -print0 \
      | sort -z \
      | xargs -0 -r sha256sum > CHECKSUMS.sha256
  )

  local today
  today="$(date +%Y-%m-%d)"

  if [ "$journal" = "oui" ]; then
    local journal_file="$repo_root/sauvegardes/JOURNAL.md"
    local journal_fresh="non"
    if [ ! -f "$journal_file" ]; then
      printf '# Journal des sauvegardes\n\n' > "$journal_file"
      journal_fresh="oui"
    fi
    {
      if [ "$journal_fresh" = "non" ]; then
        printf '\n'
      fi
      printf '## %s - v%s\n\n' "$today" "$version"
      printf -- '- Fichiers enregistres :\n'
      local rel
      while IFS= read -r rel; do
        printf '  - `%s`\n' "$rel"
      done < <(
        cd "$dest"
        find . -type f ! -name CHECKSUMS.sha256 | sort | sed 's|^\./||'
      )
      printf -- '- Manifeste : `sauvegardes/v%s/CHECKSUMS.sha256`\n' "$version"
      printf -- '- Verification : `cd sauvegardes/v%s && sha256sum -c CHECKSUMS.sha256`\n' \
        "$version"
    } >> "$journal_file"
    printf 'Journal : %s\n' "$journal_file"
  fi

  printf 'Sauvegarde v%s : %s fichier(s) dans %s\n' "$version" "$copied" "$dest"
}

main "$@"
