#!/usr/bin/env bash
# Exécute le binaire ARM (aarch64) du daemon sous QEMU usermode, SANS carte.
# Preuve : l'exécutable cross-compilé tourne sur le poste x86.
#
# Le binaire qemu-aarch64 (usermode) est téléchargé à la volée s'il manque —
# jamais committé dans le repo (10 Mo). Le script reste donc reproductible.
#
# Usage :
#   ./scripts/run-qemu.sh [port]
set -euo pipefail

PORT="${1:-5555}"
BIN="target/aarch64-unknown-linux-musl/release/secure-telemetry-node"
QEMU_DIR="tools"
QEMU="$QEMU_DIR/qemu-aarch64-static"
QEMU_URL="https://github.com/multiarch/qemu-user-static/releases/download/v7.2.0-1/qemu-aarch64-static.tar.gz"

cd "$(dirname "$0")/.."

# 1. Pré-requis : le binaire ARM doit être compilé.
if [[ ! -f "$BIN" ]]; then
  echo "Binaire ARM manquant. Lancer d'abord : ./scripts/build-cross.sh aarch64" >&2
  exit 1
fi

# 2. Télécharger QEMU usermode si absent (hors git, voir .gitignore).
if [[ ! -f "$QEMU" ]]; then
  echo "== Téléchargement de qemu-aarch64 usermode (utilisé hors git) =="
  mkdir -p "$QEMU_DIR"
  curl -sSL -o "$QEMU_DIR/qemu.tar.gz" "$QEMU_URL"
  tar xzf "$QEMU_DIR/qemu.tar.gz" -C "$QEMU_DIR" qemu-aarch64-static
  rm -f "$QEMU_DIR/qemu.tar.gz"
fi

echo "== Cross-compilé ARM (aarch64) exécuté sur $HOSTNAME ($(uname -m)) via qemu-aarch64 usermode =="
"$QEMU" "$BIN" --port="$PORT"