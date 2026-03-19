// ═══════════════════════════════════════════════════════════════════
// api/ — The Backend Module
// ═══════════════════════════════════════════════════════════════════
//
// This is where you build your API. The pattern:
//
//   1. Create a new file in src/api/  (e.g., src/api/users.rs)
//   2. Write handler functions:       pub fn list(ctx: Context) { ... }
//   3. Register them in routes():     Route::get("/api/users", users::list),
//
// That's it. The router, request parsing, JSON serialization,
// storage access — it's all handled for you via the Context object.
//
// ═══════════════════════════════════════════════════════════════════

mod context;
mod system;
mod kv;
mod example;
pub mod config_engine;

pub use context::Context;
pub use config_engine::ConfigEngine;

// ── Route Definition ────────────────────────────────────────────────

/// A single API route: method + path prefix → handler function.
pub struct Route {
    pub method: &'static str,
    pub prefix: &'static str,
    pub handler: fn(Context),
    pub exact: bool,
}

#[allow(dead_code)]
impl Route {
    /// Match exact path (e.g., "/api/health" matches only "/api/health")
    pub fn get(path: &'static str, handler: fn(Context)) -> Self {
        Route { method: "GET", prefix: path, handler, exact: true }
    }

    pub fn post(path: &'static str, handler: fn(Context)) -> Self {
        Route { method: "POST", prefix: path, handler, exact: true }
    }

    pub fn put(path: &'static str, handler: fn(Context)) -> Self {
        Route { method: "PUT", prefix: path, handler, exact: true }
    }

    pub fn delete(path: &'static str, handler: fn(Context)) -> Self {
        Route { method: "DELETE", prefix: path, handler, exact: true }
    }

    pub fn patch(path: &'static str, handler: fn(Context)) -> Self {
        Route { method: "PATCH", prefix: path, handler, exact: true }
    }

    /// Match path prefix (e.g., "/api/users" matches "/api/users/42/posts")
    /// Use ctx.param() and ctx.params() to extract path segments.
    pub fn get_prefix(path: &'static str, handler: fn(Context)) -> Self {
        Route { method: "GET", prefix: path, handler, exact: false }
    }

    pub fn post_prefix(path: &'static str, handler: fn(Context)) -> Self {
        Route { method: "POST", prefix: path, handler, exact: false }
    }

    pub fn put_prefix(path: &'static str, handler: fn(Context)) -> Self {
        Route { method: "PUT", prefix: path, handler, exact: false }
    }

    pub fn delete_prefix(path: &'static str, handler: fn(Context)) -> Self {
        Route { method: "DELETE", prefix: path, handler, exact: false }
    }

    pub fn patch_prefix(path: &'static str, handler: fn(Context)) -> Self {
        Route { method: "PATCH", prefix: path, handler, exact: false }
    }
}


// ═══════════════════════════════════════════════════════════════════
//  ROUTE TABLE — Register all your API endpoints here
// ═══════════════════════════════════════════════════════════════════
//
//  To add a new endpoint:
//    1. Create src/api/your_module.rs with handler functions
//    2. Add `mod your_module;` above
//    3. Add Route entries below
//
//  Routes are matched top-to-bottom. First match wins.
//  Prefix routes should come AFTER exact routes for the same path.
// ═══════════════════════════════════════════════════════════════════

pub fn routes() -> Vec<Route> {
    vec![
        // ── System (built-in) ───────────────────────────────────────
        Route::get("/api/health",           system::health),
        Route::get("/api/info",             system::info),

        // ── Key-Value Store (built-in) ──────────────────────────────
        Route::get("/api/kv",               kv::list_keys),
        Route::get_prefix("/api/kv",        kv::get_key),
        Route::put_prefix("/api/kv",        kv::put_key),
        Route::delete_prefix("/api/kv",     kv::delete_key),

        // ── Example routes (remove or replace these) ────────────────
        Route::get("/api/echo",             example::echo),
        Route::post("/api/echo",            example::echo_post),
        Route::get_prefix("/api/greet",     example::greet),

        // ─────────────────────────────────────────────────────────────
        //  YOUR ROUTES GO HERE
        // ─────────────────────────────────────────────────────────────
        // Route::get("/api/products",          products::list),
        // Route::get_prefix("/api/products",   products::get_by_id),
        // Route::post("/api/products",         products::create),
        // Route::put_prefix("/api/products",   products::update),
        // Route::delete_prefix("/api/products", products::remove),
    ]
}
