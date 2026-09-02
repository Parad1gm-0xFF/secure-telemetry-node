# Spec RPM pour le noeud de telemetrie — variante "build from source"
#
# VARIANTE REDPESK FACTORY. Contrairement a spec/secure-telemetry-node.spec
# (qui empaquette un binaire pre-compile), ce spec COMPILE le code source
# avec Cargo dans la factory redpesk. Il est donc utilisable tel quel en
# projet "standard" redpesk : la factory clone le depot GitHub, execute
# cargo build pour l'architecture cible, puis empaquette le resultat en RPM.
#
# Build arche-types :
#   - aarch64 : cognitely pour RPi3B+/redpesk OS
#   - x86_64  : pour la cible de test redpesk (QEMU virtuel)
#
# Le projet est conçu pour compiler en statique musl, mais la factory redpesk
# construit pour SON environnement (CentOS/redpesk). On ne force donc pas musl
# ici : on compile simplement le binaire natif de l'arch cible choisie, via le
# toolchain Cargo fourni par l'infrastructure.

# --- Désactivation des paquets -debuginfo / -debugsource ---
# Le binaire Rust n'a pas besoin de debuginfo ; éviter la génération automatique
# de "-debugsource" évite l'erreur rpm "Empty %files ... debugsourcefiles.list".
%global _enable_debug_packages 0

Name:           secure-telemetry-node
Version:        0.1.0
Release:        1%{?dist}
Summary:        Secure telemetry node daemon (Rust, source build)

License:        Apache-2.0
URL:            https://github.com/Parad1gm-0xFF/secure-telemetry-node
Source0:        %{name}-%{version}.tar.gz

# La factory fournit le toolchain Rust (cargo). Pas de recompilation musl ici.
BuildRequires:  cargo
BuildRequires:  rust

# Binaire Rust : pas de paquet -debuginfo/-debugsource a generer (economise
# du build et evite l'erreur "Empty %files ... debugsourcefiles.list").
%global debug_package %{nil}

# BuildArch: on ne le fixe pas => la factory utilise l'arch cible du projet
# (aarch64 ou x86_64 selon le choix dans l'UI redpesk).

%description
Demon de telemetrie securise pour Linux embarque, compile A LA SOURCE par la
redpesk factory. Fournit un petit microservice TCP (esprit afb-binder) exposant
temperature CPU, etat GPIO et memoire libre. Securise des le build : sandbox
seccomp, unite systemd durcie.

%prep
%autosetup

%build
# Compilation Rust cible par la factory (arch du projet).
cargo build --release --locked \
    --manifest-path %{_builddir}/%{name}-%{version}/Cargo.toml

%install
install -D -m 0755 %{_builddir}/%{name}-%{version}/target/release/secure-telemetry-node \
    %{buildroot}%{_sbindir}/secure-telemetry-node
install -D -m 0644 %{_builddir}/%{name}-%{version}/packaging/secure-telemetry-node.service \
    %{buildroot}%{_unitdir}/secure-telemetry-node.service

%check
# Test non bloquant : on lance le daemon, on interroge son endpoint HTTP
# (télémétrie), puis on l'arrête. --help n'existe pas (le daemon écoute en
# boucle) donc on teste le vrai comportement de service.
cd %{_builddir}/%{name}-%{version}
# Port élevé pour éviter tout conflit pendant le test.
PORT_T=5599 ./target/release/secure-telemetry-node --port=5599 \
    > /tmp/stn-test.log 2>&1 &
PID=$!
sleep 1
# Le daemon expose un microservice HTTP : on vérifie qu'il répond.
if curl -sf http://127.0.0.1:5599/ >/dev/null 2>&1; then
    echo "secure-telemetry-node: test OK (daemon répond)";
    kill "$PID" 2>/dev/null || true;
else
    echo "secure-telemetry-node: ECHEC — daemon ne répond pas";
    cat /tmp/stn-test.log 2>/dev/null;
    kill "$PID" 2>/dev/null || true;
    exit 1;
fi

%files
%{_sbindir}/secure-telemetry-node
%{_unitdir}/secure-telemetry-node.service

%post
systemctl daemon-reload || true

%changelog
* Wed Sep 02 2026 Parad1gm <parad1gm_0xFF@gmail.com> - 0.1.0-1
- Variante redpesk factory : compilation a la source via Cargo.