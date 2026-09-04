# 🐟 secure-telemetry-node.

> Daemon de télémétrie **sécurisée** pour Linux embarqué - écrit en **Rust**, cross-compilé **ARM**, packagé **RPM**, exécuté **sous QEMU**, porté **RISC-V**.

Projet de démonstration de bout en bout, pensé comme une « démonstration » pour un poste
d'**Ingénieur Linux Embarqué Kernel/BSP** chez **IoT.bzh** (Lorient) - et aligné sur
l'architecture de leur plateforme **redpesk®** (factory SaaS/on-premise, packaging
RPM, sécurité dès le build, support long terme).

---

## 🎯 Pourquoi ce projet.

Il illustre, en un seul dépôt, les missions probables d'un profil Kernel/BSP embarqué :

| Mission / compétence | Preuve dans ce projet |
|---|---|
| Intégration / optimisation de **BSP Yocto** | Couche `meta-secure-node` + recipe (Cible Raspberry Pi 3B+) |
| Programmation bas-niveau **C/C++/Rust** | Daemon écrit en Rust, binaire statique musl |
| **Drivers / power management** | Accès matériel (Température CPU, GPIO) |
| **Tests spécifiques** | Test d'intégration sous **QEMU** (Sans carte) |
| **Portabilité RISC-V** | Rebuild identique vers `riscv64gc-musl` |
| Packaging **RPM** / factory | Specfile RPM installable via `dnf` |
| **Cybersécurité dès le build** | Sandbox seccomp + unité systemd durcie |

---

## 🧱 Architecture.

```
src/main.rs                → Daemon Rust (std seul, FFI prctl — aucune dépendance externe).
Cargo.toml                 → Cibles aarch64 / riscv64, binaire statique musl.
spec/secure-telemetry-node.spec → specfile RPM unique, « build from source » (Cargo).
packaging/secure-telemetry-node.service → unité systemd durcie.
layers/meta-secure-node/   → Layer Yocto (recipe .bb) pour l'image RPi3B+.
scripts/build-cross.sh     → Cross-compile aarch64 + riscv64 (Docker).
scripts/run-qemu.sh        → Exécution ARM sous QEMU usermode (Sans carte).
scripts/flash-rpi3.sh      → Écriture de l'image sur carte SD RPi3B+.
tools/qemu-aarch64-static  → QEMU usermode (Sans droits root).
dist/                      → RPM construits (Aarch64 pré-compilé + x86_64 build source).
```

---

## ✅ Démonstration.

### 1. Cross-compiler vers ARM depuis un poste x86.

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

### 2. Exécuter le binaire ARM **sous QEMU**, sans carte.

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

### 3. Porter vers RISC-V (Mission de l'offre).

```bash
docker run --rm -v "$PWD":/work -w /work \
  ghcr.io/rust-cross/rust-musl-cross:riscv64gc-musl \
  cargo build --release --target riscv64gc-unknown-linux-musl
```

✅ Compilation **riscv64gc-musl** réussie (portabilité démontrée).

---

## 📦 Packaging RPM (Modèle redpesk).

Dans la redpesk factory, chaque application/BSP devient un package installable/
actualisable via `dnf`, indépendant des images OS. Un **specfile unique** compile
le code Rust dans la factory. Des **artefacts pré-construits** sont aussi fournis.

