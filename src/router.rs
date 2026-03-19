// ═══════════════════════════════════════════════════════════════════
// Router — authentication + three-tier request dispatch
// ═══════════════════════════════════════════════════════════════════
//
// Requests are processed in this order:
//
//   0. Authentication (API key check)
//   1. Rust routes (src/api/*.rs)     — compiled handlers, full power
//   2. Config engine (endpoints.json) — runtime CRUD, no rebuild
//   3. Static files (frontend/*)      — HTML/CSS/JS/images
//   4. 404 fallback
//
// ═══════════════════════════════════════════════════════════════════

use std::io::Write;

use crate::api::{ConfigEngine, Context, Route};
use crate::http::{self, HttpRequest};
use crate::security::{AuthResult, SecurityConfig};
use crate::static_files;
use crate::storage::Storage;

/// Dispatch a request through the security + three-tier pipeline.
pub fn handle_request(
    request: HttpRequest,
    mut writer: Box<dyn Write + Send>,
    routes: &[Route],
    config_engine: &Option<ConfigEngine>,
    storage: &'static Storage,
    security: &SecurityConfig,
) {
    let url = request.url.clone();
    let method = request.method.clone();

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
