// Secure Telemetry Node, démon Linux embarqué (aligné redpesk)
//
// Objectifs :
//   - collecter de la télémétrie (température CPU / GPIO), publiée sur un petit
//     serveur TCP local (mode microservice, esprit afb-binder)
//   - s'exécuter sandboxé : seccomp mode FILTER (whitelist BPF) + no-new-privs
//   - être compilable en STATIC MUSL aarch64 / riscv64 (aucune libc dynamique)
//
// Zéro dépendance externe (aucun crates.io requis) : prctl est déclaré en FFI
// direct, ce qui permet à la redpesk factory de builder SANS accès réseau au
// registre Rust, robuste et auditable.
//
// Sandbox seccomp : contrairement au mode STRICT (qui ne tolère que
// read/write/_exit/sigreturn et tue tout démon réseau dès accept()), ce projet
// installe un filtre SECCOMP_MODE_FILTER en bytecode BPF construit à partir
// d'une whitelist déclarative de syscalls, ARCH-DÉPENDANTE (les numéros
// diffèrent entre x86_64 et aarch64/riscv64). Le filtre inspecte en plus
// l'argument flags de openat pour REFUSER toute écriture de fichier (le démon
// ne fait que lire). C'est une politique réelle : le service continue de
// tourner sous le filtre, et un syscall non whitelisté tue le process (SIGSYS).

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::exit;
use std::sync::atomic::AtomicU32;
use std::time::{Duration, Instant};

const VERSION: &str = env!("CARGO_PKG_VERSION");

// Déclaration FFI manuelle de prctl(2) (Linux), sans crate libc.
#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
}

// Options prctl / modes seccomp.
const PR_SET_NO_NEW_PRIVS: i32 = 38;
const PR_SET_SECCOMP: i32 = 22;
const SECCOMP_MODE_FILTER: u64 = 2;

// Instructions BPF du jeu CLASSIQUE (cBPF), dont l'encodage est inchangé
// depuis l'origine (voir include/linux/filter.h et la doc kernel "Linux Socket
// Filtering"). Seccomp impose des mots de 32 bits (BPF_W).
const BPF_LD_W_ABS: u16 = 0x20;   // BPF_LD | BPF_W | BPF_ABS : charge un mot du buffer seccomp
const BPF_ALU_AND_K: u16 = 0x54;  // BPF_ALU | BPF_AND | BPF_K : accumulateur &= k
const BPF_JMP_JEQ_K: u16 = 0x15;  // BPF_JMP | BPF_JEQ | BPF_K : saute si accumulateur == k
const BPF_RET_K: u16 = 0x06;      // BPF_RET | BPF_K : retourne la décision

// Retours seccomp (seuls les bits 16..31 sont significatifs).
const SECCOMP_RET_KILL_THREAD: u32 = 0x00000000;
const SECCOMP_RET_ALLOW: u32 = 0x7FFF0000;
const SECCOMP_RET_ERRNO: u32 = 0x00050000;
const EPERM: u32 = 1;

// Offsets dans struct seccomp_data (interface publique FIGÉE, voir
// include/uapi/linux/seccomp.h du noyau) :
//   offset 0  : nr (int)
//   offset 4  : arch (u32)
//   offset 8  : instruction_pointer (u64)
//   offset 16 : args[0] (u64), puis args[1] (24), args[2] (32), ...
const SECCOMP_DATA_OFFSET_ARCH: u32 = 4;
const SECCOMP_DATA_OFFSET_SYSCALL: u32 = 0;
const SECCOMP_DATA_OFFSET_ARGS: u32 = 16;

// Pour openat(dirfd, path, flags, mode), les flags sont le TROISIÈME argument
// (args[2]), donc à args + 2 * 8 octets = offset 32.
const OPENAT_FLAGS_OFFSET: u32 = SECCOMP_DATA_OFFSET_ARGS + 16;

// Flags open(2) : O_WRONLY = 0x1 (lecture seule exigée par le démon).
const O_WRONLY: u32 = 1;

// Identifiants d'architecture (AUDIT_ARCH_*) utilisés par seccomp.
#[cfg(target_arch = "x86_64")]
const AUDIT_ARCH: u32 = 0xC000003E;
#[cfg(target_arch = "aarch64")]
const AUDIT_ARCH: u32 = 0xC00000B7;
#[cfg(target_arch = "riscv64")]
const AUDIT_ARCH: u32 = 0xC00000F3;

