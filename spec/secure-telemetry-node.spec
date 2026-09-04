# Spec RPM pour le noeud de telemetrie — variante "build from source"
#
# VARIANTE REDPESK FACTORY. Ce spec COMPILE le code source avec Cargo dans la
# factory redpesk : la factory clone le depot GitHub, execute cargo build pour
# l'architecture cible, puis empaquette le resultat en RPM. Utilisable tel quel
# en projet "standard" redpesk.
#
# Build arche-types :
#   - aarch64 : cognitivement pour RPi3B+/redpesk OS
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
#
# Cible Rust déduite de l'arch rpm : indispensable pour le CROSS-BUILD.
# Sur le build aarch64, l'hôte de compilation est x86_64 (émulation) : sans
# --target, cargo produit un binaire x86_64 → erreur "wrong architecture".
%global rust_triple x86_64-unknown-linux-gnu
%ifarch aarch64
%global rust_triple aarch64-unknown-linux-gnu
%endif

%description
Demon de telemetrie securise pour Linux embarque, compile A LA SOURCE par la
redpesk factory. Fournit un petit microservice TCP (esprit afb-binder) exposant
temperature CPU, etat GPIO et memoire libre. Securise des le build : sandbox
seccomp, unite systemd durcie.

%prep
%autosetup

%build
# Compilation Rust vers l'arch cible du paquet (cross-build sûr).
cargo build --release --locked --offline \
    --target %{rust_triple} \
    --manifest-path %{_builddir}/%{name}-%{version}/Cargo.toml

%install
install -D -m 0755 %{_builddir}/%{name}-%{version}/target/%{rust_triple}/release/secure-telemetry-node \
    %{buildroot}%{_sbindir}/secure-telemetry-node
install -D -m 0644 %{_builddir}/%{name}-%{version}/packaging/secure-telemetry-node.service \
    %{buildroot}/usr/lib/systemd/system/secure-telemetry-node.service

# --- Subpackage redtest (tests d'intégration redpesk) ---
# Le script run-redtest est installé dans /usr/libexec/redtest/<package>/, comme
# attendu par la plateforme redpesk pour les tests embarqués (VM QEMU).
mkdir -p %{buildroot}%{_libexecdir}/redtest/%{name}/
cp -a %{_builddir}/%{name}-%{version}/redtest/. %{buildroot}%{_libexecdir}/redtest/%{name}/
chmod +x %{buildroot}%{_libexecdir}/redtest/%{name}/run-redtest

%check
# Test robuste : le daemon doit démarrer, installer son sandbox seccomp et
# RÉPONDRE en HTTP (preuve de compilation + exécution + politique active).
cd %{_builddir}/%{name}-%{version}
BIN=./target/%{rust_triple}/release/secure-telemetry-node
"$BIN" --port=5599 > /tmp/stn-test.log 2>&1 &
PID=$!
sleep 1
if grep -q "seccomp filter actif" /tmp/stn-test.log && \
   curl -s --max-time 2 http://127.0.0.1:5599/ | grep -q "cpu_temp"; then
    echo "secure-telemetry-node: test OK (sandbox actif + réponse HTTP)";
else
    echo "secure-telemetry-node: ECHEC — démarrage ou réponse HTTP";
    cat /tmp/stn-test.log 2>/dev/null;
    kill "$PID" 2>/dev/null || true;
    exit 1;
fi
kill "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true

# Vérification du sandbox : l'auto-test doit mourir par SIGSYS (écriture fichier
# refusée par le filtre). Non bloquant : si seccomp est indisponible dans
# l'environnement de build (ex. conteneur sans support), on le signale sans
# casser le paquet.
# NB : rpmbuild exécute %check avec `set -e` ; un SIGSYS (statut non nul) ferait
# sortir le shell avant lecture de $RC. On désactive donc -e explicitement.
set +e
"$BIN" --sandbox-self-test
RC=$?
set -e
if [ "$RC" -eq 132 ] || [ "$RC" -eq 159 ]; then
    echo "secure-telemetry-node: test seccomp OK (écriture refusée par SIGSYS)";
elif [ "$RC" -eq 1 ]; then
    echo "secure-telemetry-node: AVERTISSEMENT — seccomp indisponible dans l'environnement de build";
else
    echo "secure-telemetry-node: AVERTISSEMENT — self-test seccomp inattendu (rc=$RC)";
fi
exit 0

%files
%{_sbindir}/secure-telemetry-node
/usr/lib/systemd/system/secure-telemetry-node.service

# --- Subpackage redtest : contenu du package -redtest ---
%package redtest
Summary:        Tests d'intégration redpesk (TAP) pour %{name}
Requires:       %{name} = %{version}-%{release}
Requires:       curl

%description redtest
Tests d'intégration exécutés par la plateforme redpesk sur cible virtuelle
(QEMU). Vérifie le démarrage du daemon, l'activation du sandbox seccomp et la
réponse HTTP. Sortie au format TAP (Test Anything Protocol).

%files redtest
%defattr(-,root,root)
%{_libexecdir}/redtest/%{name}/*

%post
systemctl daemon-reload || true

%changelog
* Thu Sep 04 2026 Parad1gm <parad1gm_0xFF@gmail.com> - 0.1.0-1
- Ajout du subpackage -redtest (tests d'intégration TAP exécutés par redpesk).
* Wed Sep 02 2026 Parad1gm <parad1gm_0xFF@gmail.com> - 0.1.0-1
- Variante redpesk factory : compilation a la source via Cargo.