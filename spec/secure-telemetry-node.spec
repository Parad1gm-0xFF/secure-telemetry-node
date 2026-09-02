# Spec RPM minimal pour le noeud de telemetrie.
# Alignement redpesk : dans la redpesk factory, chaque application/BSP devient
# un package RPM installe/actualise via dnf, independamment de l'image OS.
# On reprend ce modele : le daemon se livre en RPM aarch64.

Name:           secure-telemetry-node
Version:        0.1.0
Release:        1%{?dist}
Summary:        Secure telemetry node daemon (Rust, static musl)

License:        Apache-2.0
URL:            https://github.com/Parad1gm-0xFF/secure-telemetry-node
Source0:        %{name}-%{version}.tar.gz

# Cible : aarch64 (RPi3B+/arm64). Le binaire est deja compile en static musl
# (cargo build --target aarch64-unknown-linux-musl --release).
BuildArch:      aarch64
BuildRequires:  cargo, rust

%description
Demon de telemetrie securise pour Linux embarque, compile en static musl aarch64.
Fournit un petit microservice TCP (esprit afb-binder) exposant temperature CPU,
etat GPIO et memoire libre. Binary statique : aucune dependance dynamique.

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
* %{_date} <vous@exemple.fr> - 0.1.0-1
- Version initiale du daemon de telemetrie securise aarch64.