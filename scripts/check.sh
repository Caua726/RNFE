#!/usr/bin/env bash
# Portão de tarefa: formata, lint com -D warnings e roda os testes do núcleo.
# Uso: scripts/check.sh [args extras para cargo test]   (ex.: --test nestest)
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all
cargo clippy -p rnfe-core -p rnfe-frontend -p rnfe-tty --all-targets --features rnfe-core/serde -- -D warnings
cargo test -p rnfe-core -p rnfe-frontend --features rnfe-core/serde "$@"