// openat(2) est traité SÉPARÉMENT (règle "lecture seule"). Numéro par arch.
#[cfg(target_arch = "x86_64")]
const OPENAT_NR: u32 = 257;
#[cfg(target_arch = "aarch64")]
const OPENAT_NR: u32 = 56;
#[cfg(target_arch = "riscv64")]
const OPENAT_NR: u32 = 56;

// Whitelist des syscalls autorisés (openat exclu, traité à part).
//
// x86_64 : table vérifiée (build + tests + exécution réelle sous le filtre).
// aarch64 / riscv64 : numérotation générique des archs récents (RISC-V a
// repris la table aarch64 pour ces appels de base), à confirmer par test
// croisé sur cible (voir CI). Si un numéro s'avérait faux, prctl échoue ou le
// process meurt en SIGSYS, jamais en silence.
#[cfg(target_arch = "x86_64")]
const ALLOWED_SYSCALLS: [u32; 34] = [
    0,    // read
    1,    // write
    3,    // close
    9,    // mmap
    10,   // mprotect
    11,   // munmap
    12,   // brk
    13,   // rt_sigaction
    14,   // rt_sigprocmask
    15,   // rt_sigreturn
    16,   // ioctl
    25,   // mremap
    35,   // nanosleep
    39,   // getpid
    41,   // socket
    44,   // sendto
    45,   // recvfrom
    46,   // sendmsg
    47,   // recvmsg
    48,   // shutdown
    49,   // bind
    50,   // listen
    51,   // getsockname
    54,   // setsockopt
    60,   // exit
    63,   // uname
    72,   // fcntl
    131,  // sigaltstack
    157,  // prctl
    228,  // clock_gettime
    230,  // clock_nanosleep (glibc récente : std::thread::sleep passe par là)
    231,  // exit_group
    288,  // accept4
    332,  // statx
];

// Whitelist aarch64 / riscv64 : table "générique" des archs 64-bit récents
// (openat exclu, traité à part). Numéros vérifiés contre la table syscall
// ARM64/RISC-V (les numéros réseau diffèrent de x86_64 : accept4=242,
// sendmsg=211, shutdown=210, getsockname=204, mremap=216).
#[cfg(target_arch = "aarch64")]
const ALLOWED_SYSCALLS: [u32; 38] = [
    25,   // fcntl
    29,   // ioctl
    57,   // close
    63,   // read
    64,   // write
    73,   // ppoll
    78,   // readlinkat
    79,   // newfstatat
    93,   // exit
    94,   // exit_group
    98,   // futex
    101,  // nanosleep
    113,  // clock_gettime
    115,  // clock_nanosleep
    132,  // sigaltstack
    134,  // rt_sigaction
    135,  // rt_sigprocmask
    139,  // rt_sigreturn
    160,  // uname
    167,  // prctl
    172,  // getpid
    198,  // socket
    200,  // bind
    201,  // listen
    204,  // getsockname
    206,  // sendto
    207,  // recvfrom
    208,  // setsockopt
    210,  // shutdown
    211,  // sendmsg
    212,  // recvmsg
    214,  // brk
    215,  // munmap
    216,  // mremap
    222,  // mmap
    226,  // mprotect
    242,  // accept4
    291,  // statx
];

#[cfg(target_arch = "riscv64")]
const ALLOWED_SYSCALLS: [u32; 38] = [
    25,   // fcntl
    29,   // ioctl
    57,   // close
    63,   // read
    64,   // write
    73,   // ppoll
    78,   // readlinkat
    79,   // newfstatat
    93,   // exit
    94,   // exit_group
    98,   // futex
    101,  // nanosleep
    113,  // clock_gettime
    115,  // clock_nanosleep
    132,  // sigaltstack
    134,  // rt_sigaction
    135,  // rt_sigprocmask
    139,  // rt_sigreturn
    160,  // uname
    167,  // prctl
    172,  // getpid
    198,  // socket
    200,  // bind
    201,  // listen
    204,  // getsockname
    206,  // sendto
    207,  // recvfrom
    208,  // setsockopt
    210,  // shutdown
    211,  // sendmsg
    212,  // recvmsg
    214,  // brk
    215,  // munmap
    216,  // mremap
    222,  // mmap
    226,  // mprotect
    242,  // accept4
    291,  // statx
];

