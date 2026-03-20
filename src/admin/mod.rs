// ═══════════════════════════════════════════════════════════════════
// Admin Console — web-based management for the unikernel
// ═══════════════════════════════════════════════════════════════════
//
// All /admin/* requests are dispatched here, bypassing the normal
// API key authentication. The admin console uses Ed25519 key-pair
// authentication instead.
//
// Route table:
//   GET  /admin/api/auth/key        → serve embedded private key (public)
//   POST /admin/api/auth/challenge  → generate nonce (public)
//   POST /admin/api/auth/verify     → verify signature (public)
//   POST /admin/api/auth/logout     → revoke session
//   GET  /admin/api/dashboard       → system stats
//   GET  /admin/api/logs            → log buffer
//   DELETE /admin/api/logs          → clear log buffer
//   GET  /admin/api/tls             → cert info
//   POST /admin/api/tls/upload      → upload new certs + hot-reload
//   GET  /admin/api/kv              → list KV keys
//   GET  /admin/api/kv/:key         → read value
//   PUT  /admin/api/kv/:key         → write value
//   DELETE /admin/api/kv/:key       → delete key
//   GET  /admin/api/endpoints       → current endpoints.json
//   PUT  /admin/api/endpoints       → update endpoints.json
//   GET  /admin/api/security        → API key config
//   PUT  /admin/api/security        → update API key config
//   GET  /admin/api/ports           → port configuration
//   PUT  /admin/api/ports           → update port config
//   GET  /admin/api/smtp            → SMTP configuration
//   PUT  /admin/api/smtp            → update SMTP config
//   POST /admin/api/smtp/test       → send test email
//   GET  /admin/*                   → serve embedded admin UI
// ═══════════════════════════════════════════════════════════════════

pub mod auth;
pub mod dashboard;
pub mod endpoints;
pub mod kv_browser;
pub mod logs;
pub mod ports_manager;
pub mod security_manager;
pub mod session;
pub mod smtp_manager;
pub mod tls_manager;
pub mod ui;

use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::http::{self, HttpRequest};
use crate::storage::Storage;

use auth::AdminAuth;
use logs::LogBuffer;
use session::SessionStore;

pub struct AdminState {
    pub auth: AdminAuth,
    pub sessions: SessionStore,
    pub log_buffer: LogBuffer,
    pub tls_config: Arc<RwLock<Arc<rustls::ServerConfig>>>,
    pub boot_time: u64,
    pub request_count: AtomicU64,
}

impl AdminState {
    pub fn increment_requests(&self) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn total_requests(&self) -> u64 {
        self.request_count.load(Ordering::Relaxed)
    }

    pub fn log(&self, msg: &str) {
        self.log_buffer.capture(msg);
    }
}

/// Macro for logging through AdminState (writes to serial + ring buffer).
#[macro_export]
macro_rules! admin_log {
    ($state:expr, $($arg:tt)*) => {
        $state.log(&format!($($arg)*))
    };
}

/// Dispatch an /admin/* request. Called from router before API key auth.
pub fn handle(
    request: HttpRequest,
    mut writer: Box<dyn Write + Send>,
    admin_state: &'static AdminState,
    storage: &'static Storage,
) {
    let method = request.method.clone();
    let url = request.url.clone();

    // Strip query string for routing
    let path = url.split('?').next().unwrap_or(&url);

    // ── Public auth endpoints (no session required) ──────────────
    match (method.as_str(), path) {
        ("GET", "/admin/api/auth/key") => {
            return auth_key(writer, admin_state);
        }
        ("POST", "/admin/api/auth/challenge") => {
            return auth_challenge(request, writer, admin_state);
        }
        ("POST", "/admin/api/auth/verify") => {
            return auth_verify(request, writer, admin_state);
        }
        _ => {}
    }

    // ── All other /admin/api/* require a valid session ───────────
    if path.starts_with("/admin/api/") {
        let token = extract_admin_token(&request);
        match token {
            Some(t) if admin_state.sessions.validate(&t) => {
                // Session valid — dispatch to handler
            }
            _ => {
                let _ = http::write_response(
                    &mut writer,
                    401,
                    "application/json",
                    br#"{"error":"admin session required"}"#,
                );
                return;
            }
        }

        // ── Authenticated admin API routes ───────────────────────
        match (method.as_str(), path) {
            ("POST", "/admin/api/auth/logout") => {
                if let Some(t) = extract_admin_token(&request) {
                    admin_state.sessions.revoke(&t);
                }
                let _ = http::write_response(
                    &mut writer,
                    200,
                    "application/json",
                    br#"{"status":"logged out"}"#,
                );
            }

            ("GET", "/admin/api/dashboard") => {
                dashboard::handle(writer, admin_state, storage);
            }

            ("GET", "/admin/api/logs") => {
                let lines = parse_query_param(&url, "lines")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(200);
                logs::handle_get(writer, admin_state, lines);
            }
            ("DELETE", "/admin/api/logs") => {
                logs::handle_clear(writer, admin_state);
            }

            ("GET", "/admin/api/tls") => {
                tls_manager::handle_get(writer, admin_state);
            }
            ("POST", "/admin/api/tls/upload") => {
                tls_manager::handle_upload(request, writer, admin_state);
            }

            ("GET", "/admin/api/endpoints") => {
                endpoints::handle_get(writer);
            }
            ("PUT", "/admin/api/endpoints") => {
                endpoints::handle_put(request, writer);
            }

            ("GET", "/admin/api/security") => {
                security_manager::handle_get(writer, storage);
            }
            ("PUT", "/admin/api/security") => {
                security_manager::handle_put(request, writer, storage);
            }

            ("GET", "/admin/api/ports") => {
                ports_manager::handle_get(writer, storage);
            }
            ("PUT", "/admin/api/ports") => {
                ports_manager::handle_put(request, writer, storage);
            }

            ("GET", "/admin/api/smtp") => {
                smtp_manager::handle_get(writer, storage);
            }
            ("PUT", "/admin/api/smtp") => {
                smtp_manager::handle_put(request, writer, storage);
            }
            ("POST", "/admin/api/smtp/test") => {
                smtp_manager::handle_test(request, writer, storage);
            }

            // KV browser — prefix match for /admin/api/kv/*
            ("GET", p) if p == "/admin/api/kv" => {
                kv_browser::handle_list(writer, storage);
            }
            ("GET", p) if p.starts_with("/admin/api/kv/") => {
                let key = &p["/admin/api/kv/".len()..];
                kv_browser::handle_get(writer, storage, key);
            }
            ("PUT", p) if p.starts_with("/admin/api/kv/") => {
                let key = &p["/admin/api/kv/".len()..];
                kv_browser::handle_put(request, writer, storage, key);
            }
            ("DELETE", p) if p.starts_with("/admin/api/kv/") => {
                let key = &p["/admin/api/kv/".len()..];
                kv_browser::handle_delete(writer, storage, key);
            }

            _ => {
                let _ = http::write_response(
                    &mut writer,
                    404,
                    "application/json",
                    br#"{"error":"admin endpoint not found"}"#,
                );
            }
        }
        return;
    }

    // ── Admin UI (HTML) ──────────────────────────────────────────
    if method.as_str() == "GET" {
        ui::serve(path, writer);
    } else {
        let _ = http::write_response(
            &mut writer,
            405,
            "application/json",
            br#"{"error":"method not allowed"}"#,
        );
    }
}

