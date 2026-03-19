<p align="center">
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust"/>
  <img src="https://img.shields.io/badge/HermitOS-Unikernel-ff6b35?style=for-the-badge" alt="HermitOS"/>
  <img src="https://img.shields.io/badge/Target-x86__64-blue?style=for-the-badge" alt="x86_64"/>
  <img src="https://img.shields.io/badge/Binary-1.5MB-success?style=for-the-badge" alt="1.5MB"/>
</p>

<h1 align="center">
  <br>
  <code>rust-web-appliance</code>
  <br>
  <sub>An All-Rust Unikernel Web Server Appliance</sub>
  <br>
</h1>

<p align="center">
  <strong>A complete operating system, TCP/IP network stack, HTTP server, config-driven REST API engine, and web application compiled into a single 1.5 MB bootable binary.</strong>
</p>

<p align="center">
  No Linux. No containers. No systemd. No glibc.<br/>
  Just Rust, all the way down.
</p>

---

## What Is This?

This is a **unikernel** -- not a regular application that runs on top of an operating system, but an application that **is** the operating system. When this binary boots on a hypervisor, there is no kernel beneath it, no shell, no package manager, no cron. There is only your code and the minimal HermitOS runtime needed to talk to virtual hardware.

```
 Traditional Web Server Stack            Rust Web Appliance
 ================================        ================================

  Your App (Node/Python/Go)               Your App (Rust)
  Runtime (V8/CPython/Go)                 + HermitOS Kernel (Rust)
  System Libs (glibc, OpenSSL)            + TCP/IP Stack (smoltcp, Rust)
  Linux Kernel (~30M lines of C)          + HTTP Server (tiny_http, Rust)
  Bootloader (GRUB)                       + Config API Engine (Rust)
                                          + VirtIO Drivers (Rust)
  ~500MB+ image                           ───────────────────────────────
                                          = ONE SINGLE ELF BINARY
                                          ~1.5MB image
```

---

## Two Ways to Build Your Backend

This appliance offers **two backend development modes** that work simultaneously. Pick the one that fits your task -- or use both.

```
            ┌──────────── REQUEST ────────────┐
            │                                 │
            v                                 v
   ┌─────────────────┐            ┌────────────────────┐
   │  Tier 1: Rust   │            │  Tier 2: Config    │
   │  src/api/*.rs   │            │  endpoints.json    │
   │                 │            │                    │
   │  Full power.    │            │  Zero code.        │
   │  Custom logic,  │            │  Drop JSON file,   │
   │  auth, external │            │  reboot, full CRUD │
   │  APIs, anything │            │  API appears.      │
   │                 │            │                    │
   │  Edit Rust,     │            │  Edit JSON,        │
   │  cargo build,   │            │  reboot appliance, │
   │  reboot.        │            │  done.             │
   └────────┬────────┘            └─────────┬──────────┘
            │                               │
            └──────────┬────────────────────┘
                       │
                       v
              ┌─────────────────┐
              │   KV Storage    │
              │  (VirtioFS or   │
              │   in-memory)    │
              └─────────────────┘
```

### Tier 1: Rust API Handlers (`src/api/`)

For custom business logic, authentication, external integrations -- anything that needs code. This is the same edit-restart cycle as Node.js (`edit → cargo build → reboot`), except the output is bare-metal machine code instead of interpreted JS.

**Adding a Rust endpoint -- 3 steps:**

**Step 1.** Create a handler file:

```rust
// src/api/products.rs
use super::Context;

pub fn list(ctx: Context) {
    ctx.json(200, r#"{"products":[]}"#);
}

pub fn get_by_id(ctx: Context) {
    let id = ctx.param();        // "/api/products/42" -> "42"
    ctx.json(200, &format!(r#"{{"id":"{}"}}"#, id));
}

pub fn create(mut ctx: Context) {
    let body = ctx.body_string(); // read POST body
    // ... your logic here
    ctx.json(201, r#"{"status":"created"}"#);
}
```

**Step 2.** Register the module in `src/api/mod.rs`:

```rust
mod products;  // <-- add this line
```

**Step 3.** Add routes to the table in `src/api/mod.rs`:

```rust
Route::get("/api/products",          products::list),
Route::get_prefix("/api/products",   products::get_by_id),
Route::post("/api/products",         products::create),
```

**What `Context` gives you:**