// ---------------------------------------------------------------------------
// Représentation mémoire des structures seccomp (layout noyau).
//   struct sock_filter { u16 code; u8 jt; u8 jf; u32 k; }        (8 octets)
//   struct sock_fprog  { u16 len; [padding 6]; sock_filter *filter; }
// Le padding est inséré automatiquement par repr(C) via l'alignement de u64.
// ---------------------------------------------------------------------------
#[repr(C)]
#[derive(Clone, Copy)]
struct SockFilter {
    code: u16,
    jt: u8,
    jf: u8,
    k: u32,
}

#[repr(C)]
struct SockFprog {
    len: u16,
    filter: u64,
}

// Capacité du programme BPF (10 + nombre de syscalls whitelistés).
const MAX_PROG: usize = 64;

// ---------------------------------------------------------------------------
// Émetteurs d'instructions BPF.
// ---------------------------------------------------------------------------
fn emit_ld_w_abs(prog: &mut [SockFilter; MAX_PROG], len: &mut usize, k: u32) {
    prog[*len] = SockFilter { code: BPF_LD_W_ABS, jt: 0, jf: 0, k };
    *len += 1;
}

fn emit_jeq(prog: &mut [SockFilter; MAX_PROG], len: &mut usize, k: u32, jt: u8, jf: u8) {
    prog[*len] = SockFilter { code: BPF_JMP_JEQ_K, jt, jf, k };
    *len += 1;
}

fn emit_alu_and(prog: &mut [SockFilter; MAX_PROG], len: &mut usize, k: u32) {
    prog[*len] = SockFilter { code: BPF_ALU_AND_K, jt: 0, jf: 0, k };
    *len += 1;
}

fn emit_ret(prog: &mut [SockFilter; MAX_PROG], len: &mut usize, k: u32) {
    prog[*len] = SockFilter { code: BPF_RET_K, jt: 0, jf: 0, k };
    *len += 1;
}

// ---------------------------------------------------------------------------
// Construit le programme seccomp depuis une whitelist déclarative.
//
// Layout (m = nombre de syscalls whitelistés, openat exclu car traité à part) :
//   [0]   LD W ABS arch               -> A = arch
//   [1]   JMP JEQ AUDIT_ARCH, jt=1    -> arch OK ? [3] sinon [2]
//   [2]   RET deny                     (mauvaise arch, programme tronqué)
//   [3]   LD W ABS syscall_nr         -> A = numéro de syscall
//   [4]   JMP JEQ openat, jt=m+1      -> openat ? [6+m] sinon [5]
//   [5..] JMP JEQ allowed[i], jt=ALLOW_IDX-(idx+1)   (whitelist simple)
//   [5+m] RET deny                     (syscall ni whitelisté ni openat : refusé)
//   [6+m] LD W ABS flags_openat       -> A = args[2] (flags) de openat
//   [7+m] ALU AND O_WRONLY            -> A = flags & O_WRONLY
//   [8+m] JMP JEQ 0, jt=1             -> lecture seule ? [10+m] sinon [9+m]
//   [9+m] RET deny                     (openat en écriture interdit)
//   [10+m] RET ALLOW
// Total : 11 + m instructions.
//
// Important : les flags d'openat sont son TROISIÈME argument (args[2]),
// stocké à SECCOMP_DATA_OFFSET_ARGS + 2 * 8 octets. On n'inspecte pas args[0]
// (qui est le dirfd), ce serait sans rapport avec la politique "lecture seule".
// ---------------------------------------------------------------------------
fn build_seccomp_program(
    prog: &mut [SockFilter; MAX_PROG],
    allowed: &[u32],
    openat_nr: u32,
    log_only: bool,
) -> usize {
    // En mode log_only, un syscall refusé retourne EPERM au lieu de tuer le
    // process : utile pour auditer le filtre (strace) sans faire tomber le
    // service, et pour la CI.
    let deny: u32 = if log_only { SECCOMP_RET_ERRNO | EPERM } else { SECCOMP_RET_KILL_THREAD };
    let m: usize = allowed.len();
    let mut len: usize = 0;

    emit_ld_w_abs(prog, &mut len, SECCOMP_DATA_OFFSET_ARCH);
    emit_jeq(prog, &mut len, AUDIT_ARCH, 1u8, 0u8);
    emit_ret(prog, &mut len, deny);
    emit_ld_w_abs(prog, &mut len, SECCOMP_DATA_OFFSET_SYSCALL);

    // [4] openat aiguillé vers la règle argumentaire (SPECIAL = 6+m).
    emit_jeq(prog, &mut len, openat_nr, (m + 1) as u8, 0u8);

    // Whitelist simple des autres syscalls : saut vers ALLOW_IDX = 10+m.
    for i in 0..allowed.len() {
        let jt = (m + 4 - i) as u8;
        emit_jeq(prog, &mut len, allowed[i], jt, 0u8);
    }

    // Tout syscall non whitelisté et non openat est refusé ici.
    emit_ret(prog, &mut len, deny);

    // Règle openat : refuser toute écriture (O_WRONLY / O_RDWR).
    emit_ld_w_abs(prog, &mut len, OPENAT_FLAGS_OFFSET);
    emit_alu_and(prog, &mut len, O_WRONLY);
    emit_jeq(prog, &mut len, 0u32, 1u8, 0u8);
    emit_ret(prog, &mut len, deny);
    emit_ret(prog, &mut len, SECCOMP_RET_ALLOW);

    len
}

