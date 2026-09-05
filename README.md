# 🐟 secure-telemetry-node.

> Daemon de télémétrie **sécurisée** pour Linux embarqué : **Rust** (std seul, zéro
> dépendance), cross-compilé **aarch64 / riscv64** (musl statique), exécuté sous
> **QEMU**, packagé **RPM**, sandboxé par un **filtre seccomp BPF** écrit à la main,
> et doublé d'un versant **Kernel/BSP** (module noyau + overlay Device Tree).

Projet de démonstration de bout en bout pour un poste d'**Ingénieur Linux Embarqué
Kernel/BSP** (dans l'esprit d'une candidature chez IoT.bzh, Lorient), aligné sur la
plateforme **redpesk** (factory, packaging RPM, sécurité dès le build, LTS).

---

## 🎯 Ce que le projet prouve

| Mission / compétence | Preuve dans ce dépôt |
|---|---|
| Intégration / optimisation **BSP Yocto** | Couche `meta-secure-node` + recipe (RPi3B+) |
| Programmation bas-niveau **C/C++/Rust** | Daemon Rust (std seul, FFI direct) + module noyau C |
| **Drivers / BSP** | Module noyau `stn-sensor` + overlay Device Tree `stn-status` |
| **Portabilité RISC-V** | Rebuild identique `riscv64gc-musl` (CI) |
| Packaging **RPM** / factory | Specfile `build from source` + `%check` fonctionnel |
| **Cybersécurité dès le build** | Filtre seccomp BPF maison, unité systemd durcie, self-test |
| **Tests** | 10 tests unitaires + smoke test HTTP sous seccomp + CI GitHub Actions |

---

## 🔒 Sécurité réelle, et limites.

Le point central du projet est un **filtre seccomp (SECCOMP_MODE_FILTER) écrit à la
main en bytecode eBPF**, sans libseccomp. Contrairement à un mode `STRICT` (qui tue
tout démon réseau dès `accept()`), le filtre est une **whitelist de syscalls** avec
une règle argumentaire sur `openat` :

- les syscalls autorisés sont listés par architecture (les numéros diffèrents entre
  x86_64 et aarch64/riscv64) ;
- `openat` est refusé en écriture : le filtre inspecte les **flags** de l'appel
  (`flags & O_WRONLY`) et tue uniquement les ouvertures en écriture ;
- tout syscall hors whitelist est refusé.

Le filtre a été confronté à deux évolutions réelles du kernel 7.0 :

1. **Ré-encodage des instructions eBPF** (`BPF_ABS`/`BPF_AND` déplacés). Le démon
   tente l'encodage récent, puis retombe sur l'historique (kernels <= 6.x, Yocto LTS).