// ── Auth handlers ────────────────────────────────────────────────

fn auth_key(
    mut writer: Box<dyn Write + Send>,
    admin_state: &'static AdminState,
) {
    let pem = admin_state.auth.private_key_pem();
    let body = format!(r#"{{"key":"{}"}}"#, pem.replace('\n', "\\n"));
    let _ = http::write_response(&mut writer, 200, "application/json", body.as_bytes());
}

fn auth_challenge(
    _request: HttpRequest,
    mut writer: Box<dyn Write + Send>,
    admin_state: &'static AdminState,
) {
    let challenge = admin_state.auth.create_challenge();
    let body = format!(r#"{{"challenge":"{}"}}"#, challenge);
    let _ = http::write_response(&mut writer, 200, "application/json", body.as_bytes());
}

fn auth_verify(
    request: HttpRequest,
    mut writer: Box<dyn Write + Send>,
    admin_state: &'static AdminState,
) {
    // Parse JSON body: {"challenge": "...", "signature": "..."}
    let body_str = String::from_utf8_lossy(&request.body).to_string();
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&body_str);

    let (challenge, signature) = match parsed {
        Ok(v) => {
            let c = v.get("challenge").and_then(|v| v.as_str()).unwrap_or("");
            let s = v.get("signature").and_then(|v| v.as_str()).unwrap_or("");
            (c.to_string(), s.to_string())
        }
        Err(_) => {
            let _ = http::write_response(
                &mut writer,
                400,
                "application/json",
                br#"{"error":"invalid JSON body"}"#,
            );
            return;
        }
    };

    if challenge.is_empty() || signature.is_empty() {
        let _ = http::write_response(
            &mut writer,
            400,
            "application/json",
            br#"{"error":"challenge and signature required"}"#,
        );
        return;
    }

    match admin_state.auth.verify(&challenge, &signature) {
        Ok(()) => {
            let token = admin_state.sessions.create();
            admin_state.log("[admin] Authentication successful — session created");
            let body = format!(r#"{{"token":"{}","expires_in":3600}}"#, token);
            let _ = http::write_response(&mut writer, 200, "application/json", body.as_bytes());
        }
        Err(e) => {
            admin_state.log(&format!("[admin] Authentication failed: {}", e));
            let body = format!(r#"{{"error":"{}"}}"#, e);
            let _ = http::write_response(&mut writer, 401, "application/json", body.as_bytes());
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────

fn extract_admin_token(request: &HttpRequest) -> Option<String> {
    for (name, value) in &request.headers {
        if name.eq_ignore_ascii_case("authorization") {
            let trimmed = value.trim();
            if let Some(token) = trimmed.strip_prefix("AdminToken ") {
                return Some(token.trim().to_string());
            }
            if let Some(token) = trimmed.strip_prefix("admintoken ") {
                return Some(token.trim().to_string());
            }
        }
    }
    None
}

fn parse_query_param<'a>(url: &'a str, key: &str) -> Option<&'a str> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if kv.next()? == key {
            return kv.next();
        }
    }
    None
}