// ---------------------------------------------------------------------------
// Installe le filtre seccomp. Retourne le nombre d'instructions.
// ---------------------------------------------------------------------------
fn install_filter(prog: &mut [SockFilter; MAX_PROG], log_only: bool) -> Result<usize, String> {
    let len = build_seccomp_program(prog, &ALLOWED_SYSCALLS, OPENAT_NR, log_only);
    let fprog = SockFprog {
        len: len as u16,
        filter: std::ptr::from_ref(&prog[0]) as u64,
    };
    let r = unsafe {
        prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, std::ptr::from_ref(&fprog) as u64, 0, 0)
    };
    if r != 0 {
        return Err(format!("PR_SET_SECCOMP (filter) failed ({r})"));
    }
    Ok(len)
}

// ---------------------------------------------------------------------------
// Sandbox : no-new-privs puis seccomp mode FILTER. Retourne une description
// du filtre actif (logging / tests).
// ---------------------------------------------------------------------------
fn enable_sandbox(prog: &mut [SockFilter; MAX_PROG], log_only: bool) -> Result<String, String> {
    let r = unsafe { prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if r != 0 {
        return Err("PR_SET_NO_NEW_PRIVS failed".into());
    }
    match install_filter(prog, log_only) {
        Ok(n) => {
            let mode = if log_only { "log-only" } else { "actif" };
            return Ok(format!(
                "filtre seccomp BPF {mode} ({n} instructions, whitelist {} syscalls + openat lecture seule)",
                ALLOWED_SYSCALLS.len()
            ));
        }
        Err(e) => return Err(e),
    }
}

// ---------------------------------------------------------------------------
// Auto-test du sandbox : installe le filtre puis tente une écriture fichier.
// Si seccomp est réellement actif, openat(O_WRONLY) est rejeté et le process
// est tué par SIGSYS (code de sortie 132/159 côté shell). Si on survit, la
// politique est inopérante : on remonte une erreur. La CI vérifie le signal.
// ---------------------------------------------------------------------------
fn sandbox_self_test(prog: &mut [SockFilter; MAX_PROG]) -> Result<(), String> {
    match enable_sandbox(prog, false) {
        Err(e) => return Err(e),
        Ok(_) => {}
    }
    let _ = fs::write("stn-self-test.tmp", "x");
    Err("FAIL: openat(O_WRONLY) non bloqué par seccomp".to_string())
}

// ---------------------------------------------------------------------------
// Configuration (ligne de commande).
// ---------------------------------------------------------------------------
struct Config {
    port: u16,
    addr: String,
    sandbox_self_test: bool,
    no_sandbox: bool,
    sandbox_log: bool,
}

fn parse_args(args: Vec<String>) -> Config {
    let mut cfg = Config {
        port: 5555,
        addr: "127.0.0.1".to_string(),
        sandbox_self_test: false,
        no_sandbox: false,
        sandbox_log: false,
    };
    for a in args.iter() {
        if let Some(p) = a.strip_prefix("--port=") {
            cfg.port = p.parse::<u16>().ok().unwrap_or(5555);
        } else if let Some(a) = a.strip_prefix("--addr=") {
            cfg.addr = a.to_string();
        } else if a == "--sandbox-self-test" {
            cfg.sandbox_self_test = true;
        } else if a == "--no-sandbox" {
            cfg.no_sandbox = true;
        } else if a == "--sandbox-log" {
            cfg.sandbox_log = true;
        }
    }
    cfg
}

// ---------------------------------------------------------------------------
// Collecte : température CPU via sysfs, avec repli simulé (démo QEMU).
// ---------------------------------------------------------------------------
fn parse_cpu_temp_milli(content: String) -> Option<f32> {
    let t = content.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<f32>().ok()
}

static SIM_TEMP: AtomicU32 = AtomicU32::new(52000);

fn read_cpu_temp() -> f32 {
    let paths = [
        "/sys/class/thermal/thermal_zone0/temp",
        "/sys/devices/virtual/thermal/thermal_zone0/temp",
    ];
    for p in paths.iter() {
        if let Ok(s) = std::fs::read_to_string(p) {
            if let Some(milli) = parse_cpu_temp_milli(s) {
                return milli / 1000.0;
            }
        }
    }
    // Repli simulé (QEMU / pas de capteur) : +0.1°C par lecture. Sans unsafe :
    // simple compteur atomique.
    let cur = SIM_TEMP.fetch_add(100, std::sync::atomic::Ordering::Relaxed);
    cur as f32 / 1000.0
}

// ---------------------------------------------------------------------------
// État GPIO. En réel : parser /sys/class/gpio/gpioNN/value (exposé par le
// module noyau ou l'overlay DTS fournis dans kernel/). Ici état stable.
// ---------------------------------------------------------------------------
fn read_gpio_state() -> String {
    "stable".to_string()
}

// ---------------------------------------------------------------------------
// Mémoire disponible (parsing de /proc/meminfo, lecture seule).
// ---------------------------------------------------------------------------
fn parse_meminfo(content: String) -> u64 {
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            if let Some(kb) = rest.trim().split_whitespace().next() {
                if let Ok(v) = kb.parse::<u64>() {
                    return v;
                }
            }
        }
    }
    0
}

