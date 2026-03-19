// ═══════════════════════════════════════════════════════════════════
// Router — three-tier request dispatch
// ═══════════════════════════════════════════════════════════════════
//
// Requests are matched in this order:
//
//   1. Rust routes (src/api/*.rs)     — compiled handlers, full power
//   2. Config engine (endpoints.json) — runtime CRUD, no rebuild
//   3. Static files (frontend/*)      — HTML/CSS/JS/images
//   4. 405 fallback
//
// You should never need to edit this file.
// ═══════════════════════════════════════════════════════════════════

use tiny_http::{Request, Response, Header, StatusCode};

use crate::api::{ConfigEngine, Context, Route};
use crate::static_files;
use crate::storage::Storage;

/// Dispatch a request through the three-tier pipeline.
pub fn handle_request(
    request: Request,
    routes: &[Route],
    config_engine: &Option<ConfigEngine>,
    storage: &'static Storage,
) {
    let url = request.url().to_string();
    let method = request.method().to_string();

    // ── Tier 1: Rust routes (first match wins) ──────────────────────
    // Check if any compiled route matches before consuming the request
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
        let ctx = Context::new(request, storage, route.prefix.len());
        (route.handler)(ctx);
        return;
    }

    // ── Tier 2: Config-driven engine ────────────────────────────────
    if let Some(engine) = config_engine {
        match engine.try_handle(request, storage) {
            Ok(()) => return,           // config engine handled it
            Err(req) => {
                // Not a config-driven route, fall through with request returned
                return dispatch_fallback(req, &method, &url);
            }
        }
    }

    // ── Tier 3 & 4: Static files / 405 ──────────────────────────────
    dispatch_fallback(request, &method, &url);
}

fn dispatch_fallback(request: Request, method: &str, url: &str) {
    if method == "GET" {
        static_files::serve(request, url);
    } else {
        let header = Header::from_bytes("Content-Type", "application/json").unwrap();
        let _ = request.respond(
            Response::from_string(r#"{"error":"not found"}"#)
                .with_status_code(StatusCode(404))
                .with_header(header),
        );
    }
}
