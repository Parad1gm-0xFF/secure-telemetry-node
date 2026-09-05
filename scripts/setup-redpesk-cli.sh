#!/usr/bin/env bash
# Installe rp-cli (redpesk-cli) sur Ubuntu/Debian depuis le repo redpesk-ci.
#
# Usage : sudo ./scripts/setup-redpesk-cli.sh
#   MY_DISTRO=xUbuntu_24.04 par défaut (base Ubuntu la plus proche pour 26.04).
#
# Voir : https://docs.redpesk.bzh/docs/en/master/redpesk-factory/rp_cli/1_installation.html
set -euo pipefail

MY_DISTRO="${MY_DISTRO:-xUbuntu_24.04}"
REPO_FILE_PATH="/etc/apt/sources.list.d/redpesk-ci.list"
REPO_KEY_PATH="/usr/share/keyrings/redpesk-tool.gpg"
REPO_URL="https://download.redpesk.bzh/redpesk-ci/armel-update/tools/${MY_DISTRO}"

if [ "$(id -u)" -ne 0 ]; then
  echo "Lancer avec sudo : sudo $0" >&2
  exit 1
fi

echo "== Déclaration du repo redpesk-ci (${MY_DISTRO}) =="
curl -fsSL "$REPO_URL/Release.key" | gpg --dearmor | tee "$REPO_KEY_PATH" >/dev/null
echo "deb [signed-by=$REPO_KEY_PATH] $REPO_URL ./" | tee "$REPO_FILE_PATH" >/dev/null

echo "== apt update =="
apt update

echo "== Installation de redpesk-cli =="
apt-get install -y redpesk-cli

echo "== Vérification =="
rp-cli help 2>&1 | head -5
echo "OK : rp-cli installé."