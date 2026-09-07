# Spec RPM pour le noeud de telemetrie — variante "package local RPi".
#
# VARIANTE DEPLOYMENT LOCAL : empaquette le binaire aarch64 STATIQUE musl
# précompilé (cross-compile aarch64-unknown-linux-musl, voir README) pour
# installation sur la cible Raspberry Pi 3B+ via dnf install local.
#
# Différences avec spec/secure-telemetry-node.spec (variante factory) :
#   - pas de %build : le binaire est précompilé (zéro BuildRequires) ;
#   - pas de %check : le binaire aarch64 ne s'exécute pas sur l'hôte x86_64
#     (il est validé sur cible, voir redtest et scripts/test-post-local.sh) ;
#   - BuildArch aarch64 explicite (le binaire cible est aarch64).
#
# Usage (sur hôte x86_64, conteneur almalinux:9) :
#   rpmbuild --define "_topdir $PWD/rpmbuild" --target aarch64 \
#     -bb SPECS/secure-telemetry-node-rpi.spec

%global _enable_debug_packages 0
%global debug_package %{nil}

Name:           secure-telemetry-node
Version:        0.1.0
Release:        1%{?dist}
Summary:        Secure telemetry node daemon (Rust, static aarch64)

License:        Apache-2.0
URL:            https://github.com/Parad1gm-0xFF/secure-telemetry-node
Source0:        %{name}-%{version}.tar.gz
Source1:        secure-telemetry-node

BuildArch:      aarch64

%description
Demon de telemetrie securise pour Linux embarque, binaire Rust statique musl
aarch64 (zero dependance). Securise des le build : sandbox seccomp (filtre
cBPF whitelist + regle openat lecture seule), unite systemd durcie.
Variante packaging local RPi3B+ (binaire precompile, hors factory).

%prep
%autosetup

%install
install -D -m 0755 %{SOURCE1} %{buildroot}%{_sbindir}/secure-telemetry-node
install -D -m 0644 packaging/secure-telemetry-node.service \
    %{buildroot}/usr/lib/systemd/system/secure-telemetry-node.service
mkdir -p %{buildroot}%{_libexecdir}/redtest/%{name}/
install -m 0755 redtest/run-redtest %{buildroot}%{_libexecdir}/redtest/%{name}/run-redtest

%files
%{_sbindir}/secure-telemetry-node
/usr/lib/systemd/system/secure-telemetry-node.service

# L'unité systemd tourne sous l'utilisateur dédié telemetry : création à
# l'installation (%pre), avant le démarrage du service.
%pre
getent group telemetry >/dev/null || groupadd -r telemetry
getent passwd telemetry >/dev/null || useradd -r -g telemetry -s /usr/sbin/nologin -d /var/lib/telemetry telemetry
exit 0

%package redtest
Summary:        Tests d'intégration redpesk (TAP) pour %{name}
Requires:       %{name} = %{version}-%{release}
Requires:       curl

%description redtest
Tests d'intégration exécutés sur cible : daemon actif, sandbox seccomp en
mode FILTER (état kernel via /proc), réponse HTTP, écriture refusée (SIGSYS).
Sortie TAP.

%files redtest
%defattr(-,root,root)
%{_libexecdir}/redtest/%{name}/*

%post
systemctl daemon-reload || true

%changelog
* Sun Sep 07 2026 Parad1gm <parad1gm_0xFF@gmail.com> - 0.1.0-1
- Variante packaging local RPi3B+ : binaire statique aarch64 precompile.
