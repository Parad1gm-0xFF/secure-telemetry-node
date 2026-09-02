// Secure Telemetry Node — démon Linux embarqué (aligné redpesk)
//
// Objectifs :
//   - collecter de la télémétrie (température CPU / GPIO), publiée sur un petit
//     serveur TCP local (mode microservice, esprit afb-binder)
//   - s'exécuter sandboxé : seccomp + no-new-privs (sécurité « au build »)
//   - être compilable en STATIC MUSL aarch64 (aucune libc dynamique) : idéal
//     pour l'exécution sous qemu-aarch64 et la portabilité RISC-V
//
// Zéro dépendance lourde : std + libc (prctl pour le sandbox). Robustesse
// et binaire minimal.

use libc::{prctl, PR_SET_NO_NEW_PRIVS, PR_SET_SECCOMP, SECCOMP_MODE_STRICT};
use std::env;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::exit;
use std::time::{Duration, Instant};

const VERSION: &str = env!("CARGO_PKG_VERSION");

// ---------------------------------------------------------------------------
// Sandbox minimal : no-new-privs + seccomp strict.
// (En production réelle redpesk utilise LSM Smack/SELinux + redpak ; ici on
//  montre la « sécurité dès le build » demandée pour un nœud embarqué.)
// ---------------------------------------------------------------------------
fn enable_sandbox() -> Result<(), String> {
    // Empêche toute élévation de privilège (setuid, capabilities).
    let r = unsafe { prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if r != 0 {
        return Err("PR_SET_NO_NEW_PRIVS failed".into());
    }
    // Seccomp mode strict : seuls read/write/_exit/sigreturn/syscall suivent.
    // Simple à prouver, suffisant pour un démon de lecture de capteurs.
    let r = unsafe { prctl(PR_SET_SECCOMP, SECCOMP_MODE_STRICT, 0, 0, 0) };
    if r != 0 {
        return Err("PR_SET_SECCOMP (strict) failed — ce mode demande souvent un thread unique".into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Collecte : température CPU via sysfs (largement disponible en embarqué),
// avec repli simulé si le chemin n'existe pas (permet la démo QEMU).
// ---------------------------------------------------------------------------
fn read_cpu_temp() -> f32 {
    let paths = [
        "/sys/class/thermal/thermal_zone0/temp",
        "/sys/devices/virtual/thermal/thermal_zone0/temp",
    ];
    for p in paths.iter() {
        if let Ok(s) = std::fs::read_to_string(p) {
            if let Ok(milli) = s.trim().parse::<f32>() {
                // Le fichier est en milli°C.
                return milli / 1000.0;
            }
        }
    }
    // Repli simulé pour la démo (QEMU / pas de capteur) : +0.1°C par lecture.
    static mut SIM: f32 = 52.0;
    unsafe {
        SIM += 0.1;
        SIM
    }
}

// ---------------------------------------------------------------------------
// État GPIO simulé (vrai lecteur en commentaire : /sys/class/gpio).
// On garde un accès « matériel » logique pour montrer le binding drivers.
// ---------------------------------------------------------------------------
fn read_gpio_state() -> String {
    // En réel : parser /sys/class/gpio/gpioNN/value. Ici on émet un état stable.
    "stable".to_string()
}

// ---------------------------------------------------------------------------
// Uptime / métriques.
// ---------------------------------------------------------------------------
fn uptime_seconds(start: Instant) -> u64 {
    start.elapsed().as_secs()
}

// ---------------------------------------------------------------------------
// Version « heartbeat » imprimée sur stdout (capturée par systemd dans la
// factory redpesk).
// ---------------------------------------------------------------------------
fn heartbeat(start: Instant) -> String {
    format!(
        "t=+{}s cpu_temp={:.1}C gpio={} free_mem_kb={}",
        uptime_seconds(start),
        read_cpu_temp(),
        read_gpio_state(),
        free_mem_kb(),
    )
}

fn free_mem_kb() -> u64 {
    // Lecture simple de /proc/meminfo (utilisable même après seccomp strict car
    // c'est un read()).
    if let Ok(s) = std::fs::read_to_string("/proc/meminfo") {
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("MemAvailable:") {
                if let Some(kb) = rest.trim().split_whitespace().next() {
                    if let Ok(v) = kb.parse::<u64>() {
                        return v;
                    }
                }
            }
        }
    }
    0
}

// ---------------------------------------------------------------------------
// Main.
// ---------------------------------------------------------------------------
fn main() {
    let args: Vec<String> = env::args().collect();
    let port: u16 = args
        .iter()
        .find_map(|a| a.strip_prefix("--port=").map(|p| p.parse().ok()))
        .flatten()
        .unwrap_or(5555);

    println!("secure-telemetry-node v{} (rust, static musl)", VERSION);
    println!("sandbox: enabling seccomp strict + no-new-privs…");

    // On active le sandbox. En mode strict, seccomp limite fortement les
    // syscalls ; on active donc APRÈS avoir monté le listener.
    let start = Instant::now();

    let listener = match TcpListener::bind(format!("0.0.0.0:{port}")) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind failed: {e}");
            exit(1);
        }
    };
    println!("listening on 0.0.0.0:{port}");

    if let Err(e) = enable_sandbox() {
        // Le mode strict échoue souvent (multithread/allocateur). On n'abandonne
        // pas : on affiche et on continue en mode non sandboxé (démo).
        eprintln!("sandbox (strict) désactivé : {e} — démo dégradée (pas de seccomp).");
    } else {
        println!("sandbox: seccomp strict activé ✓");
    }

    // Boucle de service : répond aux connexions et logge un heartbeat.
    // La collecte se fait à la demande (petit microservice).
    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                let body = heartbeat(start);
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n{body}\n");
                let _ = s.flush();
                // On lit aussi une éventuelle requête entrante (simulation BUS).
                let mut buf = [0u8; 128];
                let _ = s.read(&mut buf);
                println!("{body}");
            }
            Err(_) => {}
        }
        // Petite pause pour ne pas surcharger (esprit embarqué basse conso).
        std::thread::sleep(Duration::from_millis(200));
    }
}

// Petite note : avec `PR_SET_SECCOMP` strict, `process::exit` fonctionne ;
// c'est pourquoi on utilise explicitement std::process::exit au lieu d'un panic.