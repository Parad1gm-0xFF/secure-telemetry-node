# Recipe Yocto : secure-telemetry-node
# Compile le daemon Rust pour la cible embarquee (aarch64 / RPi3B+), installe
# l'unite systemd, et embarque le versant Kernel/BSP (module stn-sensor +
# overlay Device Tree) dans la racine.
#
# Utilisation :
#   bitbake secure-telemetry-node            (construire le package seul)
#   IMAGE_INSTALL:append = " secure-telemetry-node"   (dans l'image)

SUMMARY = "Secure telemetry node daemon (Rust, static)"
HOMEPAGE = "https://github.com/Parad1gm-0xFF/secure-telemetry-node"
LICENSE = "Apache-2.0"
LIC_FILES_CHKSUM = "file://LICENSE;md5=3b83ef96387f14655fc854ddc3c6bd57"

# En build d'integration reel, pointer SRCREV vers le hash du commit source
# (ex. SRCREV = "78d8c53...") et passer par un SRC_URI git. Le mode "file://"
# sert au developpement local (devtool) et demontre le contenu embarque.
SRC_URI = "file://src/main.rs \
           file://Cargo.toml \
           file://packaging/secure-telemetry-node.service \
           file://kernel/stn-sensor.c \
           file://kernel/overlays/stn-status.dts"

SRCREV = "AUTOINC"

# Cargo integre Yocto (cargo-bbclass). Aucune dependance externe : le daemon
# n'utilise que std + FFI direct, pas de crates.io requis (build offline).
inherit cargo systemd

CARGO_BUILD_FLAGS:append = " --release"

SYSTEMD_SERVICE:${PN} = "secure-telemetry-node.service"

# Etape install : binaire, unite systemd, et sources Kernel/BSP (compilation
# du module et de l'overlay a faire dans le build kernel, voir kernel/).
do_install:append() {
    install -d ${D}${sbindir}
    install -m 0755 ${CARGO_BIN_DIR}/${BPN} ${D}${sbindir}/${BPN}
    install -d ${D}${systemd_system_unitdir}
    install -m 0644 ${WORKDIR}/packaging/secure-telemetry-node.service \
                    ${D}${systemd_system_unitdir}/${BPN}.service
    install -d ${D}/usr/src/${BPN}/kernel/overlays
    install -m 0644 ${WORKDIR}/kernel/stn-sensor.c ${D}/usr/src/${BPN}/kernel/
    install -m 0644 ${WORKDIR}/kernel/overlays/stn-status.dts \
                    ${D}/usr/src/${BPN}/kernel/overlays/
}

# L'unite cree un utilisateur dedie (meme nom que dans le .service).
inherit useradd
USERADD_PACKAGES = "${PN}"
USERADD_PARAM:${PN} = "-u 1200 -r -s /sbin/nologin -d /var/lib/telemetry telemetry"

FILES:${PN} += "${sbindir}/${BPN} ${systemd_system_unitdir}/${BPN}.service \
                /usr/src/${BPN}/kernel/stn-sensor.c \
                /usr/src/${BPN}/kernel/overlays/stn-status.dts"