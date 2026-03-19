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

mod api;
mod server;
mod router;
mod storage;
mod static_files;

fn main() {
    // ── Serial Lifeline ─────────────────────────────────────────────
    // All println! output goes to COM1 serial. QEMU with `-serial stdio`
    // will display this in the terminal. If anything panics, the panic
    // handler also writes here.
    println!("════════════════════════════════════════════════════════");
    println!("  Rust Web Appliance v{}", env!("CARGO_PKG_VERSION"));
    println!("  Build: {} ({})",
        env!("CARGO_PKG_NAME"),
        if cfg!(debug_assertions) { "debug" } else { "release" }
    );
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
    // Leaked into 'static so all handler threads can reference it
    // without Arc overhead. This is intentional — storage lives for
    // the entire appliance lifetime (i.e., until power off).
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

    // ── Start the web server ────────────────────────────────────────
    let bind_addr = "0.0.0.0:8080";
    println!("[init] Starting HTTP server on {}", bind_addr);
    println!();

    if let Err(e) = server::run(bind_addr, storage) {
        eprintln!("[FATAL] Server error: {}", e);
    }

    eprintln!("[FATAL] Main loop exited — this should not happen");
}
