// supervise_stn.bpf.c - programme eBPF de supervision du daemon
// secure-telemetry-node, chargé avec bpftool.
//
// Démonstration du tooling eBPF "ingénieur kernel" : un programme C compilé
// pour la machine virtuelle eBPF, attaché à des tracepoints, avec des maps
// pour accumuler les compteurs. Complète la démo bpftrace (supervise-stn.bt).
//
// Ce que le programme fait :
//   - compte les syscalls du daemon (tracepoint raw_syscalls:sys_enter) ;
//   - compte les signaux SIGSYS générés (tracepoint signal:signal_generate) :
//     c'est la preuve que seccomp BLOQUE réellement des appels (écritures).
//
// Compilation (voir Makefile) :
//   clang -O2 -g -target bpf -c supervise_stn.bpf.c -o supervise_stn.o
//
// Chargement (root/CAP_BPF) :
//   bpftool prog load ./supervise_stn.o /sys/fs/bpf/stn-supervise
//   bpftool map dump name stn_syscalls      # compteur de syscalls
//   bpftool map dump name stn_sigsys        # compteur de SIGSYS
//
// Nettoyage :
//   rm -f /sys/fs/bpf/stn-supervise

#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

#define BPF_ANY 0

// Types des tracepoints (définis localement pour rester auto-suffisant ;
// ils correspondent aux champs publics documentés dans include/trace/events).

// raw_syscalls:sys_enter : le seul champ d'argument est le numéro de syscall.
// Le PID s'obtient avec bpf_get_current_pid_tgid().
struct tracepoint_raw_syscalls_sys_enter {
    __u16 id;
};

// signal:signal_generate : champs publics documentés (sig, info, task, group,
// result). Le PID du process courant (celui qui génère le signal) s'obtient
// avec bpf_get_current_pid_tgid().
struct tracepoint_signal_signal_generate {
    __s32 sig;
    __s64 info;
    __s64 task;
    __s32 group;
    __s32 result;
};

// Maps de comptage (clé = PID, valeur = compteur).
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 1024);
    __type(key, __u32);
    __type(value, __u64);
} stn_syscalls __weak = {};

struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 1024);
    __type(key, __u32);
    __type(value, __u64);
} stn_sigsys __weak = {};

// Incrémente un compteur dans une map pour un PID donné.
static void __always_inline inc_counter(void *map, __u32 pid)
{
    __u64 *v;
    v = bpf_map_lookup_elem(map, &pid);
    if (v)
        __sync_fetch_and_add(v, 1);
    else
        bpf_map_update_elem(map, &pid, &(const __u64){1}, BPF_ANY);
}

// Tracepoint raw_syscalls:sys_enter : chaque syscall du process.
SEC("tracepoint/raw_syscalls/sys_enter")
int count_syscall(struct tracepoint_raw_syscalls_sys_enter *args)
{
    __u32 pid = (__u32)(bpf_get_current_pid_tgid() >> 32);
    inc_counter(&stn_syscalls, pid);
    return 0;
}

// Tracepoint signal:signal_generate : SIGSYS généré = syscall bloqué par seccomp.
// Le PID du process fautif (celui qui génère le SIGSYS) est obtenu via le
// helper bpf_get_current_pid_tgid().
SEC("tracepoint/signal/signal_generate")
int count_sigsys(struct tracepoint_signal_signal_generate *args)
{
    if (args->sig != 31)   // SIGSYS
        return 0;
    __u32 pid = (__u32)(bpf_get_current_pid_tgid() >> 32);
    inc_counter(&stn_sigsys, pid);
    return 0;
}

char LICENSE[] SEC("license") = "GPL";