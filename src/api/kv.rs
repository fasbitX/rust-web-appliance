// ═══════════════════════════════════════════════════════════════════
// Key-Value Store Routes — CRUD over the storage layer
// ═══════════════════════════════════════════════════════════════════

use super::Context;

/// GET /api/kv — list all stored keys
pub fn list_keys(ctx: Context) {
    match ctx.storage.list_keys() {
        Ok(keys) => {
            let keys_json: Vec<String> = keys.iter()
                .map(|k| format!(r#""{}""#, k))
                .collect();
            let body = format!(r#"{{"keys":[{}]}}"#, keys_json.join(","));
            ctx.json(200, &body);
        }
        Err(e) => ctx.error(500, &e.to_string()),
    }
}

/// GET /api/kv/:key — read a single value
pub fn get_key(ctx: Context) {
    let key = ctx.param();
    if key.is_empty() {
        ctx.error(400, "key is required");
        return;
    }
    match ctx.storage.get(key) {
        Some(value) => {
            let body = format!(r#"{{"key":"{}","value":{}}}"#, key, value);
            ctx.json(200, &body);
        }
        None => ctx.error(404, "not found"),
    }
}

/// PUT /api/kv/:key — write a value (body = JSON)
pub fn put_key(mut ctx: Context) {
    let key = ctx.param().to_string();
    if key.is_empty() {
        ctx.error(400, "key is required");
        return;
    }
    let body = ctx.body_string();
    if body.is_empty() {
        ctx.error(400, "body is required");
        return;
    }
    match ctx.storage.set(&key, &body) {
        Ok(()) => ctx.json(200, r#"{"status":"ok"}"#),
        Err(e) => ctx.error(500, &e.to_string()),
    }
}

/// DELETE /api/kv/:key — delete a key
pub fn delete_key(ctx: Context) {
    let key = ctx.param();
    if key.is_empty() {
        ctx.error(400, "key is required");
        return;
    }
    match ctx.storage.delete(key) {
        Ok(true) => ctx.json(200, r#"{"status":"deleted"}"#),
        Ok(false) => ctx.error(404, "not found"),
        Err(e) => ctx.error(500, &e.to_string()),
    }
}
