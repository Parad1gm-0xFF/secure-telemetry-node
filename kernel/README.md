# kernel/ - versant Kernel/BSP

Preuve du versant **Kernel/BSP** du poste visé (Ingénieur Linux Embarqué
Kernel/BSP), en complément du daemon applicatif (`src/`).

## Contenu

| Fichier | Rôle |
|---|---|
| `stn-sensor.c` | Driver noyau d'exemple : pseudo-capteur exposé en `/dev/stn-sensor` (lecture = valeur simulée en milli-degrés). Cycle de vie complet : `module_init`, `misc_register`, callback de lecture, `misc_deregister`. |
| `Makefile` | Build du module contre le kernel courant, compilation de l'overlay Device Tree, cible `ebpf`. |
| `overlays/stn-status.dts` | Overlay Device Tree RPi3B+ : ajoute une LED d'état (gpio-leds) et un nœud de pseudo-capteur (lien BSP <-> applicatif). |
| `ebpf/` | Supervision eBPF du daemon (bpftrace + programme eBPF compilé bpftool). Voir `ebpf/README.md`. |

## Build

```bash
# Module contre le kernel courant (poste de dev) :
make

# Overlay Device Tree (besoin de dtc) :
make overlay

# Supervision eBPF du daemon (bpftrace, root requis) :
make ebpf

# Cross pour la cible embarquée (ex. SDK Yocto/redpesk) :
make KERNELDIR=<kernel-build-dir> ARCH=arm64 CROSS_COMPILE=aarch64-linux-
```

## Déploiement cible (RPi3B+ sous Yocto/redpesk)

```bash
# Charger le module :
insmod stn-sensor.ko
cat /dev/stn-sensor        # -> 25000

# Ou via le gestionnaire de modules (fichier .ko installé dans la racine) :
echo 'KERNEL_MODULE_AUTOLOAD += "stn-sensor"' >> local.conf
```

Overlay : copier `overlays/stn-status.dtbo` dans la partition boot
(`/boot/overlays/`) et activer dans `config.txt` :

```
dtoverlay=stn-status
```

Le daemon Rust reste la source de vérité : il lit la température CPU via
`thermal_zone0`, avec repli simulé. Sur une carte industrielle équipée du
pseudo-capteur, il suffit d'ajouter le chemin `/dev/stn-sensor` dans
`read_cpu_temp()`.

## Note de compatibilité kernel

L'API `miscdevice` est **stable depuis longtemps** : la structure référence une
table `struct file_operations *fops` (au moins depuis v4.19, inchangée sur 6.x
et 7.x). Ce module utilise cette interface standard ; il compile contre les
kernels 6.17 et 7.0 (vérifié), et le pattern est identique sur 6.6 LTS
(Yocto/redpesk Scarthgap).