#!/usr/bin/env bash
# Baixa o APK do último CI verde de `main` e abre o instalador (Termux + termux-api).
# Uso: scripts/apk.sh [run-id]
set -euo pipefail
cd "$(dirname "$0")/.."
id="${1:-$(gh run list --branch main --workflow CI --status success --limit 1 --json databaseId -q '.[0].databaseId')}"
[ -n "$id" ] || { echo "nenhum CI verde encontrado" >&2; exit 1; }
out="${TMPDIR:-/tmp}/rnfe-apk-$id"
rm -rf "$out"
gh run download "$id" -n rnfe-android-apk -D "$out"
apk="$(ls "$out"/*.apk | head -1)"
echo "APK: $apk ($(du -h "$apk" | cut -f1))"
if [ -d "$HOME/storage/downloads" ]; then
  cp "$apk" "$HOME/storage/downloads/rnfe.apk"
  echo "copiado para Downloads/rnfe.apk"
  command -v termux-open >/dev/null && termux-open --view "$HOME/storage/downloads/rnfe.apk" && echo "instalador aberto"
fi
