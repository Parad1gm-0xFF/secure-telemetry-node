#!/usr/bin/env bash
# Écrit l'image Yocto redpesk/secure-node sur une carte SD Raspberry Pi 3B+.
# À utiliser avec une image produite par la layer meta-secure-node.
# ⚠ ATTENTION : écrit sur un vrai périphérique de bloc. Vérifier le device !
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 <image-file> <device: /dev/sdX>"
  echo "ex :  $0 build/tmp/deploy/images/raspberrypi3-64/secure-node-image-raspberrypi3-64.rootfs.wic /dev/sdc"
  exit 1
fi

IMG="$1"
DEV="$2"

[[ -f "$IMG" ]] || { echo "image introuvable : $IMG"; exit 1; }
[[ -b "$DEV" ]] || { echo "$DEV n'est pas un périphérique bloc."; exit 1; }
echo "⚠ Pression une fois sur Entrée pour écraser $DEV ($(blockdev --getsize64 "$DEV" 2>/dev/null | awk '{print $1/1024/1024/1024" GiB"}' || echo "?"))"
read -r

echo "== écriture (dd, pv optionnel) =="
if command -v pv >/dev/null; then
  pv "$IMG" | dd of="$DEV" bs=4M conv=fsync status=none
else
  dd if="$IMG" of="$DEV" bs=4M conv=fsync status=progress
fi
echo "== rescan partitions =="
sync
if command -v partprobe >/dev/null; then partprobe "$DEV" || true; fi
echo "OK. Insérez la carte dans la RPi3B+ et démarrez (console tty / SSH)."