| Method | What It Does |
|--------|-------------|
| `ctx.param()` | Path segment after prefix (`/api/users/42` -> `"42"`) |
| `ctx.params()` | All segments as `Vec<&str>` |
| `ctx.query()` | Query string after `?` |
| `ctx.body_string()` | Read request body as String |
| `ctx.body_json::<T>()` | Parse body into any serde struct |
| `ctx.storage` | Direct access to the KV store |
| `ctx.json(status, body)` | Send JSON response |
| `ctx.json_value(status, &val)` | Serialize any serde value as JSON |
| `ctx.text(status, body)` | Send plain text |
| `ctx.error(status, msg)` | Send `{"error":"..."}` |

### Tier 2: Config-Driven API Engine (`backend/endpoints.json`)

For data-driven CRUD -- products, blog posts, users, any entity you just need to store and retrieve. **No Rust code required.** Edit a JSON file, reboot, your API exists.

**Adding a config-driven collection -- 1 step:**

Edit `backend/endpoints.json`:

```json
{
    "collections": {
        "products": {
            "fields": {
                "name":     { "type": "string", "required": true },
                "price":    { "type": "number", "required": true },
                "category": { "type": "string" },
                "in_stock": { "type": "bool" }
            }
        }
    }
}
```

**What you get automatically (5 endpoints per collection):**

| Generated Endpoint | What It Does |
|---|---|
| `GET /api/products` | List all products |
| `GET /api/products/:id` | Get one product |
| `POST /api/products` | Create (auto-generates ID + timestamp, validates fields) |
| `PUT /api/products/:id` | Update (validates fields, adds `updated_at`) |
| `DELETE /api/products/:id` | Delete |

**Example usage:**

```bash
# Create
curl -X POST http://localhost:8080/api/products \
  -d '{"name":"Widget","price":9.99,"category":"gadgets","in_stock":true}'
# Returns: {"id":"18e5f3a1b0c00000","name":"Widget","price":9.99,...,"created_at":1710806400}

# List
curl http://localhost:8080/api/products
# Returns: {"collection":"products","count":1,"items":[...]}

# Get by ID
curl http://localhost:8080/api/products/18e5f3a1b0c00000

# Update
curl -X PUT http://localhost:8080/api/products/18e5f3a1b0c00000 \
  -d '{"price":12.99}'

# Delete
curl -X DELETE http://localhost:8080/api/products/18e5f3a1b0c00000
```

