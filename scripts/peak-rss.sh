#!/usr/bin/env bash
# Mede o pico de RAM (soma de RSS dos rustc/cc/ld simultâneos) de um comando de build.
# Uso: scripts/peak-rss.sh cargo build -p rnfe-core --release
set -uo pipefail
"$@" &
pid=$!
peak=0
while kill -0 "$pid" 2>/dev/null; do
  now=$(ps -eo rss=,comm= 2>/dev/null | awk '/rustc|clang|cc1|ld|lld/ {s+=$1} END {print s+0}')
  [ "${now:-0}" -gt "$peak" ] && peak=$now
  sleep 0.2
done
wait "$pid"; rc=$?
echo "pico de RAM (rustc/cc/ld): $((peak / 1024)) MB"
exit $rc
