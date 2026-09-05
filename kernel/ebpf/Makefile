# Makefile eBPF - supervision du daemon secure-telemetry-node.
#
# Deux modes :
#   1. bpftrace (démo simple, script .bt) : nécessite bpftrace + root/CAP_BPF
#      make supervise            # supervise le daemon lancé par supervise-stn.sh
#   2. programme eBPF compilé en C (démo "ingénieur kernel") : nécessite
#      clang + bpf-headers, chargé avec bpftool
#      make prog                 # compile kernel/ebpf/supervise_stn.c -> .o
#
# NB : les opérations eBPF (bpf() syscall) sont restreintes aux privilégiés ;
#      il faut root ou CAP_BPF/CAP_SYS_ADMIN.

CLANG ?= clang
BPFTOOL ?= bpftool
BPFTRACE ?= bpftrace
STN_DAEMON ?= /usr/sbin/secure-telemetry-node

.PHONY: all supervise prog clean

all: supervise

# --- Démo bpftrace : lance le daemon puis le supervise ------------------
supervise:
	$(BPFTRACE) --version >/dev/null 2>&1 || { echo "bpftrace manquant"; exit 1; }
	./supervise-stn.sh "${STN_DAEMON}"

# --- Programme eBPF compilé (bpftool) ------------------------------------
prog: supervise_stn.o
	$(BPFTOOL) prog load ./supervise_stn.o /sys/fs/bpf/stn-supervise
	@echo "Programme eBPF chargé : /sys/fs/bpf/stn-supervise (root requis)"

supervise_stn.o: supervise_stn.bpf.c
	$(CLANG) -O2 -g -target bpf -c $< -o $@

clean:
	rm -f supervise_stn.o