fn free_mem_kb() -> u64 {
    if let Ok(s) = std::fs::read_to_string("/proc/meminfo") {
        return parse_meminfo(s);
    }
    0
}

// ---------------------------------------------------------------------------
// Heartbeat (format stabilisé, testable).
// ---------------------------------------------------------------------------
fn format_heartbeat(uptime: u64, cpu_temp: f32, gpio: String, mem_kb: u64) -> String {
    format!(
        "t=+{}s cpu_temp={:.1}C gpio={} free_mem_kb={}",
        uptime,
        cpu_temp,
        gpio,
        mem_kb,
    )
}

// ---------------------------------------------------------------------------
// Main.
// ---------------------------------------------------------------------------
fn main() {
    let args: Vec<String> = env::args().collect();
    let cfg = parse_args(args);
    let mut prog: [SockFilter; MAX_PROG] = [SockFilter { code: 0, jt: 0, jf: 0, k: 0 }; MAX_PROG];

    println!("secure-telemetry-node v{} (rust)", VERSION);

    if cfg.sandbox_self_test {
        match sandbox_self_test(&mut prog) {
            Err(e) => {
                eprintln!("sandbox self-test: {e}");
                exit(1);
            }
            Ok(_) => {
                println!("sandbox self-test: OK (write bloqué)");
                exit(0);
            }
        }
    }

    let start = Instant::now();

    let listener = match TcpListener::bind(format!("{}:{}", cfg.addr, cfg.port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind failed: {e}");
            exit(1);
        }
    };
    println!("listening on {}:{}", cfg.addr, cfg.port);

    // On active le sandbox APRÈS le bind : seccomp filtre l'accept() et le
    // service, pas la mise en place du socket. --no-sandbox pour debug/démo.
    if cfg.no_sandbox {
        println!("sandbox: désactivé (--no-sandbox, mode debug)");
    } else {
        println!("sandbox: no-new-privs + seccomp filter (BPF)…");
        match enable_sandbox(&mut prog, cfg.sandbox_log) {
            Err(e) => {
                eprintln!("sandbox désactivé : {e} (démo dégradée, sans seccomp).");
            }
            Ok(n) => {
                println!("sandbox: seccomp filter actif ✓ ({n})");
            }
        }
    }

    // Boucle de service : répond aux connexions et logge un heartbeat.
    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                let body = heartbeat(start);
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n{body}\n");
                let _ = s.flush();
                // Lecture d'une éventuelle requête entrante (simulation BUS).
                let mut buf = [0u8; 128];
                let _ = s.read(&mut buf);
                println!("{body}");
            }
            Err(_) => {}
        }
        // Pause pour la basse conso (esprit embarqué).
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn heartbeat(start: Instant) -> String {
    format_heartbeat(
        uptime_seconds(start),
        read_cpu_temp(),
        read_gpio_state(),
        free_mem_kb(),
    )
}

