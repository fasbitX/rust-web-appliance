// ═══════════════════════════════════════════════════════════════════
// Router — vhost + admin console + authentication + three-tier dispatch
// ═══════════════════════════════════════════════════════════════════
//
// Requests are processed in this order:
//
//   0. Admin console (/admin/*)      — own auth, bypasses vhost + API keys
//   1. VHost enforcement             — reject if Host header doesn't match
//   2. Authentication (API key check)
//   3. Rust routes (src/api/*.rs)     — compiled handlers, full power
//   4. Config engine (endpoints.json) — runtime CRUD, no rebuild
//   5. Static files (frontend/*)      — HTML/CSS/JS/images
//   6. 404 fallback
//
// ═══════════════════════════════════════════════════════════════════

use std::io::Write;

use crate::admin::{self, AdminState};
use crate::api::{ConfigEngine, Context, Route};
use crate::http::{self, HttpRequest};
use crate::security::{AuthResult, SecurityConfig};
use crate::static_files;
use crate::storage::Storage;

/// Dispatch a request through the vhost + admin + security + three-tier pipeline.
pub fn handle_request(
    request: HttpRequest,
    mut writer: Box<dyn Write + Send>,
    routes: &[Route],
    config_engine: &Option<ConfigEngine>,
    storage: &'static Storage,
    security: &SecurityConfig,
    admin_state: &'static AdminState,
    vhost: &str,
) {
    let url = request.url.clone();
    let method = request.method.clone();

    // ── Tier 0: Admin console (own auth, bypasses vhost + API keys) ──
    if url.starts_with("/admin/") || url == "/admin" {
        admin::handle(request, writer, admin_state, storage);
        return;
    }

    // ── VHost enforcement ────────────────────────────────────────────
    // If a vhost is configured, reject requests whose Host header
    // doesn't match. This acts like Apache's VirtualHost — only traffic
    // addressed to the correct hostname is served.
    if !vhost.is_empty() {
        if !check_vhost(&request, vhost) {
            let host_value = request.header("host").unwrap_or("(none)");
            println!(
                "[vhost] Rejected: Host '{}' does not match vhost '{}'",
                host_value, vhost
            );
            let body = format!(
                r#"{{"error":"misdirected request","expected":"{}"}}"#,
                vhost
            );
            let _ = http::write_response(
                &mut writer,
                421,
                "application/json",
                body.as_bytes(),
            );
            return;
        }
    }

    // ── Authentication ───────────────────────────────────────────────
    match security.check(&request) {
        AuthResult::Allowed => {}
        AuthResult::Denied(status, message) => {
            let body = format!(r#"{{"error":"{}"}}"#, message);
            let _ = http::write_response(
                &mut writer,
                status,
                "application/json",
                body.as_bytes(),
            );
            return;
        }
    }

    // ── Tier 1: Rust routes (first match wins) ──────────────────────
    let mut matched_route = None;
    for (i, route) in routes.iter().enumerate() {
        if method != route.method {
            continue;
        }
        let hit = if route.exact {
            url == route.prefix || url.starts_with(&format!("{}?", route.prefix))
        } else {
            url == route.prefix
                || url.starts_with(&format!("{}/", route.prefix))
                || url.starts_with(&format!("{}?", route.prefix))
        };
        if hit {
            matched_route = Some(i);
            break;
        }
    }

    if let Some(i) = matched_route {
        let route = &routes[i];
        let ctx = Context::new(request, writer, storage, route.prefix.len());
        (route.handler)(ctx);
        return;
    }

    // ── Tier 2: Config-driven engine ────────────────────────────────
    if let Some(engine) = config_engine {
        if engine.try_handle(&request, &mut writer, storage) {
            return;
        }
    }

    // ── Tier 3 & 4: Static files / 404 ──────────────────────────────
    if method == "GET" {
        static_files::serve(&url, &mut writer);
    } else {
        let _ = http::write_response(
            &mut writer,
            404,
            "application/json",
            br#"{"error":"not found"}"#,
        );
    }
}

/// Check if the request's Host header matches the configured vhost.
/// Strips port suffix and compares case-insensitively.
/// Returns true if the host matches (request should be served).
fn check_vhost(request: &HttpRequest, vhost: &str) -> bool {
    let host_header = match request.header("host") {
        Some(h) => h,
        None => return false, // No Host header = reject
    };

    // Strip port suffix (e.g., "example.com:443" → "example.com")
    let host = host_header.split(':').next().unwrap_or(host_header);

    let vhost_lower = vhost.to_ascii_lowercase();
    let host_lower = host.to_ascii_lowercase();

    // Exact match
    if host_lower == vhost_lower {
        return true;
    }

    // Allow www prefix (www.example.com matches example.com and vice versa)
    if let Some(stripped) = host_lower.strip_prefix("www.") {
        if stripped == vhost_lower {
            return true;
        }
    }
    if let Some(stripped) = vhost_lower.strip_prefix("www.") {
        if host_lower == stripped {
            return true;
        }
    }

    false
}
