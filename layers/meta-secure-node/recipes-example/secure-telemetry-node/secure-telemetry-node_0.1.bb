# Recipe Yocto : secure-telemetry-node
# Compile le daemon Rust pour la cible embarquee (aarch64 / RPi3B+) et
# l'installe dans l'image rootfs. Deploi pour nerves := nerves.
#
# ligne d'utilisation :
#   bitbake secure-telemetry-node    (construire le package seul)
#   IMAGE_INSTALL:append = " secure-telemetry-node"  (dans l'image)

SUMMARY = "Secure telemetry node daemon (Rust, static)"
HOMEPAGE = "https://github.com/Parad1gm-0xFF/secure-telemetry-node"
LICENSE = "Apache-2.0"
LIC_FILES_CHKSUM = "file://LICENSE;md5=<remplacer-par-hash>"

# SRCREV a pointer vers le commit ; ici repo local (devconv).
SRC_URI = "file://src/main.rs \
           file://Cargo.toml \
           file://packaging/secure-telemetry-node.service"

SRCREV = "AUTOINC"

# On emploie le cargo integré Yocto (cargo-class). Pas de dep CRates externes
# hormis libc fourni par le registry (voir cargo-bbclass / local cache).
inherit cargo systemd

# Cible par defaut : le TUNE_ARCH de la machine (aarch64 pour RPi3B+).
# Binaire statique musl:
CARGO_BUILD_FLAGS:append = " --release"
# on passe la target via cargo (aarch64-unknown-linux-musl) en LOCALCONF.

SYSTEMD_SERVICE:${PN} = "secure-telemetry-node.service"

# Etape install : placer le binaire compilé par cargo et l'unite systemd.
do_install:append() {
    install -d ${D}${sbindir}
    install -m 0755 ${CARGO_BIN_DIR}/${BPN} ${D}${sbindir}/${BPN}
    install -d ${D}${systemd_system_unitdir}
    install -m 0644 ${WORKDIR}/packaging/secure-telemetry-node.service \
                    ${D}${systemd_system_unitdir}/${BPN}.service
}

# L'unite cree un utilisateur dedie (meme nom que dans le .service).
inherit useradd
USERADD_PACKAGES = "${PN}"
USERADD_PARAM:${PN} = "-u 1200 -r -s /sbin/nologin -d /var/lib/telemetry telemetry"

FILES:${PN} += "${sbindir}/${BPN} ${systemd_system_unitdir}/${BPN}.service"