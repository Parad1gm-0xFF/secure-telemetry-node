#!/usr/bin/env bash
# session-log.sh - exécute une commande et la journalise (commande + sortie +
# code retour + horodatage) dans un fichier de session suivi en temps réel.
#
# Usage :
#   ./scripts/session-log.sh "<description>" <commande...>
#
# Exemple :
#   ./scripts/session-log.sh "rebuild release" cargo build --release
#
# Suivi en temps réel (dans un second terminal) :
#   tail -f "${STN_SESSION_LOG:-/tmp/stn-session.log}"
#
# Variables :
#   STN_SESSION_LOG : chemin du journal (défaut /tmp/stn-session.log)

set -uo pipefail

LOG="${STN_SESSION_LOG:-/tmp/stn-session.log}"
DESC="${1:-}"; shift

[ $# -gt 0 ] || { echo "usage: session-log.sh \"<description>\" <commande...>" >&2; exit 2; }

sep="────────────────────────────────────────────────────────────"
{
    echo ""
    echo "$sep"
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $DESC"
    echo "\$ $*"
} >> "$LOG"

# Sortie streamée en temps réel vers le terminal ET le journal (tee -a),
# ligne à ligne pour que tail -f suive au fil de l'eau.
bash -o pipefail -c "$*" 2>&1 | stdbuf -oL tee -a "$LOG"
RC=${PIPESTATUS[0]}

echo "[rc=$RC $(date '+%H:%M:%S')]" >> "$LOG"
exit "$RC"