**Built-in features:**
- Auto-generated hex IDs (timestamp + sequence)
- `created_at` timestamp on POST
- `updated_at` timestamp on PUT
- Field type validation (`string`, `number`, `bool`)
- Required field enforcement
- Merge-on-update (PUT merges new fields into existing, doesn't replace)
- `id` and `created_at` are protected from overwrite

### What Needs Rust vs What's Config-Driven

| Use Case | Config-Driven | Needs Rust |
|---|---|---|
| CRUD endpoints for any data shape | Yes | No |
| Field validation (type, required) | Yes | No |
| Auto-generated IDs + timestamps | Yes | No |
| List all items in a collection | Yes | No |
| Custom business logic | No | Yes |
| Authentication / authorization | No | Yes |
| Calling external APIs | No | Yes |
| Complex computed responses | No | Yes |

### Priority Order

Requests are matched in this order:

1. **Rust routes** (`src/api/mod.rs`) -- first match wins
2. **Config engine** (`backend/endpoints.json`) -- CRUD for defined collections
3. **Static files** (`frontend/`) -- HTML/CSS/JS
4. **404** fallback

Rust routes always win. If you define a Rust handler for `/api/products` AND a config collection called `products`, the Rust handler takes priority.

---

## Frontend Drop Zone (`frontend/`)

Drop your web files here. They're served automatically -- no configuration, no build step.

```
frontend/
├── index.html          <-- Main page (served at /)
├── css/
│   └── style.css       <-- Stylesheets
├── js/
│   └── app.js          <-- JavaScript
├── img/                <-- Images (PNG, JPG, SVG, ICO)
├── fonts/              <-- Web fonts (WOFF2, TTF)
└── pages/              <-- Additional HTML pages
```

The included starter `index.html` + `app.js` provide a working interactive dashboard with health status, appliance info, and a live KV store you can PUT/GET/DELETE from the browser.

If no frontend files are deployed (or VirtioFS isn't available), the appliance serves a built-in embedded index page with API documentation.

**MIME types** are detected automatically from file extensions: `.html`, `.css`, `.js`, `.json`, `.png`, `.jpg`, `.svg`, `.ico`, `.woff2`, `.ttf`, `.wasm`, and more.

---

## Architecture

```
                    ┌─────────────────────────────────────────────┐
                    │            QEMU / KVM Hypervisor            │
                    │                                             │
                    │   ┌─────────────────────────────────────┐   │
                    │   │      rust-web-appliance (ELF)       │   │
                    │   │                                     │   │
                    │   │  ┌───────────┐  ┌───────────────┐   │   │
                    │   │  │ HTTP Srv  │  │ Config Engine  │   │   │
                    │   │  │ tiny_http │  │ endpoints.json │   │   │
                    │   │  └─────┬─────┘  └───────┬───────┘   │   │
                    │   │        │                 │           │   │
                    │   │  ┌─────┴─────┐    ┌─────┴─────┐     │   │
                    │   │  │ Rust API  │    │ KV Store   │     │   │
                    │   │  │ Handlers  │────│ JSON/FS    │     │   │
                    │   │  └─────┬─────┘    └─────┬─────┘     │   │
                    │   │        │                 │           │   │
                    │   │  ┌─────┴─────────────────┴───────┐   │   │
                    │   │  │     Rust std:: APIs            │   │   │
                    │   │  │  (TcpListener, fs, thread)     │   │   │
                    │   │  └─────┬─────────────────┬───────┘   │   │
                    │   │        │                 │           │   │
                    │   │  ┌─────┴─────┐    ┌─────┴─────┐     │   │
                    │   │  │  smoltcp  │    │ VirtioFS   │     │   │
                    │   │  │  TCP/IP   │    │  (FUSE)    │     │   │
                    │   │  └─────┬─────┘    └─────┬─────┘     │   │
                    │   │        │                 │           │   │
                    │   │  ┌─────┴─────┐    ┌─────┴─────┐     │   │
                    │   │  │virtio-net │    │ virtio-fs  │     │   │
                    │   │  │  driver   │    │  driver    │     │   │
                    │   │  └─────┬─────┘    └─────┬─────┘     │   │
                    │   │        │                 │           │   │
                    │   │  ┌─────┴─────────────────┴───────┐   │   │
                    │   │  │      HermitOS Kernel           │   │   │
                    │   │  │   (CPU, Memory, PCI, DHCP)     │   │   │
                    │   │  └───────────────────────────────┘   │   │
                    │   └─────────────────────────────────────┘   │
                    │              │                │              │
                    │         ─────┴────      ──────┴───          │
                    │        │ vNIC     │    │ vDisk    │         │
                    │         ──────────      ──────────          │
                    └─────────────────────────────────────────────┘
```

### What Happens at Boot

The HermitOS kernel does everything **before `main()` is called**:

| Step | What Happens | Your Code? |
|------|-------------|------------|
| 1 | CPU mode setup, GDT, IDT, page tables | No |
| 2 | Global heap allocator initialized | No |
| 3 | PCI bus enumerated, VirtIO devices probed | No |
| 4 | `virtio-net` driver attached | No |
| 5 | DHCPv4 lease acquired (public IP from hypervisor) | No |
| 6 | VirtioFS mounted (if configured) | No |
| 7 | COM1 serial initialized for `println!` | No |
| 8 | **`main()` runs** | **Yes** |
| 9 | Storage layer initialized (VirtioFS or in-memory fallback) | Auto |
| 10 | Config engine loads `backend/endpoints.json` | Auto |
| 11 | Rust route table built | Auto |
| 12 | HTTP server starts, 4 worker threads spawned | Auto |

---

## Project Structure

```
rust-web-appliance/
│
├── frontend/                      FRONTEND DROP ZONE
│   ├── index.html                 Main page (interactive dashboard)
│   ├── css/style.css              Stylesheet (dark theme, responsive)
│   ├── js/app.js                  JS controller (health, KV store UI)
│   ├── img/                       Drop images here
│   ├── fonts/                     Drop web fonts here
│   └── pages/                     Drop additional HTML pages here
│
├── backend/                       CONFIG-DRIVEN API
│   └── endpoints.json             Define collections here -> auto CRUD
│
├── src/                           RUST BACKEND (compiled)
│   ├── main.rs                    Entry point, boot banner, serial lifeline
│   ├── server.rs                  tiny_http with 4 worker threads
│   ├── router.rs                  Three-tier dispatch (Rust -> Config -> Static)
│   ├── storage.rs                 Dual-backend KV (VirtioFS or in-memory)
│   ├── static_files.rs            Static file serving + embedded fallback
│   └── api/
│       ├── mod.rs                 Route table (register Rust endpoints here)
│       ├── context.rs             Context type (request/response helpers)
│       ├── config_engine.rs       Config-driven CRUD engine
│       ├── system.rs              Built-in: /api/health, /api/info
│       ├── kv.rs                  Built-in: /api/kv CRUD
│       └── example.rs             Example handlers (copy as template)
│
├── scripts/
│   ├── build.sh                   Compile for HermitOS (debug/release)
│   ├── run-qemu.sh                Launch in QEMU (3 networking modes)
│   └── make-image.sh              Create bootable .img (GRUB + hermit-loader)
│
├── .cargo/config.toml             x86_64-unknown-hermit target + build-std
├── Cargo.toml                     Hermit kernel features + app dependencies
└── rust-toolchain.toml            Pinned to nightly (required for build-std)
```

---

## Quick Start

### Prerequisites

| Tool | Purpose |
|------|---------|
| `rustup` | Rust toolchain manager |
| Rust **nightly** | Required for `-Z build-std` |
| `rust-src` component | Standard library source for cross-compilation |
| `qemu-system-x86_64` | Virtual machine for testing |
| `hermit-loader` | Multiboot-compatible bootloader for HermitOS |

### 1. Install Dependencies

```bash
# Rust nightly + source (handled automatically by rust-toolchain.toml)
rustup component add rust-src --toolchain nightly

# QEMU
sudo apt install qemu-system-x86_64    # Debian/Ubuntu
sudo dnf install qemu-system-x86       # Fedora
brew install qemu                      # macOS

# hermit-loader
curl -L -o hermit-loader-x86_64 \
  https://github.com/hermit-os/loader/releases/latest/download/hermit-loader-x86_64
chmod +x hermit-loader-x86_64
```

### 2. Build

```bash
# Debug build (faster compile, ~opt-level 1)
./scripts/build.sh

# Release build (opt-level 3, stripped)
./scripts/build.sh release
```

Output: `target/x86_64-unknown-hermit/release/rust-web-appliance` (1.5 MB)

### 3. Run in QEMU

```bash
# Basic mode -- user networking, no root needed
# HTTP available at http://localhost:8080
./scripts/run-qemu.sh

# TAP networking -- guest gets real IP on 10.0.5.x subnet
./scripts/run-qemu.sh --tap

# Full stack -- TAP + VirtioFS for persistent storage
./scripts/run-qemu.sh --virtiofs
```

### 4. Test It

```bash
# Dashboard
curl http://localhost:8080/

# System endpoints
curl http://localhost:8080/api/health
curl http://localhost:8080/api/info

# Raw KV store
curl -X PUT http://localhost:8080/api/kv/greeting -d '"Hello, Unikernel!"'
curl http://localhost:8080/api/kv/greeting
curl http://localhost:8080/api/kv

# Config-driven collections (auto-generated from endpoints.json)
curl -X POST http://localhost:8080/api/products \
  -d '{"name":"Widget","price":9.99,"in_stock":true}'
curl http://localhost:8080/api/products
curl -X POST http://localhost:8080/api/blog_posts \
  -d '{"title":"First Post","body":"Hello from the unikernel!","author":"admin"}'
curl http://localhost:8080/api/blog_posts

# Example Rust endpoints
curl http://localhost:8080/api/echo
curl http://localhost:8080/api/greet/World
```

---

## The Node.js Comparison

```
Node.js workflow:          Edit route.js  ->  npm start (3s)  ->  running
Rust appliance workflow:   Edit handler.rs -> cargo build (18s) -> running
```

Both require a restart when backend code changes. The difference is what you get for that restart:

| | Node.js on Linux | This Appliance |
|---|---|---|
| Runtime layers | JS -> V8 -> Node -> Linux -> KVM | Machine code -> KVM |
| Memory footprint | ~80-300MB | ~8-16MB |
| Image size | runtime + node_modules + OS | 1.5MB total |
| Cold boot to serving | 2-10 seconds | ~200ms |
| Attack surface | npm supply chain, V8, Node, Linux, glibc, OpenSSL | Your Rust code |
| Runtime dependencies | thousands | zero |
| `node_modules/` | the abyss | doesn't exist |

The config-driven engine closes the remaining gap -- for pure CRUD, you don't even edit Rust, just JSON.

---

## Deploying a Bootable Image

### Create the Image

```bash
./scripts/build.sh release
sudo ./scripts/make-image.sh
```

This produces `appliance.img` containing:
- MBR partition table
- GRUB bootloader (i386-pc target)
- `hermit-loader` as the Multiboot kernel
- `rust-web-appliance` as the Multiboot module
- `frontend/*` copied to `/www`
- `backend/endpoints.json` copied to `/backend`

### Test the Image Locally

```bash
qemu-system-x86_64 -enable-kvm -m 256M \
  -serial stdio -display none \
  -hda appliance.img \
  -netdev user,id=u1,hostfwd=tcp::8080-:8080 \
  -device virtio-net-pci,netdev=u1
```

### Deploy to DigitalOcean

```bash
doctl compute image create rust-web-appliance \
  --image-url <your-upload-url> \
  --region nyc1

doctl compute droplet create my-appliance \
  --image <image-id> \
  --size s-1vcpu-1gb \
  --region nyc1
```

> **Note:** DigitalOcean Custom Images expect cloud-init. A bare unikernel image may require
> experimental validation. Alternative targets: self-managed KVM hosts, Hetzner Cloud,
> or any bare-metal provider that allows raw image uploads.

---

## QEMU Networking Modes

### Basic (Default)

```
 Host                          Guest
 ────────                      ──────
 localhost:8080  ─── fwd ───>  :8080
 (QEMU user-net)
```
No root required. Best for development.

### TAP

```
 Host                          Guest
 ────────                      ──────
 tap10 (10.0.5.1)  ────────>  10.0.5.x
```

Setup:
```bash
sudo ip tuntap add tap10 mode tap user $USER
sudo ip addr add 10.0.5.1/24 dev tap10
sudo ip link set dev tap10 up
```

### VirtioFS (Full Stack)

```
 Host                          Guest
 ────────                      ──────
 tap10 + VirtioFS              10.0.5.x
 /tmp/guestfs/                 /www      (frontend files)
   ├── www/                    /data     (KV storage)
   ├── data/                   /backend  (endpoints.json)
   └── backend/
```

Setup:
```bash
mkdir -p /tmp/guestfs/www /tmp/guestfs/data /tmp/guestfs/backend
cp -r frontend/* /tmp/guestfs/www/
cp backend/endpoints.json /tmp/guestfs/backend/
virtiofsd --socket-path=/tmp/vhostqemu --shared-dir=/tmp/guestfs &
```

---

## Built-in API Endpoints

These are always available regardless of configuration:

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/api/health` | Health check (`{"status":"ok"}`) |
| `GET` | `/api/info` | Appliance name, version, OS |
| `GET` | `/api/kv` | List all raw KV keys |
| `GET` | `/api/kv/:key` | Read a raw KV value |
| `PUT` | `/api/kv/:key` | Write a raw KV value |
| `DELETE` | `/api/kv/:key` | Delete a raw KV key |
| `GET` | `/api/echo` | Example: static JSON response |
| `POST` | `/api/echo` | Example: echo request body |
| `GET` | `/api/greet/:name` | Example: parameterized response |

---

## Dual-Mode Storage

| Mode | When | Persistence |
|------|------|-------------|
| **VirtioFS** | QEMU launched with `--virtiofs` | Survives reboots (host filesystem) |
| **In-Memory** | Basic QEMU / no VirtioFS | Lost on shutdown |

Auto-detected at boot. Both the raw KV API and the config-driven collections use the same storage layer.

---

## Serial Lifeline (COM1)

Every `println!` goes to COM1 serial. QEMU's `-serial stdio` pipes it to your terminal. Kernel panics produce full backtraces here.

```
════════════════════════════════════════════════════════
  Rust Web Appliance v0.1.0
  Build: rust-web-appliance (release)
════════════════════════════════════════════════════════

[boot] Target OS: hermit (HermitOS unikernel)
[boot] Architecture: x86_64
[boot] The kernel has already:
[boot]   - Initialized CPU, memory, allocator
[boot]   - Enumerated PCI bus, probed virtio drivers
[boot]   - Acquired IP via DHCPv4 (virtio-net)
[boot]   - Mounted VirtioFS (if configured)

[init] Initializing storage layer...
[init] Storage ready
[init] Starting HTTP server on 0.0.0.0:8080

[http] Listening on http://0.0.0.0:8080
[http] 9 compiled Rust routes registered
[http] Loading config-driven API engine...
[config-api]   /products (4 fields, 2 required)
[config-api]   /blog_posts (4 fields, 2 required)
[config-api] 2 collections loaded, 5 endpoints each
[http] Config engine active
[http] Spawning 4 worker threads
[http] Worker 0 started
[http] Worker 1 started
[http] Worker 2 started
[http] Worker 3 started
```

For kernel log verbosity:
```bash
HERMIT_LOG_LEVEL_FILTER=debug cargo build --target x86_64-unknown-hermit
```

---

## Technology Stack

| Layer | Technology | Role |
|-------|-----------|------|
| **Kernel** | [HermitOS](https://github.com/hermit-os/kernel) v0.13.0 | CPU init, memory, PCI, scheduling |
| **Network Driver** | `virtio-net` (kernel built-in) | NIC interface to hypervisor |
| **TCP/IP** | [smoltcp](https://github.com/smoltcp-rs/smoltcp) (kernel built-in) | Full TCP/IP stack in Rust |
| **DHCP** | smoltcp DHCPv4 (kernel built-in) | IP acquisition on boot |
| **HTTP Server** | [tiny_http](https://github.com/tiny-http/tiny-http) 0.12 | Synchronous HTTP/1.1 |
| **API Engine** | Config-driven (custom) | Runtime CRUD from JSON schema |
| **Serialization** | [serde](https://serde.rs/) + serde_json | JSON storage + API validation |
| **Filesystem** | VirtioFS (kernel built-in) | Host-shared persistent storage |
| **Bootloader** | [hermit-loader](https://github.com/hermit-os/loader) | Multiboot ELF loader |
| **Build** | `build-std` (Rust nightly) | Cross-compile std for hermit |

Every single component is written in Rust. There is **zero C code** in the runtime.

---

## Key Design Decisions

### Why `tiny_http` instead of Axum/Hyper?
Axum and Hyper depend on Tokio, which depends on `mio` for async I/O. **`mio` explicitly excludes `target_os = "hermit"`** from its epoll/poll support. No mio = no Tokio = no Axum. `tiny_http` uses synchronous `std::net::TcpListener`, which HermitOS fully supports.

### Why JSON flat files instead of an embedded database?
`redb` (the intended embedded DB) requires `mmap` syscall support, which HermitOS does not provide. `virtio-blk` (block device driver) is also not implemented in the HermitOS kernel. VirtioFS provides real file I/O through the host, making JSON flat files the pragmatic choice.

### Why no SSH server?
`russh` has a hard, non-optional dependency on Tokio. Same Tokio/mio incompatibility. Remote access is handled at the hypervisor level: serial console or SSH into the KVM host machine.

### Why `nightly` Rust?
The `-Z build-std` flag (needed to cross-compile the Rust standard library for `x86_64-unknown-hermit`) is an unstable feature. This is a hard requirement for any HermitOS application.

### Why two backend tiers?
A unikernel compiles everything into one binary -- there's no runtime code loading like PHP/Node. The config-driven engine gives you the "drop a file, get an API" experience for the 80% of work that's just CRUD. The Rust handler tier gives you escape velocity for the 20% that needs real logic. Both coexist, and Rust routes always take priority.

---

## Roadmap

- [ ] TLS termination (rustls -- no OpenSSL dependency)
- [ ] WebSocket support for real-time dashboard
- [ ] Prometheus-compatible `/metrics` endpoint
- [ ] Multi-core support (`-smp N`)
- [ ] ARM64 / `aarch64-unknown-hermit` target
- [ ] Automated CI pipeline (build + QEMU smoke test)
- [ ] Cloud-init shim for DigitalOcean compatibility
- [ ] Config engine: filtering/search query parameters
- [ ] Config engine: pagination for large collections
- [ ] Config engine: relationship fields between collections

---

## License

This project is provided as-is for educational and experimental purposes.

---

<p align="center">
  <sub>Built with nothing but Rust. No C. No Linux. No compromises.</sub>
</p>