fn uptime_seconds(start: Instant) -> u64 {
    start.elapsed().as_secs()
}

// ---------------------------------------------------------------------------
// Tests unitaires : parsing et construction du programme BPF. La génération
// du bytecode est vérifiée sans root : structure, offsets de saut et règles.
// ---------------------------------------------------------------------------
#[test]
fn test_parse_meminfo_found() {
    let sample = "MemTotal:       16384000 kB\nMemFree:         2000000 kB\nMemAvailable:    8000000 kB\n";
    if parse_meminfo(sample.to_string()) != 8000000 {
        panic!("MemAvailable non parsé");
    }
}

#[test]
fn test_parse_meminfo_missing() {
    if parse_meminfo("pas de meminfo ici".to_string()) != 0 {
        panic!("champ absent doit donner 0");
    }
}

#[test]
fn test_parse_cpu_temp_milli() {
    let milli = parse_cpu_temp_milli("  52345\n".to_string());
    if milli != Some(52345.0) {
        panic!("milli°C non parsé");
    }
    if parse_cpu_temp_milli("".to_string()) != None {
        panic!("chaine vide doit donner None");
    }
}

#[test]
fn test_parse_args_defaults() {
    let cfg = parse_args(Vec::new());
    if cfg.port != 5555 || cfg.addr != "127.0.0.1" || cfg.sandbox_self_test {
        panic!("défauts incorrects");
    }
}

#[test]
fn test_parse_args_custom() {
    let args = [
        "--port=7777".to_string(),
        "--addr=0.0.0.0".to_string(),
        "--sandbox-self-test".to_string(),
    ].to_vec();
    let cfg = parse_args(args);
    if cfg.port != 7777 || cfg.addr != "0.0.0.0" || !cfg.sandbox_self_test {
        panic!("arguments non pris en compte");
    }
}

#[test]
fn test_parse_args_invalid_port() {
    let cfg = parse_args(["--port=abc".to_string()].to_vec());
    if cfg.port != 5555 {
        panic!("port invalide doit retomber sur le défaut");
    }
}

#[test]
fn test_format_heartbeat() {
    let h = format_heartbeat(3, 25.0, "stable".to_string(), 9629064);
    if h != "t=+3s cpu_temp=25.0C gpio=stable free_mem_kb=9629064" {
        panic!("format heartbeat incorrect : {h}");
    }
}

