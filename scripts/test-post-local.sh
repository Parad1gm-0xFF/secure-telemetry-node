#!/usr/bin/env bash
# test-post-local.sh - validation locale de bout en bout (sandbox + eBPF).
#
# Usage :
#   ./scripts/test-post-local.sh            # avec docker (build) + sudo
#   ./scripts/test-post-local.sh --no-build # si le binaire existe déjà
#
# Prérequis : docker (build), sudo (supervision eBPF), bpftrace.
# Ce script est fait pour tourner sur le poste de dev x86_64.

set -euo pipefail
cd "$(dirname "$0")/.."

DO_BUILD=1
if [[ "${1:-}" == "--no-build" ]]; then
    DO_BUILD=0
fi

BIN="./target/release/secure-telemetry-node"
PORT=5555
SUPERVISE_SCRIPT="$(pwd)/kernel/ebpf/supervise-stn.bt"

echo "=========================================================="
echo " 1/5 Tests unitaires Rust"
echo "=========================================================="
if [[ "$DO_BUILD" == "1" ]]; then
    docker run --rm -v "$PWD":/work -w /work rust:latest cargo test --release \
        | grep "test result"
    echo "== build release =="
    docker run --rm -v "$PWD":/work -w /work rust:latest cargo build --release \
        | tail -1
else
    echo "build sauté (--no-build)"
fi

echo "=========================================================="
echo " 2/5 Smoke HTTP sous seccomp"
echo "=========================================================="
# NB : le nom de process est tronqué à 15 chars ("secure-telemetr"), donc
# pkill -x ne matche pas. On utilise pkill -f (ligne de commande complète).
pkill -f "secure-telemetry-node" 2>/dev/null || true
sleep 0.3
"$BIN" --port=$PORT > /tmp/stn-smoke.log 2>&1 &
DPID=$!
sleep 1
RESP=$(curl -s --max-time 2 "http://127.0.0.1:$PORT/")
echo "réponse : $RESP"
echo "$RESP" | grep -q "cpu_temp" || { echo "ECHEC : pas de réponse HTTP"; cat /tmp/stn-smoke.log; kill $DPID 2>/dev/null; exit 1; }
grep -q "seccomp filter actif" /tmp/stn-smoke.log || { echo "ECHEC : sandbox inactif"; cat /tmp/stn-smoke.log; kill $DPID 2>/dev/null; exit 1; }
echo "OK : service sandboxé et fonctionnel"
kill $DPID 2>/dev/null || true
wait $DPID 2>/dev/null || true

echo "=========================================================="
echo " 3/5 Self-test seccomp (SIGSYS attendu)"
echo "=========================================================="
set +e
"$BIN" --sandbox-self-test
RC=$?
set -e
if [ "$RC" -eq 132 ] || [ "$RC" -eq 159 ]; then
    echo "OK : l'écriture est refusée par le filtre (SIGSYS, rc=$RC)"
else
    echo "AVERTISSEMENT : seccomp indisponible (rc=$RC)"
fi

echo "=========================================================="
echo " 4/5 Supervision eBPF (bpftrace, sudo) - 10s"
echo "     Pendant ce temps, des requêtes HTTP sont émises."
echo "=========================================================="
command -v bpftrace >/dev/null 2>&1 || { echo "bpftrace manquant : installé ?"; exit 1; }
sudo -n true 2>/dev/null || echo "Note : sudo demandera le mot de passe."

"$BIN" --port=$PORT > /tmp/stn-supervised.log 2>&1 &
SPID=$!
sleep 1
echo "daemon PID: $SPID"

# Supervision en arrière-plan, requêtes HTTP pendant la supervision.
sudo bpftrace "$SUPERVISE_SCRIPT" "$SPID" > /tmp/stn-bpftrace.log 2>&1 &
BTPID=$!
sleep 2
for i in 1 2 3 4 5; do
    curl -s --max-time 2 "http://127.0.0.1:$PORT/" > /dev/null
    sleep 1
done
sleep 2
kill $BTPID 2>/dev/null || true
wait $BTPID 2>/dev/null || true

kill $SPID 2>/dev/null || true
wait $SPID 2>/dev/null || true

echo "== sortie bpftrace (extrait) =="
grep -E "@syscalls|@accept|@recvfrom|@sendto|@openat|@sigsys|SECCOMP|^@\[" /tmp/stn-bpftrace.log | head -25

echo "=========================================================="
echo " 5/5 Bilan"
echo "=========================================================="
SIGSYS_COUNT=$(grep -c "SECCOMP: SIGSYS" /tmp/stn-bpftrace.log 2>/dev/null || echo 0)
echo "SIGSYS observés pendant la supervision : $SIGSYS_COUNT (attendu : 0 en fonctionnement normal)"
echo "Log bpftrace complet : /tmp/stn-bpftrace.log"
echo "Log daemon : /tmp/stn-supervised.log"
echo "Terminé."