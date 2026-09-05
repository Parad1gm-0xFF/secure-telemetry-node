# kernel/ebpf/ - supervision eBPF du daemon

Complément du sandbox seccomp : le filtre seccomp (src/main.rs) est écrit en
**cBPF** (l'API seccomp n'accepte que du classic BPF : `struct sock_filter`,
voir `include/linux/filter.h` ; le kernel traduit cBPF → eBPF en interne pour
l'exécution, mais l'interface publique est cBPF). La **supervision** du daemon,
elle, utilise du vrai **eBPF** pour observer son activité en direct.

## Contenu

| Fichier | Rôle |
|---|---|
| `supervise-stn.bt` | Script bpftrace : compte les syscalls du daemon, détecte les SIGSYS bloqués par seccomp, affiche le trafic réseau et les lectures de capteurs. |
| `supervise-stn.sh` | Wrapper : lance le daemon puis le superviseur eBPF. |
| `supervise_stn.bpf.c` | Programme eBPF en C (compile avec clang, charge avec bpftool) : comptage de syscalls + SIGSYS dans des maps. Démo du tooling kernel (verifier, maps, attach points). |
| `Makefile` | Cibles `supervise` (bpftrace) et `prog` (bpftool). |

## Prérequis

- `bpftrace` et/ou `bpftool` selon le mode ;
- droits root ou `CAP_BPF` + `CAP_SYS_ADMIN` : le syscall `bpf()` est restreint
  aux privilégiés sur la plupart des distributions
  (`/proc/sys/kernel/unprivileged_bpf_disabled`).

## Démo bpftrace (simple)

```bash
sudo make -C kernel/ebpf supervise DAEMON=./target/release/secure-telemetry-node
```

ou manuellement :

```bash
sudo bpftrace kernel/ebpf/supervise-stn.bt <pid-du-daemon>
```

Sortie (extrait) :

```
--- état du sandbox (toutes les 5s) ---
@syscalls[secure-telemet]: 42
--- appels réseau ---
@accept4[secure-telemet]: 1
@recvfrom[secure-telemet]: 1
@sendto[secure-telemet]: 3
--- openat (lecture) ---
@openat[secure-telemet, /proc/meminfo]: 1
--- SIGSYS (écritures refusées) ---
@[secure-telemet]: 0
```

Lire `@sigsys` : s'il est à 0, le filtre seccomp bloque correctement (aucune
écriture non autorisée n'aboutit). En lançant `--sandbox-self-test` sous le
superviseur, on doit observer un **SIGSYS** compté (la preuve eBPF du blocage).

## Démo bpftool (programme eBPF compilé)

```bash
# Compiler (clang requis) :
make -C kernel/ebpf prog

# Inspecter les maps :
bpftool map dump name stn_syscalls
bpftool map dump name stn_sigsys
```

## À retenir

- Seccomp = cBPF (API figée), eBPF = supervision. Les deux sont complémentaires.
- La supervision eBPF est la preuve *en direct* que le sandbox seccomp
  fonctionne (comptage de syscalls whitelistés, zéro SIGSYS attendu).