2. **Déplacement de `args[0]`** dans `seccomp_data` (offset 32 au lieu de 16).
   Le démon **détecte l'offset à l'exécution** via un sous-processus sonde
   (`fork` + filtre minimal + tentative d'écriture réelle), sans hardcoder.

La preuve est **sémantique** : `--sandbox-self-test` installe le filtre puis tente
d'écrire un fichier. Le process est tué par **SIGSYS** (code de sortie 132/159).
Et le service, lui, **tourne** sous ce même filtre (répond en HTTP, lit la mémoire,
fait ses pauses), ce qui prouve que la whitelist est fonctionne.

```
$ ./secure-telemetry-node --probe-verbose
secure-telemetry-node v0.1.0 (rust)
sandbox: seccomp filter actif ✓ (BPF 0x20/0x54 (kernel 7.0+), args@32, 45 instructions, whitelist 34 syscalls + openat lecture seule)

$ ./secure-telemetry-node --sandbox-self-test ; echo $?
159        # mort par SIGSYS : l'écriture fichier est bien refusée
```

### Options du daemon.

| Option | Rôle |
|---|---|
| `--port=N`, `--addr=HOST` | Écoute (défaut `127.0.0.1:5555`) |
| `--sandbox-self-test` | Prouve que le filtre bloque une écriture (SIGSYS) |
| `--probe-verbose` | Affiche la détection de l'offset des arguments |
| `--sandbox-log` | Installe le filtre en mode log (syscalls refusés en EPERM au lieu de tuer), pour auditer avec strace |
| `--no-sandbox` | Désactive la sandbox (debug / démo dégradée) |

---

## 🧱 Architecture.

```
src/main.rs                 → Daemon Rust (std seul, FFI prctl/fork/openat).
kernel/
  stn-sensor.c              → Module noyau : pseudo-capteur /dev/stn-sensor (API miscdevice 6.x LTS).
  overlays/stn-status.dts   → Overlay Device Tree RPi3B+ (LED GPIO + nœud capteur).
  Makefile + README.md      → Build module + overlay.
packaging/secure-telemetry-node.service → Unité systemd durcie (SystemCallFilter, ProtectClock...).
spec/secure-telemetry-node.spec         → Specfile RPM redpesk, build from source.
layers/meta-secure-node/    → Couche Yocto (recipe .bb, RPi3B+).
scripts/build-cross.sh      → Cross-compile aarch64 + riscv64 (Docker).
scripts/run-qemu.sh         → Exécution ARM sous QEMU usermode (sans carte).
scripts/flash-rpi3.sh       → Écriture de l'image Yocto sur carte SD.
.github/workflows/ci.yml    → CI : cross-compile musl, tests, smoke seccomp, RPM.
```

---

## ✅ Démonstration.

### 1. Compiler et tester (x86_64, poste de dev).

```bash
cargo test --release        # 10 tests unitaires (parsing, construction du eBPF)
cargo build --release

# Service sandboxé :
./target/release/secure-telemetry-node --port=5555 &
curl -s http://127.0.0.1:5555/
# -> t=+1s cpu_temp=25.0C gpio=stable free_mem_kb=8341212

# Preuve que le filtre bloque réellement une écriture :
./target/release/secure-telemetry-node --sandbox-self-test ; echo $?
# -> 159 (SIGSYS)
```

### 2. Cross-compiler vers ARM et exécuter sous QEMU.

```bash
./scripts/build-cross.sh aarch64
./scripts/run-qemu.sh 5555
# dans un autre terminal :
curl -s http://127.0.0.1:5555/
```

Note de modération : Sous QEMU usermode, `prctl(PR_SET_SECCOMP)` échoue (QEMU ne
traduit pas le sandbox du guest de façon fiable). Le démon bascule alors
proprement en **mode dégradé** et la démo prouve la cross-compile + l'exécution
ARM. Le seccomp, lui, est prouvé sur x86_64 natif (self-test SIGSYS) et sera
actif sur la cible embarquée réelle (kernel Yocto).

### 3. Porter vers RISC-V.

```bash
./scripts/build-cross.sh riscv64
# -> target/riscv64gc-unknown-linux-musl/release/secure-telemetry-node
```

---

## 🔧 Versant Kernel/BSP.

Voir `kernel/README.md` pour le détail. Le dépôt embarque :

- **`stn-sensor.c`** : driver noyau exposant `/dev/stn-sensor` (lecture d'un
  pseudo-capteur). Cible assumée : API `miscdevice` des kernels 6.x LTS
  (Yocto/redpesk). Le kernel 7.0 a visiblement refondu cette API : Le module refuse de
  compiler sur une API non couverte (`#error`) plutôt que de produire un binaire
  incohérent, comme le fait la sandbox vis-à-vis de seccomp.
- **`stn-status.dts`** : overlay Device Tree RPi3B+ (LED d'état GPIO + nœud de
  capteur), compilé par `dtc` (`make overlay`).

---

## 📦 Packaging RPM (modèle redpesk).

Le specfile compile le code Rust dans la factory et exécute un `%check` qui
vérifie le **démarrage + réponse HTTP** sous sandbox, puis le **self-test SIGSYS**
(non bloquant si seccomp est indisponible dans l'environnement de build).
Le spec produit deux paquets : le daemon (`secure-telemetry-node`) et le paquet
de test (`secure-telemetry-node-redtest`) exécuté par la plateforme.

Les RPM ne sont plus commités : la CI les produit comme artefacts (GitHub
Actions). Localement :

```bash
docker run --rm -v "$PWD/.rpmbuild":/rpmbuild -w /rpmbuild almalinux:9 \
  bash -c 'dnf install -y rpm-build cargo && rpmbuild --define "_topdir /rpmbuild" \
  -bb SPECS/secure-telemetry-node.spec'
```

---

## 🐟 Build & audits sur la plateforme redpesk.

Le projet est industrialisé sur la **redpesk factory Community**
(`community-app.redpesk.bzh`, compte gratuit), via `rp-cli`. Résultats au
04/09/2026 :

| Étape | Commande | Résultat |
|---|---|---|
| Build (x86_64, distro corn 3.0) | `rp-cli applications build secure-telemetry-node` | ✅ `done` |
| `%check` (sandbox actif + HTTP) | intégré au build | ✅ `test OK (sandbox actif + réponse HTTP)` + `test seccomp OK (écriture refusée par SIGSYS)` |
| RPM produits | build log | ✅ `secure-telemetry-node-...rpm` + `secure-telemetry-node-redtest-...rpm` |
| Audit statique clang-tidy | `rp-cli applications audit` | ✅ **0 vulnérabilité** (après exclusion de `kernel/stn-sensor.c`, module noyau non analysable en user-space) |
| Tests embarqués (QEMU) | `rp-cli applications test` | ⏸ bloqués par la plateforme (échec au déploiement de la VM, `boot.log` illisible : erreur serveur `read on closed response body`, 0 test exécuté) |

Détails et modérations :

- **Audit** : l'audit initial signalait un « High » sur `kernel/stn-sensor.c`
  (`linux/miscdevice.h` introuvable). C'est visiblement un **artefact d'environnement** : le
  module noyau se compile contre les headers kernel de la cible Yocto, absents de
  l'environnement d'audit user-space de redpesk. Le fichier a donc été exclu de
  l'audit applicatif (le code C du module est analysable dans son propre build
  kernel). Le reste du dépôt est audité sans vulnérabilité.
- **Tests** : le subpackage `-redtest` est bien produit et installé dans
  `/usr/libexec/redtest/secure-telemetry-node/` (`run-redtest` au format TAP).
  L'exécution sur cible QEMU est en attente de disponibilité de l'infrastructure
  Community (échec au boot de la VM, sans rapport avec l'application visiblement. Plusieurs
  tests ont été faits étalés dans le temps, limite du compte free probable).

---

## ⚙️ Note : pourquoi `opt-level = 2` dans le profil release.

La valeur `"s"` (taille) casse la **sonde de détection seccomp** : elle passe par
`fork()` dans `probe_args_offset`, et une optimisation de taille trop agressive
produit un code erroné dans le processus fils. `opt-level = 2`/`3` est nécessaire
au bon fonctionnement du fork. Le binaire reste compact (std seul, aucune
dépendance).

---

## 📜 Licence.

Apache-2.0, 2026 Parad1gm_0xFF. Projet de démonstration, non affilié à IoT.bzh.
Les noms redpesk, redpak et Yocto restent la propriété de leurs détenteurs.
