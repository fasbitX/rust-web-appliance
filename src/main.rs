// ═══════════════════════════════════════════════════════════════════
// Rust Web Appliance — HermitOS Unikernel Entry Point
// ═══════════════════════════════════════════════════════════════════
//
// The `hermit` crate IS the kernel. Linking it (via `use hermit as _`)
// causes the entire HermitOS kernel to be statically linked into this
// binary. Before main() runs, the kernel has already:
//
//   1. Initialized the CPU, GDT, IDT, page tables
//   2. Set up the global allocator (heap)
//   3. Enumerated PCI devices and probed virtio drivers
//   4. Initialized virtio-net and run DHCPv4 (acquired IP)
//   5. Mounted VirtioFS if available
//   6. Configured COM1 serial for println! output
//
// This means our application code uses normal std:: APIs.
// No unsafe PCI poking, no manual allocator setup.
// ═══════════════════════════════════════════════════════════════════

#[cfg(target_os = "hermit")]
use hermit as _;

mod admin;
mod api;
mod http;
mod ports;
mod router;
mod security;
mod server;
pub mod smtp;
mod static_files;
mod storage;
mod tls;

fn main() {
    // ── Serial Lifeline ─────────────────────────────────────────────
    println!("════════════════════════════════════════════════════════");
    println!("  Rust Web Appliance v{}", env!("CARGO_PKG_VERSION"));
    println!("  Build: {} ({})",
        env!("CARGO_PKG_NAME"),
        if cfg!(debug_assertions) { "debug" } else { "release" }
    );
    println!("  Security: TLS + API key authentication");
    println!("════════════════════════════════════════════════════════");
    println!();

    println!("[boot] Target OS: hermit (HermitOS unikernel)");
    println!("[boot] Architecture: x86_64");
    println!("[boot] The kernel has already:");
    println!("[boot]   - Initialized CPU, memory, allocator");
    println!("[boot]   - Enumerated PCI bus, probed virtio drivers");
    println!("[boot]   - Acquired IP via DHCPv4 (virtio-net)");
    println!("[boot]   - Mounted VirtioFS (if configured)");
    println!();

    // ── Initialize storage layer ────────────────────────────────────
    println!("[init] Initializing storage layer...");
    let storage: &'static storage::Storage = match storage::Storage::init() {
        Ok(store) => {
            println!("[init] Storage ready");
            Box::leak(Box::new(store))
        }
        Err(e) => {
            eprintln!("[FATAL] Storage init failed: {}", e);
            eprintln!("[FATAL] Is VirtioFS mounted? Check QEMU args.");
            return;
        }
    };

    // ── Initialize security layer ───────────────────────────────────
    println!("[init] Initializing security...");
    let security: &'static security::SecurityConfig = Box::leak(Box::new(
        security::SecurityConfig::load(storage),
    ));
    println!();

    // ── Initialize TLS ──────────────────────────────────────────────
    println!("[init] Initializing TLS...");
    let tls_config = match tls::init() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("[FATAL] TLS init failed: {}", e);
            eprintln!("[FATAL] Check cert files at /data/tls/ or embedded certs");
            return;
        }
    };
    println!();

    // ── Initialize Admin Console ──────────────────────────────────
    println!("[init] Initializing admin console...");
    let tls_holder = std::sync::Arc::new(std::sync::RwLock::new(tls_config));

    let admin_state: &'static admin::AdminState = Box::leak(Box::new(admin::AdminState {
        auth: admin::auth::AdminAuth::init(),
        sessions: admin::session::SessionStore::new(),
        log_buffer: admin::logs::LogBuffer::new(),
        tls_config: std::sync::Arc::clone(&tls_holder),
        boot_time: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        request_count: std::sync::atomic::AtomicU64::new(0),
    }));
    println!("[init] Admin console ready at /admin/");
    println!();

    // ── Load port configuration ──────────────────────────────────────
    println!("[init] Loading port configuration...");
    let port_config = ports::PortConfig::load(storage);
    println!("[init] VHost: {}", if port_config.vhost.is_empty() { "(any)" } else { &port_config.vhost });
    println!("[init] Port 80  (HTTP):  {}", if port_config.http.mode == "redirect" { "redirect -> HTTPS" } else { "off" });
    println!("[init] Port 443 (HTTPS): {}", if port_config.https.enabled { "on" } else { "off" });
    println!("[init] Port 8443 (API):  {}", if port_config.api.enabled { "on" } else { "off" });
    println!();

    // ── Start the multi-port server ──────────────────────────────────
    println!("[init] Starting server...");
    println!();

    if let Err(e) = server::run(&port_config, tls_holder, storage, security, admin_state) {
        eprintln!("[FATAL] Server error: {}", e);
    }

    eprintln!("[FATAL] Main loop exited — this should not happen");
}