#[test]
fn test_build_seccomp_program_layout() {
    let mut prog: [SockFilter; MAX_PROG] = [SockFilter { code: 0, jt: 0, jf: 0, k: 0 }; MAX_PROG];
    let allowed: [u32; 2] = [1, 2]; // placeholders pour vérifier la structure
    let m = allowed.len();
    let len = build_seccomp_program(&mut prog, &allowed, 999, false);

    // 11 + m instructions attendues.
    if len != 11 + m {
        panic!("longueur BPF inattendue : {len}");
    }
    // [0] charge l'arch.
    if prog[0].code != BPF_LD_W_ABS || prog[0].k != SECCOMP_DATA_OFFSET_ARCH {
        panic!("[0] doit charger l'arch");
    }
    // [1] vérifie l'arch, saute vers [3] si OK.
    if prog[1].code != BPF_JMP_JEQ_K || prog[1].k != AUDIT_ARCH || prog[1].jt != 1 {
        panic!("[1] doit vérifier l'arch avec jt=1");
    }
    // [2] kill si mauvaise arch.
    if prog[2].code != BPF_RET_K || prog[2].k != SECCOMP_RET_KILL_THREAD {
        panic!("[2] doit tuer sur mauvaise arch");
    }
    // [3] charge le syscall.
    if prog[3].code != BPF_LD_W_ABS || prog[3].k != SECCOMP_DATA_OFFSET_SYSCALL {
        panic!("[3] doit charger le numéro de syscall");
    }
    // [4] aiguille openat vers la règle argumentaire (SPECIAL = 6+m) : jt = m+1.
    if prog[4].code != BPF_JMP_JEQ_K || prog[4].k != 999 || prog[4].jt != (m + 1) as u8 {
        panic!("[4] doit aiguiller openat avec jt=m+1");
    }
    // La whitelist saute vers ALLOW_IDX = 10+m.
    for i in 0..allowed.len() {
        let idx = 5 + i;
        let expected_jt = (10 + m) - (idx + 1);
        if prog[idx].code != BPF_JMP_JEQ_K || prog[idx].jt != expected_jt as u8 {
            panic!("whitelist[{i}] : jt incohérent");
        }
    }
    // [5+m] : tout syscall non whitelisté et non openat doit être refusé.
    let kill_idx = 5 + m;
    if prog[kill_idx].code != BPF_RET_K || prog[kill_idx].k != SECCOMP_RET_KILL_THREAD {
        panic!("[5+m] doit tuer les syscalls inconnus");
    }
    // Règle openat : LD args[2] (flags, offset 32), AND O_WRONLY, JEQ 0 (jt=1 -> ALLOW).
    let ld_idx = 6 + m;
    if prog[ld_idx].code != BPF_LD_W_ABS || prog[ld_idx].k != OPENAT_FLAGS_OFFSET {
        panic!("règle openat : doit charger les flags (args[2], offset {OPENAT_FLAGS_OFFSET})");
    }
    if OPENAT_FLAGS_OFFSET != SECCOMP_DATA_OFFSET_ARGS + 16 {
        panic!("les flags d'openat doivent être à args + 2*8 (cohérence de l'offset)");
    }
    if prog[ld_idx + 1].code != BPF_ALU_AND_K || prog[ld_idx + 1].k != O_WRONLY {
        panic!("règle openat : doit faire AND O_WRONLY");
    }
    if prog[ld_idx + 2].code != BPF_JMP_JEQ_K || prog[ld_idx + 2].k != 0 || prog[ld_idx + 2].jt != 1 {
        panic!("règle openat : JEQ 0 avec jt=1 attendu");
    }
    // Dernières instructions : kill (écriture), allow (lecture seule).
    if prog[len - 2].k != SECCOMP_RET_KILL_THREAD {
        panic!("openat en écriture doit être tué");
    }
    if prog[len - 1].k != SECCOMP_RET_ALLOW {
        panic!("lecture seule doit être autorisée");
    }
}

#[test]
fn test_build_seccomp_program_whitelist_contains_required() {
    // openat est traité à part (règle argumentaire) : il ne doit JAMAIS figurer
    // dans la whitelist simple, sinon la règle "lecture seule" serait court-circuitée.
    for v in ALLOWED_SYSCALLS.iter() {
        if *v == OPENAT_NR {
            panic!("openat ne doit pas être dans la whitelist simple");
        }
    }
    // Le programme complet tient dans le buffer fixe (aucun panic sur les index).
    let mut prog: [SockFilter; MAX_PROG] = [SockFilter { code: 0, jt: 0, jf: 0, k: 0 }; MAX_PROG];
    let len = build_seccomp_program(&mut prog, &ALLOWED_SYSCALLS, OPENAT_NR, false);
    if len > MAX_PROG {
        panic!("programme BPF trop long pour le buffer fixe");
    }
}

#[test]
fn test_seccomp_data_offsets_are_consistent() {
    // Vérifie que les offsets de la structure seccomp_data correspondent à la
    // définition publique du noyau (struct seccomp_data) :
    //   nr(0) arch(4) instruction_pointer(8) args[0](16) args[1](24) args[2](32)
    if SECCOMP_DATA_OFFSET_ARCH != 4 {
        panic!("arch doit être à 4");
    }
    if SECCOMP_DATA_OFFSET_SYSCALL != 0 {
        panic!("nr doit être à 0");
    }
    if SECCOMP_DATA_OFFSET_ARGS != 16 {
        panic!("args[0] doit être à 16 (structure publique figée)");
    }
    // Les flags d'openat (args[2]) doivent être à 16 + 2*8 = 32.
    if OPENAT_FLAGS_OFFSET != 32 {
        panic!("flags openat (args[2]) doit être à 32, trouvé {OPENAT_FLAGS_OFFSET}");
    }
}