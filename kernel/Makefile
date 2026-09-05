# Makefile kernel/secure-telemetry-node.
#   make            : module noyau + overlay Device Tree
#   make modules    : module stn-sensor (contre le kernel courant)
#   make overlay    : compile l'overlay Device Tree (besoin de dtc)
#   make ebpf       : supervision eBPF du daemon (bpftrace/bpftool, voir ebpf/)
#   make clean      : nettoyage
#
# Cible embarquee (Yocto/redpesk) : construire contre le kernel de la cible,
# par ex. via un SDK Yocto :
#   make KERNELDIR=<path-to-kernel-build> ARCH=arm64 CROSS_COMPILE=aarch64-linux-
#
# Voir kernel/README.md pour le deploiement.

obj-m := stn-sensor.o

KERNELDIR ?= /lib/modules/$(shell uname -r)/build
PWD := $(shell pwd)
DTC ?= dtc

.PHONY: all modules overlay ebpf clean install

all: modules overlay

modules:
	$(MAKE) -C $(KERNELDIR) M=$(PWD) modules

overlay:
	$(DTC) -I dts -O dtb -o overlays/stn-status.dtbo overlays/stn-status.dts

ebpf:
	$(MAKE) -C ebpf supervise

clean:
	$(MAKE) -C $(KERNELDIR) M=$(PWD) clean 2>/dev/null || true
	$(MAKE) -C ebpf clean 2>/dev/null || true
	rm -f overlays/stn-status.dtbo

install: modules overlay
	$(MAKE) -C $(KERNELDIR) M=$(PWD) modules_install