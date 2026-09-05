#!/usr/bin/env bash
# supervise-stn.sh - lance le daemon secure-telemetry-node puis le supervise
# avec eBPF (bpftrace) : comptage des syscalls, détection des SIGSYS bloqués
# par seccomp, trafic réseau et lectures de capteurs en direct.
#
# Prérequis : bpftrace installé, droits root ou CAP_BPF (le bpf() syscall est
# restreint aux privilégiés sur la plupart des distributions).
#
# Usage :
#   sudo ./supervise-stn.sh [--duration N] [--port N] [binaire-du-daemon]
#   sudo ./supervise-stn.sh --daemon-only        # lance juste le daemon
#
# Exemple :
#   sudo ./supervise-stn.sh --duration 15

set -euo pipefail

DURATION=10
PORT=5555
DAEMON=${DAEMON:-/usr/sbin/secure-telemetry-node}
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BT="$SCRIPT_DIR/supervise-stn.bt"

# --- Arguments simples -----------------------------------------------
while [[ $# -gt 0 ]]; do
    case "$1" in
        --duration) DURATION="$2"; shift 2 ;;
        --port)     PORT="$2"; shift 2 ;;
        --daemon-only) DAEMON_ONLY=1; shift ;;
        *) DAEMON="$1"; shift ;;
    esac
done

# --- Lancement du daemon ---------------------------------------------
"$DAEMON" --port="$PORT" > /tmp/stn-supervised.log 2>&1 &
DPID=$!
echo "daemon PID: $DPID (port $PORT), log: /tmp/stn-supervised.log"
sleep 1

if [[ -n "${DAEMON_ONLY:-}" ]]; then
    echo "mode daemon-only : arrêt avec Ctrl-C"
    wait "$DPID"
    exit 0
fi

# --- Supervision eBPF --------------------------------------------------
echo "== supervision eBPF du daemon pendant ${DURATION}s =="
timeout "$DURATION" bpftrace "$BT" "$DPID" || true

# --- Vérification finale ----------------------------------------------
echo "== réponse HTTP pendant la supervision =="
curl -s --max-time 2 "http://127.0.0.1:$PORT/" && echo

kill "$DPID" 2>/dev/null || true
wait "$DPID" 2>/dev/null || true
echo "== fin =="