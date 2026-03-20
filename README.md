# Rust Web Appliance

An **All-Rust unikernel** web server appliance built on [HermitOS](https://github.com/hermit-os). The entire OS, TCP/IP stack, HTTPS server, and application compile into a single bootable binary (~1.5MB). No Linux, no containers, no glibc — just bare-metal KVM.

## Quick Start

```bash
# Build (requires nightly Rust)
cargo build --target x86_64-unknown-hermit

# Run in QEMU
./scripts/run-qemu.sh

# Access:
#   https://localhost:9443       (primary HTTPS — port 443)
#   http://localhost:8080        (HTTP redirect — port 80)
#   https://localhost:18443      (API/mobile — port 8443, off by default)
```

## Ports

| Port | Protocol | Purpose | Default |
|------|----------|---------|---------|
| 80   | HTTP     | Redirect to HTTPS (301) | Redirect (on) |
| 443  | HTTPS    | Primary web traffic | Always on |
| 8443 | HTTPS    | API / mobile app traffic | Off |

Port 8443 is a Cloudflare-compatible proxied HTTPS port for public API access, mobile apps, and third-party integrations. Enable it in the admin panel under **VHost** or in `/data/ports.json`.

Port configuration is stored in `/data/ports.json` on VirtioFS:

```json
{
  "vhost": "",
  "http":  { "port": 80,   "mode": "redirect" },
  "https": { "port": 443,  "enabled": true },
  "api":   { "port": 8443, "enabled": false }
}
```

**QEMU port forwarding (basic mode):**

| Host | Guest | Service |
|------|-------|---------|
| `localhost:8080` | `:80` | HTTP redirect |
| `localhost:9443` | `:443` | Primary HTTPS |
| `localhost:18443` | `:8443` | API / mobile HTTPS |

## Architecture

```
Request Flow:
  1. Compiled Rust handlers   (src/api/)              — first match wins
  2. Config-driven REST engine (backend/endpoints.json) — runtime CRUD, no rebuild
  3. Static files              (frontend/*)             — HTML/CSS/JS drop zone
  4. 404 fallback

Stack:
  ┌──────────────────────────────────┐
  │  Your App (src/api/, frontend/)  │
  │  HTTPS Server (rustls, httparse) │
  │  HermitOS Kernel (smoltcp TCP/IP)│
  │  KVM / QEMU                      │
  └──────────────────────────────────┘
```

## Project Layout

```
frontend/          → Drop HTML/CSS/JS here (served at /*)
backend/           → endpoints.json (config-driven CRUD API)
src/               → Rust application source
  main.rs          → Entry point
  server.rs        → HTTPS listener + request dispatch
  tls.rs           → TLS configuration (rustls + pure-Rust crypto)
  http.rs          → HTTP/1.1 request/response types
  security.rs      → API key authentication + RBAC
  api/             → Compiled API handlers
  api/example.rs   → Copy this to add a new handler
data/tls/          → Place TLS certificates here (gitignored .pem files)
certs/             → Embedded dev certificate (compiled into binary)
scripts/           → Build, run, and image creation scripts
docs/              → Release notes
```

## TLS / HTTPS

The unikernel terminates TLS natively using **rustls** with the **rustls-rustcrypto** pure-Rust crypto provider. No OpenSSL, no C FFI.

### Certificate Loading Priority

1. **VirtioFS:** `/data/tls/cert.pem` + `/data/tls/key.pem` (checked first)
2. **Embedded:** Compiled-in self-signed dev certificate (fallback)

### Cloudflare SSL Setup (Production)

When deploying behind Cloudflare, use a **Cloudflare Origin Certificate** for end-to-end encryption:

1. **Generate the Origin Certificate:**
   - Cloudflare Dashboard → your domain → **SSL/TLS** → **Origin Server**
   - Click **Create Certificate**
   - Key type: ECDSA (recommended) or RSA
   - Hostnames: your domain (e.g., `*.example.com`, `example.com`)
   - Validity: 15 years (default)

2. **Save the certificate and key:**
   ```
   data/tls/cert.pem   ← Paste the Origin Certificate here
   data/tls/key.pem    ← Paste the Private Key here
   ```

3. **Deploy to the server:**
   - Copy `cert.pem` and `key.pem` to the VirtioFS shared directory that maps to `/data/tls/` on the guest
   - For example: `cp data/tls/*.pem /path/to/shared/data/tls/`

4. **Set Cloudflare SSL mode to "Full (Strict)":**
   - Cloudflare Dashboard → SSL/TLS → Overview → **Full (Strict)**
   - This ensures encrypted traffic end-to-end (client → Cloudflare → unikernel)

5. **Boot the unikernel** — it picks up the certs automatically, no rebuild needed.

```
Traffic Flow:
  Client ──HTTPS──→ Cloudflare ──HTTPS──→ Unikernel :443  (primary traffic)
  Client ──HTTP───→ Cloudflare ──HTTP───→ Unikernel :80   (301 redirect)
  Mobile ──HTTPS──→ Cloudflare ──HTTPS──→ Unikernel :8443 (API / data access)
                     (edge TLS)           (origin TLS with Cloudflare cert)
```

### Self-Signed Certificates (Development)

```bash
# Generate a self-signed cert into data/tls/
./scripts/gen-cert.sh data/tls

# Or generate into the default certs/ directory
./scripts/gen-cert.sh
```

### Certificate Security

- **Never commit** real certificates or private keys to git
- The `data/tls/*.pem` files are gitignored — only the README is tracked
- Keep private keys with restricted permissions: `chmod 600 data/tls/key.pem`

## API Authentication

All endpoints require an API key (except `GET /api/health`). Pass it via:

```bash
# Header
curl -k https://localhost:9443/api/info -H "X-API-Key: YOUR_KEY"

# Bearer token
curl -k https://localhost:9443/api/info -H "Authorization: Bearer YOUR_KEY"
```

A default dev key is printed to serial on first boot.

## Build & Run

```bash
# Debug build
cargo build --target x86_64-unknown-hermit

# Release build
cargo build --target x86_64-unknown-hermit --release

# Run in QEMU (user-mode networking, no root)
./scripts/run-qemu.sh

# Run with TAP networking
./scripts/run-qemu.sh --tap

# Run with TAP + VirtioFS (full stack)
./scripts/run-qemu.sh --virtiofs

# Create bootable disk image
sudo ./scripts/make-image.sh
```

## Requirements

- **Rust nightly** (see `rust-toolchain.toml`)
- **QEMU** with KVM support (for local dev)
- **hermit-loader** binary (see `scripts/run-qemu.sh` for download instructions)
