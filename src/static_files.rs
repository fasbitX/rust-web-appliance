// ═══════════════════════════════════════════════════════════════════
// Static File Server — serves HTML, CSS, JS, images from VirtioFS
// ═══════════════════════════════════════════════════════════════════
//
// Static files are served from /www on the VirtioFS mount.
// Falls back to embedded defaults for the index page.
// ═══════════════════════════════════════════════════════════════════

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::http;

const STATIC_ROOT: &str = "/www";

/// Embedded default index page (used when no /www directory exists)
const DEFAULT_INDEX: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Rust Web Appliance</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
            background: #0a0a0a; color: #e0e0e0;
            display: flex; justify-content: center; align-items: center;
            min-height: 100vh;
        }
        .container {
            text-align: center; padding: 2rem;
            border: 1px solid #333; border-radius: 12px;
            background: #111; max-width: 600px;
        }
        h1 { font-size: 2rem; margin-bottom: 0.5rem; color: #ff6b35; }
        .subtitle { color: #888; margin-bottom: 2rem; }
        .status { display: inline-block; padding: 0.5rem 1rem;
            background: #1a3a1a; color: #4caf50; border-radius: 6px;
            font-family: monospace; margin-bottom: 1.5rem; }
        .lock { color: #4caf50; font-size: 1.2rem; margin-bottom: 1rem; }
        .endpoints { text-align: left; font-family: monospace; font-size: 0.9rem; }
        .endpoints dt { color: #ff6b35; margin-top: 0.75rem; }
        .endpoints dd { color: #aaa; margin-left: 1rem; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Rust Web Appliance</h1>
        <p class="subtitle">HermitOS Unikernel - Single Binary - Zero OS Bloat</p>
        <div class="lock">HTTPS Secured</div>
        <div class="status">OPERATIONAL</div>
        <dl class="endpoints">
            <dt>GET /api/health</dt>
            <dd>Health check (public)</dd>
            <dt>GET /api/info</dt>
            <dd>Appliance metadata (requires API key)</dd>
            <dt>GET /api/kv</dt>
            <dd>List all keys (requires API key)</dd>
            <dt>PUT /api/kv/:key</dt>
            <dd>Write a value (requires admin API key)</dd>
        </dl>
        <p style="color:#666; margin-top:2rem; font-size:0.8rem;">
            Auth: X-API-Key header or Authorization: Bearer &lt;key&gt;
        </p>
    </div>
</body>
</html>"#;

pub fn serve(path: &str, writer: &mut dyn Write) {
    // Normalize path
    let file_path = if path == "/" {
        "index.html"
    } else {
        path.trim_start_matches('/')
    };

    // Prevent path traversal
    if file_path.contains("..") {
        let _ = http::write_response(writer, 403, "text/plain", b"Forbidden");
        return;
    }

    let full_path = format!("{}/{}", STATIC_ROOT, file_path);

    // Try filesystem first (VirtioFS)
    if let Ok(contents) = fs::read(&full_path) {
        let content_type = mime_for_path(&full_path);
        let _ = http::write_response(writer, 200, content_type, &contents);
        return;
    }

    // Fall back to embedded index for root
    if file_path == "index.html" {
        let _ = http::write_response(
            writer,
            200,
            "text/html; charset=utf-8",
            DEFAULT_INDEX.as_bytes(),
        );
        return;
    }

    // 404
    let _ = http::write_response(writer, 404, "text/plain", b"Not Found");
}

fn mime_for_path(path: &str) -> &'static str {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}
