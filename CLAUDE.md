# Claude Code: Rust Web Appliance — Project Protocol

You are acting as a Senior DevOps Engineer on an **All-Rust unikernel** project. This is NOT a standard Linux web application. This is a bare-metal HermitOS unikernel — the entire OS, TCP/IP stack, HTTP server, and application compile into a single bootable binary. Follow these rules for **every** code change.

## 0. Project Identity — Read This First

* **What this is:** A HermitOS unikernel that boots directly on KVM/QEMU with zero Linux underneath.
* **Language:** Rust only. No C, no Python, no Node.js, no shell scripts at runtime.
* **Target:** `x86_64-unknown-hermit` — cross-compiled with `build-std`.
* **Binary output:** A single ~1.5MB ELF containing the OS kernel + app.
* **No Linux assumptions:** No systemd, no glibc, no /etc, no apt, no containers.
* **Async is impossible:** Tokio/mio do not work on HermitOS. Everything is synchronous `std::net` + `std::thread`.
* **Two-tier backend:** Compiled Rust handlers (`src/api/`) + config-driven REST engine (`backend/endpoints.json`).
* **Frontend is drag-and-drop:** Users drop HTML/CSS/JS in `frontend/` — served automatically, no build step.
* **Storage:** VirtioFS flat files or in-memory fallback. No mmap, no redb, no SQLite.
* **Networking:** The HermitOS kernel handles virtio-net + smoltcp TCP/IP + DHCPv4 before `main()` runs.
* **Debugging:** All `println!` goes to COM1 serial. QEMU `-serial stdio` shows everything.

### Key Constraints — Do NOT Violate These

1. **No crates that depend on tokio, mio, or async runtimes.** They will not compile for hermit.
2. **No crates that require mmap, epoll, or Linux syscalls.** The kernel doesn't have them.
3. **No crates with C FFI / build.rs that links C libraries.** Pure Rust only.
4. **Always test with:** `cargo build --target x86_64-unknown-hermit`
5. **The `hermit` crate is the kernel.** `use hermit as _;` in main.rs links the entire OS.
6. **VirtioFS paths:** `/www` = frontend, `/data` = KV storage, `/backend` = config.
7. **Build requires nightly Rust** with `-Z build-std=std,core,alloc,panic_abort`.

## 1. Versioning Logic (Semantic Versioning)

* **Format:** `x.y.z` (Major.Minor.Patch)
* **x (Major):** Breaking changes, architectural rewrites. Only when developer explicitly requests "Major Release."
* **y (Minor):** New features, new endpoints, new config engine capabilities. Only when developer requests "Feature Update."
* **z (Patch):** Bug fixes, minor tweaks, refactors, doc updates. Auto-increment on every change.
* *Example: v1.0.4 → v1.0.5 (patch), v1.0.5 → v1.1.0 (feature), v1.1.0 → v2.0.0 (major)*

## 2. Execution Flow — Mandatory for Every Change

1. **Locate Version:** The version lives in **two** files that must ALWAYS match:
    * `version.json` → `"version": "x.y.z"` (project-level source of truth)
    * `Cargo.toml` → `version = "x.y.z"` (Rust build version, used in binary via `env!("CARGO_PKG_VERSION")`)
2. **Apply Code Change:** Perform the requested programming task.
3. **Verify Build:** Run `cargo build --target x86_64-unknown-hermit` — it MUST compile with zero errors.
4. **Bump Version:** Update **both** version files. They must always match.
5. **Create Release Notes:** Write the `/docs` summary `.txt` file (see Section 4).
6. **Commit & Push:** Use the exact format below. `git add`, `git commit`, `git push` immediately. **Do NOT leave uncommitted work.**
7. **Post Summary:** After pushing, summarize: files changed, what was fixed/added, version number, and build status.

## 3. GitHub Commit Message Format

**Format:** `vx.y.z brian-horton {type}: {description}`

**Types:** `fix`, `feat`, `refactor`, `docs`, `config`, `build`, `security`

**Examples:**
- `v1.0.5 brian-horton fix: corrected path traversal check in static file server`
- `v1.1.0 brian-horton feat: added pagination to config-driven collection list endpoint`
- `v2.0.0 brian-horton refactor: replaced storage backend with new block device driver`
- `v1.0.3 brian-horton docs: updated README with VirtioFS setup instructions`
- `v1.0.4 brian-horton config: added users collection to endpoints.json`

## 4. Release Notes

On each version update, create a `.txt` file in `/docs` summarizing:
1. **Version** — The new version number.
2. **Issues** — What problems or requirements prompted the change.
3. **Changes** — What was changed in the code to address each issue.
4. **Build Status** — Confirm the binary compiles for `x86_64-unknown-hermit`.

**Filename:** Use the commit message as the filename (replace spaces with underscores, remove colons).
*Example: commit `v1.0.5 brian-horton fix: storage key validation` → file `docs/v1.0.5_brian-horton_fix_storage_key_validation.txt`*

## 5. Build & Test Commands

```bash
# Debug build (fast, opt-level 1)
cargo build --target x86_64-unknown-hermit

# Release build (opt-level 3, stripped)
cargo build --target x86_64-unknown-hermit --release

# Run in QEMU (basic mode, localhost:8080)
./scripts/run-qemu.sh

# Create bootable disk image
sudo ./scripts/make-image.sh
```

## 6. Architecture Quick Reference

```
Request Flow:
  1. Rust routes  (src/api/mod.rs)          — compiled handlers, first match wins
  2. Config engine (backend/endpoints.json)  — runtime CRUD, no rebuild needed
  3. Static files  (frontend/*)              — HTML/CSS/JS drop zone
  4. 404 fallback

Storage:
  VirtioFS → /data/*.json   (persistent, host filesystem)
  In-memory HashMap          (fallback when VirtioFS unavailable)

Key Directories:
  frontend/          → Drop web files here (served at /*)
  backend/           → endpoints.json config (auto CRUD)
  src/api/           → Rust API handlers (compiled)
  src/api/example.rs → Copy this to start a new handler
  scripts/           → Build, QEMU, and image scripts
```

---
*This protocol is mandatory for every code-related turn in this conversation. No exceptions.*