### Specfile `spec/secure-telemetry-node.spec` - "build from source".
Compile le code Rust **via cargo directement** (projet `standard` redpesk). C'est
la variante qu'on utilise sur la plateforme - voir
[la section dédiée](#-utiliser-son-compte-redpesk-free--community).

```bash
# RPM x86_64 validé (Compilé et testé par le specfile) :
ls dist/secure-telemetry-node-0.1.0-1.el9.x86_64.rpm

# Reconstruire localement (sur hôte x86, via Docker) :
docker run --rm -v "$PWD/.rpmbuild":/rpmbuild -w /rpmbuild almalinux:9 \
  bash -c 'dnf install -y rpm-build cargo && rpmbuild --define "_topdir /rpmbuild" \
  -bb SPECS/secure-telemetry-node.spec'
```

### Artefact pré-compilé aarch64 (Fourni, référence).
Binaire ARM cross-compilé (RPi3B+), packagé pour installation directe.

```bash
ls dist/secure-telemetry-node-0.1.0-1.el9.aarch64.rpm
dnf install secure-telemetry-node-0.1.0-1.el9.aarch64.rpm
systemctl enable --now secure-telemetry-node
```

Unité `systemd` durcie (`NoNewPrivileges`, `ProtectSystem`, `PrivateTmp`, …) —
la sécurité ne se limite pas à l'applicatif, elle s'applique au système.

---

## 🔒 Sécurité "dès le build".

- **Seccomp** + `no-new-privs` via `prctl` (Sandbox applicatif minimal).
- **Unité systemd** avec durcissement (ProtectSystem=strict, PrivateTmp, RestrictSUIDSGID…).
- **Binaire statique** : Aucune dépendance dynamique à charger/attaqquer.
- Philosophie alignée redpesk : *"La sécurité ne peut pas être ajoutée après coup"*.

> Note : Sous musl, `syscall(SECCOMP_MODE_STRICT)` échoue souvent (L'allocateur
> exige plus que read/write/_exit) → Le code bascule proprement en mode dégradé
> tout en restant fonctionnel. En production réelle, redpesk s'appuie sur
> **LSM (Smack/SELinux)** + conteneur **redpak** - c'est le niveau d'isolation cible.

### Résultat de l'audit sécurité redpesk (Interprété honnêtement).

La factory redpesk propose plusieurs analyseurs statiques (Gosec, clang-tidy,
flawfinder). Le build x86_64 a été audité - **aucune vulnérabilité remontée**.

> **Interprétation (nuancée) :** Ces outils ciblent Go et C/C++. Étant un projet
> **Rust en std pur, sans aucune dépendance externe**, ils ne trouvent aucun
> fichier de leur langage à analyser → le "0 vulnérabilité" est autant un
> artefact d'outillage qu'un signal. Le vrai argument de sécurité est structurel :
> **Aucune dépendance tierce** (Ni supply-chain à scanner, ni code C/C++/Go à
> auditer) + **sécurité dès le build** (Seccomp, unité systemd durcie, binaire
> statique). Comprendre les limites des scanners est une compétence clé pour un
> profil cybersécurité embarqué.

---

## 🔧 Déploiement sur Raspberry Pi 3B+.

1. **Construire l'image Yocto** (Poky LTS + `meta-raspberrypi` + `meta-secure-node`),
   cible `raspberrypi3-64`.
2. **Flasher** la carte SD :
   ```bash
   ./scripts/flash-rpi3.sh <image.wic> /dev/sdX
   ```
3. Booter la carte → le service démarrera au boot via systemd.

---

## 🐟 Utiliser son compte redpesk (Free / Community).

Le repo est conçu pour être **industrialisé via la redpesk factory**. Avec un
compte **Community** (gratuit), on peut faire compiler le code par la plateforme
et obtenir un RPM + un **SBOM/VEX** - le volet cybersécurité au cœur de redpesk.

### Concepts redpesk à connaître.
- **Projet `standard`** → produit un **référentiel RPM** (Les applications packagées).
- **Application** → liée à un **dépôt git** + un **specfile** (Ici `secure-telemetry-node.spec`).
- **Distribution** → ex. `redpesk lts corn 3.0` (Aligné CentOS, LTS).
- Le **test automatisé** intégré ne se fait que sur cible **x86_64 virtuelle (QEMU)** ;
  pour ARM on construit l'image ou on installe le RPM sur la carte.

### Étapes dans l'interface web (community-app.redpesk.bzh).

1. **Créer un projet** de type **`standard`** (Nom : `secure-telemetry-node`).
2. **Ajouter l'application** dans le projet :
   - **Source URL** : `https://github.com/Parad1gm-0xFF/secure-telemetry-node`
   - **Source revision** : `main`
   - **Spec file** : `spec/secure-telemetry-node.spec`
     (Option « specfile dans le dépôt source »)
3. **Choisir l'architecture** :
   - `x86_64` → pour le **test redpesk** (QEMU virtuel) - le plus rapide ;
   - `aarch64` → pour un **déploiement RPi3B+** (RPM installable / image).
4. **Lancer le build** → le builder partagé **compile le Rust via cargo**
   (Le `%check` lance le daemon et vérifie qu'il répond).
5. **Inspecter le résultat** : RPM produit + **SBOM/VEX** (Dépendances, licences, CVE)
   - très probablement le volet cybersécurité attendu d'un ingénieur Kernel/BSP chez IoT.bzh.

> Limite du compte Free : Builder **partagé**, compilation/tests "best effort"
> (Pas de QoS, files d'attente). Suffisant pour une démo.

### Stratégie d'architecture (Testée puis validée)

**Compilation par la factory (`build-from-source`) : Fait sur `x86_64`** ✅
Le build **x86_64** redpesk passe et produit un **RPM + SBOM/VEX** - c'est la preuve
de la chaîne CI redpesk (Source → build → package → analyse de sécurité).

**Cross-compilation `aarch64` : Démontrée par cross-compile locale + QEMU** ✅
Le target `aarch64` n'est **pas compilable par le builder redpesk Community** car
le builder est *offline* et le **`rust-std` pour aarch64 n'existe pas** dans les
dépôts RHEL/Alma/EPEL (Deules les std `x86_64`/`i686`/`wasm` y sont). `rustup target
add aarch64` (Qui télécharge) est donc indisponible.

La compétence **aarch64** est prouvée par les chaînes locales (Reproductibles) :
- Binaire **statique `aarch64-unknown-linux-musl`** cross-compilé (Voir plus haut),
- **Exécuté sous QEMU usermode** sans matériel ARM,
- RPM aarch64 pré-construit fourni dans `dist/`.

> **Pourquoi c'est un bon signal (Même en échec de build-aarch64) :** Démontrer
> qu'on comprend *pourquoi* le builder ne peut pas cross-compiler (Std Rust manquant
> offline) et qu'on sait où basculer (x86_64 pour la CI, cross-compile locale pour
> l'embarqué) est un atout en situation réelle : Cela reflète une vraie maîtrise des
> contraintes d'industrialisation et de build, au cœur d'un poste Kernel/BSP.

---

## 🧪 Reproduire de A à Z.

Prérequis : **Docker** (rootless suffit), `curl`, ~quelques Go d'espace suffisent.

```bash
git clone https://github.com/Parad1gm-0xFF/secure-telemetry-node
cd secure-telemetry-node

make build-aarch64   # ou : make build-riscv64
make run-qemu        # exécution ARM sous QEMU
```

---

## 📜 Licence.

Apache-2.0 - 2026 Parad1gm_0xFF.
Projet de démonstration, non affilié à IoT.bzh. Les noms redpesk, redpak et
Yocto restent la propriété de leurs détenteurs respectifs.
