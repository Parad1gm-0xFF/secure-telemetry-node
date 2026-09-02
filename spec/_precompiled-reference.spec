# Spec RPM pour le noeud de telemetrie.
# Alignement redpesk : dans la redpesk factory, chaque application/BSP devient
# un package RPM installe/actualise via dnf, independamment de l'image OS.
#
# Note : le binaire aarch64-musl est PRE-COMPILE par le flux Cargo
# (cargo build --target aarch64-unknown-linux-musl --release) puis empaquete
# ici, sans recompilation — exactement le modele par-paquet de redpesk.
#
# Build (sur hote x86 avec Docker) :
#   cd /tmp/rpmbuild && rpmbuild --define "_topdir $(pwd)" -bb SPECS/secure-telemetry-node.spec

Name:           secure-telemetry-node
Version:        0.1.0
Release:        1%{?dist}
Summary:        Secure telemetry node daemon (Rust, static musl aarch64)

License:        Apache-2.0
URL:            https://github.com/Parad1gm-0xFF/secure-telemetry-node
Source0:        %{name}-%{version}.tar.gz

# Le binaire cible est aarch64 (RPi3B+/arm64). On ne recompile pas ici.
ExclusiveArch:  aarch64

# Binaire deja stripped (pre-compile par Cargo) : pas de build-id attendu,
# et pas de paquet -debuginfo a generer.
%global _missing_build_ids_terminate_build 0
%global debug_package %{nil}

%description
Demon de telemetrie securise pour Linux embarque, compile en static musl aarch64.
Fournit un petit microservice TCP (esprit afb-binder) exposant temperature CPU,
etat GPIO et memoire libre. Binary statique : aucune dependance dynamique.
Alignement du packaging sur la redpesk factory (BSP/application en RPM, dnf).

%prep
%autosetup

%build
# (rien) — le binaire aarch64-musl est produit par le flux Cargo hors RPM.

%install
install -D -m 0755 %{_builddir}/%{name}-%{version}/target/aarch64-unknown-linux-musl/release/secure-telemetry-node \
    %{buildroot}%{_sbindir}/secure-telemetry-node
# Unite systemd (demarrage au boot).
install -D -m 0644 %{_builddir}/%{name}-%{version}/packaging/secure-telemetry-node.service \
    %{buildroot}%{_unitdir}/secure-telemetry-node.service

%files
%{_sbindir}/secure-telemetry-node
%{_unitdir}/secure-telemetry-node.service

%post
# Notifie systemd de la nouvelle unite.
systemctl daemon-reload || true

%changelog
* Wed Sep 02 2026 Parad1gm <parad1gm_0xFF@gmail.com> - 0.1.0-1
- Version initiale du daemon de telemetrie securise aarch64 (RPM empaqueté).