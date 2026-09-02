# 🐟 secure-telemetry-node

> Daemon de télémétrie **sécurisée** pour Linux embarqué — écrit en **Rust**, cross-compilé **ARM**, packagé **RPM**, exécuté **sous QEMU**, porté **RISC-V**.

Projet de démonstration de bout en bout, pensé comme un « passeport » pour un poste
d'**Ingénieur Linux Embarqué Kernel/BSP** chez **IoT.bzh** (Lorient) — et aligné sur
l'architecture de leur plateforme **redpesk®** (factory SaaS/on-premise, packaging
RPM, sécurité dès le build, support long terme).

---

## 🎯 Pourquoi ce projet

Il illustre, en un seul dépôt, les missions d'un profil Kernel/BSP embarqué :

| Mission / compétence | Preuve dans ce projet |
|---|---|
| Intégration / optimisation de **BSP Yocto** | Couche `meta-secure-node` + recipe (cible Raspberry Pi 3B+) |
| Programmation bas-niveau **C/C++/Rust** | Daemon écrit en Rust, binaire statique musl |
| **Drivers / power management** | Accès matériel (température CPU, GPIO) |
| **Tests spécifiques** | Test d'intégration sous **QEMU** (sans carte) |
| **Portabilité RISC-V** | Rebuild identique vers `riscv64gc-musl` |
| Packaging **RPM** / factory | Specfile RPM installable via `dnf` |
| **Cybersécurité dès le build** | Sandbox seccomp + unité systemd durcie |

---

## 🧱 Architecture

```
src/main.rs                → Daemon Rust (std + libc, aucun objet lourd)
Cargo.toml                 → cibles aarch64 / riscv64, binaire statique musl
spec/secure-telemetry-node.spec → packaging RPM (modèle redpesk factory)
packaging/secure-telemetry-node.service → unité systemd durcie
layers/meta-secure-node/   → layer Yocto (recipe .bb) pour l'image RPi3B+
scripts/build-cross.sh     → cross-compile aarch64 + riscv64 (Docker)
scripts/run-qemu.sh        → exécution ARM sous QEMU usermode (sans carte)
scripts/flash-rpi3.sh      → écriture de l'image sur carte SD RPi3B+
tools/qemu-aarch64-static  → QEMU usermode (sans droits root)
```

---

## ✅ Démonstration (validée)

### 1. Cross-compiler vers ARM depuis un poste x86

```bash
docker run --rm -v "$PWD":/work -w /work \
  ghcr.io/rust-cross/rust-musl-cross:aarch64-musl \
  cargo build --release --target aarch64-unknown-linux-musl
```

Résultat :
```
target/aarch64-unknown-linux-musl/release/secure-telemetry-node
=> ELF 64-bit LSB executable, ARM aarch64, statically linked, stripped (452 Ko)
```

### 2. Exécuter le binaire ARM **sous QEMU**, sans carte

```bash
./scripts/run-qemu.sh 5555
# puis, dans un autre terminal :
curl -s http://127.0.0.1:5555/
```

Sortie réelle obtenue :
```
t=+3s cpu_temp=25.0C gpio=stable free_mem_kb=9629064
```

👉 Un exécutable **ARM** qui répond depuis un poste **x86**, via QEMU usermode —
preuve de la chaîne cross-compile + émulation.

### 3. Porter vers RISC-V (mission de l'offre)

```bash
docker run --rm -v "$PWD":/work -w /work \
  ghcr.io/rust-cross/rust-musl-cross:riscv64gc-musl \
  cargo build --release --target riscv64gc-unknown-linux-musl
```

✅ Compilation **riscv64gc-musl** réussie (portabilité démontrée).

---

## 📦 Packaging RPM (modèle redpesk)

Le daemon se livre en **RPM aarch64** — artefact fourni dans **`dist/`**.
Dans la redpesk factory, chaque application/BSP devient un package installable/
actualisable via `dnf`, indépendant des images OS. On applique exactement ce
modèle : le RPM empaquette le binaire ARM pré-compilé + l'unité systemd.

```bash
# 1. RPM déjà construit (fourni) :
ls dist/secure-telemetry-node-0.1.0-1.el9.aarch64.rpm

# 2. Le reconstruire soi-même (sur hôte x86, via Docker) :
docker run --rm -v "$PWD/.rpmbuild":/rpmbuild -w /rpmbuild almalinux:9 \
  bash -c 'dnf install -y rpm-build && rpmbuild --define "_topdir /rpmbuild" \
  --target aarch64-redhat-linux-gnu -bb SPECS/secure-telemetry-node.spec'

# 3. Installer sur la cible (redpesk OS / système RPM) :
dnf install secure-telemetry-node-0.1.0-1.el9.aarch64.rpm
systemctl enable --now secure-telemetry-node
```

Unité `systemd` durcie (`NoNewPrivileges`, `ProtectSystem`, `PrivateTmp`, …) —
la sécurité ne se limite pas à l'applicatif, elle s'applique au système.

---

## 🔒 Sécurité « dès le build »

- **Seccomp** + `no-new-privs` via `prctl` (sandbox applicatif minimal).
- **Unité systemd** avec durcissement (ProtectSystem=strict, PrivateTmp, RestrictSUIDSGID…).
- **Binaire statique** : aucune dépendance dynamique à charger/attaqquer.
- Philosophie alignée redpesk : *« la sécurité ne peut pas être ajoutée après coup »*.

> Note : sous musl, `syscall(SECCOMP_MODE_STRICT)` échoue souvent (l'allocateur
> exige plus que read/write/_exit) → le code bascule proprement en mode dégradé
> tout en restant fonctionnel. En production réelle, redpesk s'appuie sur
> **LSM (Smack/SELinux)** + conteneur **redpak** — c'est le niveau d'isolation cible.

---

## 🔧 Déploiement sur Raspberry Pi 3B+

1. **Construire l'image Yocto** (poky LTS + `meta-raspberrypi` + `meta-secure-node`),
   cible `raspberrypi3-64`.
2. **Flasher** la carte SD :
   ```bash
   ./scripts/flash-rpi3.sh <image.wic> /dev/sdX
   ```
3. Booter la carte → le service démarrera au boot via systemd.

---

## 🧪 Reproduire de A à Z

Prérequis : **Docker** (rootless suffit), `curl`, ~quelques Go d'espace.

```bash
git clone https://github.com/Parad1gm-0xFF/secure-telemetry-node
cd secure-telemetry-node

make build-aarch64   # ou : make build-riscv64
make run-qemu        # exécution ARM sous QEMU
```

---

## 📜 Licence

Apache-2.0 — © 2026 Parad1gm_0xFF.
Projet de démonstration, non affilié à IoT.bzh. Les noms redpesk®, redpak® et
Yocto restent la propriété de leurs détenteurs respectifs.
