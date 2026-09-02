#!/usr/bin/env bash
# Cross-compile le daemon pour plusieurs cibles embarquées depuis un hôte x86.
# Démontre la portabilité demandée (aarch64 + riscv64), via Docker musl-cross.
#
# Usage :
#   ./scripts/build-cross.sh aarch64|riscv64|all
set -euo pipefail

IMG_AARCH64="ghcr.io/rust-cross/rust-musl-cross:aarch64-musl"
IMG_RISCV64="ghcr.io/rust-cross/rust-musl-cross:riscv64gc-musl"
cd "$(dirname "$0")/.."

build() {
  local target="$1" img="$2"
  echo "== Cross-compile $target == "
  docker run --rm -v "$PWD":/work -w /work "$img" \
    cargo build --release --target "$target" 2>&1 | tail -6
  BIN="target/$target/release/secure-telemetry-node"
  [[ -f "$BIN" ]] && { echo "-- $BIN --"; file "$BIN"; ls -lh "$BIN" | awk '{print "size:",$5}'; }
}

case "${1:-all}" in
  aarch64) build aarch64-unknown-linux-musl "$IMG_AARCH64" ;;
  riscv64) build riscv64gc-unknown-linux-musl "$IMG_RISCV64" ;;
  all)     build aarch64-unknown-linux-musl "$IMG_AARCH64"
           build riscv64gc-unknown-linux-musl "$IMG_RISCV64" ;;
  *) echo "usage: $0 [aarch64|riscv64|all]"; exit 1 ;;
esac