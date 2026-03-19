// ═══════════════════════════════════════════════════════════════════
// Example Routes — copy this file as a starting point for your API
// ═══════════════════════════════════════════════════════════════════
//
// Each handler is just a function that takes a Context and sends
// a response. You have access to:
//
//   ctx.param()          → path segment after the route prefix
//   ctx.params()         → all path segments as a Vec<&str>
//   ctx.query()          → query string (?foo=bar)
//   ctx.body_string()    → request body as String (call once, consumes body)
//   ctx.body_json::<T>() → parse body as JSON into any serde struct
//   ctx.storage          → the KV storage layer (get/set/delete/list_keys)
//
//   ctx.json(status, body)      → send JSON string
//   ctx.json_value(status, &v)  → serialize & send any serde value
//   ctx.text(status, body)      → send plain text
//   ctx.error(status, message)  → send {"error": "..."}
//
// To register your handlers, add them to routes() in src/api/mod.rs:
//
//   Route::get("/api/echo",          example::echo),
//   Route::post("/api/echo",         example::echo_post),
//   Route::get_prefix("/api/greet",  example::greet),
//
// ═══════════════════════════════════════════════════════════════════

use super::Context;
use serde::Serialize;

// ── GET /api/echo ───────────────────────────────────────────────────
// Simplest possible handler — returns a static JSON response.

pub fn echo(ctx: Context) {
    ctx.json(200, r#"{"echo":"hello from the appliance!"}"#);
}

// ── POST /api/echo ──────────────────────────────────────────────────
// Reads the request body and echoes it back.

pub fn echo_post(mut ctx: Context) {
    let body = ctx.body_string();
    let response = format!(r#"{{"you_sent":{}}}"#, body);
    ctx.json(200, &response);
}

// ── GET /api/greet/:name ────────────────────────────────────────────
// Shows how to use URL parameters and serde serialization.

#[derive(Serialize)]
struct Greeting {
    message: String,
    name: String,
}

pub fn greet(ctx: Context) {
    let name = ctx.param();
    if name.is_empty() {
        ctx.error(400, "usage: GET /api/greet/:name");
        return;
    }

    let greeting = Greeting {
        message: format!("Hello, {}! Welcome to the appliance.", name),
        name: name.to_string(),
    };

    ctx.json_value(200, &greeting);
}
