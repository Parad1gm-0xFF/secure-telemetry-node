# kernel/ - versant Kernel/BSP

Preuve du versant **Kernel/BSP** du poste visé (Ingénieur Linux Embarqué
Kernel/BSP), en complément du daemon applicatif (`src/`).

## Contenu

| Fichier | Rôle |
|---|---|
| `stn-sensor.c` | Driver noyau d'exemple : pseudo-capteur exposé en `/dev/stn-sensor` (lecture = valeur simulée en milli-degrés). Cycle de vie complet : `module_init`, `misc_register`, callback de lecture, `misc_deregister`. |
| `Makefile` | Build du module contre le kernel courant, compilation de l'overlay Device Tree. |
| `overlays/stn-status.dts` | Overlay Device Tree RPi3B+ : ajoute une LED d'état (gpio-leds) et un nœud de pseudo-capteur (lien BSP <-> applicatif). |

## Build

```bash
# Module contre le kernel courant (poste de dev) :
make

# Overlay Device Tree (besoin de dtc) :
make overlay

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

Ce module cible **délibérément** l'API `miscdevice` des kernels **6.x LTS**
(Yocto Scarthgap / redpesk LTS). Le kernel 7.0 a refondu cette API (callbacks
déplacés dans `struct file_operations`), et 6.11 a réorganisé les callbacks
dans un sous-struct `params`. Comme pour seccomp dans `src/main.rs`, ces
évolutions sont réelles : le module refuse de compiler (`#error`) sur une API
non couverte plutôt que de produire un binaire incohérent. C'est le comportement
attendu d'un BSP engineer qui assume un support kernel précis.