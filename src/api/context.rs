// ═══════════════════════════════════════════════════════════════════
// RequestContext — the single object every handler receives
// ═══════════════════════════════════════════════════════════════════
//
// Wraps the parsed HttpRequest and a writable stream so handler
// authors never need to deal with raw I/O.
//
// Usage in a handler:
//
//   pub fn my_handler(ctx: Context) {
//       let name = ctx.param();                     // URL segment after prefix
//       ctx.json(200, r#"{"ok": true}"#);           // send JSON response
//   }
//
//   pub fn my_post_handler(mut ctx: Context) {
//       let body = ctx.body_string();               // read request body
//       ctx.json(200, &format!(r#"{{"got":"{}"}}"#, body));
//   }
//
// ═══════════════════════════════════════════════════════════════════

use std::io::Write;

use crate::http::{self, HttpRequest};
use crate::storage::Storage;

/// Everything a handler needs to process a request and send a response.
#[allow(dead_code)]
pub struct Context {
    request: HttpRequest,
    writer: Option<Box<dyn Write + Send>>,
    url: String,
    method: String,
    prefix_len: usize,
    pub storage: &'static Storage,
}

#[allow(dead_code)]
impl Context {
    pub(crate) fn new(
        request: HttpRequest,
        writer: Box<dyn Write + Send>,
        storage: &'static Storage,
        prefix_len: usize,
    ) -> Self {
        let url = request.url.clone();
        let method = request.method.clone();
        Context {
            request,
            writer: Some(writer),
            url,
            method,
            prefix_len,
            storage,
        }
    }

    // ── Request accessors ───────────────────────────────────────────

    /// The full request URL (e.g., "/api/users/42?active=true")
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The HTTP method (e.g., "GET", "POST")
    pub fn method(&self) -> &str {
        &self.method
    }

    /// The URL path segment after the route prefix.
    ///
    /// If route is registered as "/api/users" and URL is "/api/users/42",
    /// then `param()` returns "42".
    ///
    /// Returns empty string if there's nothing after the prefix.
    pub fn param(&self) -> &str {
        let rest = &self.url[self.prefix_len..];
        let rest = rest.split('?').next().unwrap_or(rest);
        rest.strip_prefix('/').unwrap_or(rest)
    }

    /// Split the path after the prefix into segments.
    ///
    /// "/api/projects/7/tasks/3" with prefix "/api/projects" → ["7", "tasks", "3"]
    pub fn params(&self) -> Vec<&str> {
        self.param()
            .split('/')
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// The query string (everything after '?'), or empty.
    pub fn query(&self) -> &str {
        self.url.split_once('?').map(|(_, q)| q).unwrap_or("")
    }

    /// Read the request body as a String. Returns empty string if body is empty.
    pub fn body_string(&mut self) -> String {
        String::from_utf8_lossy(&self.request.body).to_string()
    }

    /// Read the request body and parse it as JSON.
    pub fn body_json<T: serde::de::DeserializeOwned>(&mut self) -> Result<T, String> {
        let raw = self.body_string();
        serde_json::from_str(&raw).map_err(|e| format!("JSON parse error: {}", e))
    }

    /// Get a request header by name (case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.request.header(name)
    }

    // ── Response helpers ────────────────────────────────────────────

    /// Send a JSON response with the given status code and body string.
    pub fn json(self, status: u16, body: &str) {
        self.respond_with(status, "application/json", body.as_bytes());
    }

    /// Serialize a value as JSON and send it with the given status code.
    pub fn json_value<T: serde::Serialize>(self, status: u16, value: &T) {
        match serde_json::to_string(value) {
            Ok(body) => self.json(status, &body),
            Err(e) => self.json(500, &format!(r#"{{"error":"serialize: {}"}}"#, e)),
        }
    }

    /// Send a plain text response.
    pub fn text(self, status: u16, body: &str) {
        self.respond_with(status, "text/plain; charset=utf-8", body.as_bytes());
    }

    /// Send raw bytes with a custom content type.
    pub fn bytes(self, status: u16, content_type: &str, data: &[u8]) {
        self.respond_with(status, content_type, data);
    }

    /// Send an error JSON response: {"error": "<message>"}
    pub fn error(self, status: u16, message: &str) {
        let body = format!(r#"{{"error":"{}"}}"#, message);
        self.json(status, &body);
    }

    /// Internal: consume the context and send a response.
    fn respond_with(mut self, status: u16, content_type: &str, data: &[u8]) {
        if let Some(mut writer) = self.writer.take() {
            if let Err(e) = http::write_response(&mut writer, status, content_type, data) {
                eprintln!("[api] Response error: {}", e);
            }
        }
    }
}
