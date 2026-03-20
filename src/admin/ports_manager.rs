// ═══════════════════════════════════════════════════════════════════
// Admin Ports Manager — read/write port configuration
// ═══════════════════════════════════════════════════════════════════

use std::io::Write;

use crate::http::{self, HttpRequest};
use crate::ports::PortConfig;
use crate::storage::Storage;

pub fn handle_get(mut writer: Box<dyn Write + Send>, storage: &Storage) {
    let config = PortConfig::load(storage);
    let persistent = storage.is_persistent();

    // Include persistence status so the admin UI can warn the user
    let config_json = serde_json::to_string(&config).unwrap_or_default();
    let body = format!(
        r#"{{"persistent":{},"config":{}}}"#,
        persistent, config_json
    );
    let _ = http::write_response(&mut writer, 200, "application/json", body.as_bytes());
}

pub fn handle_put(request: HttpRequest, mut writer: Box<dyn Write + Send>, storage: &Storage) {
    let body_str = String::from_utf8_lossy(&request.body).to_string();
    let config: PortConfig = match serde_json::from_str(&body_str) {
        Ok(c) => c,
        Err(e) => {
            let body = format!(r#"{{"error":"invalid JSON: {}"}}"#, e);
            let _ = http::write_response(&mut writer, 400, "application/json", body.as_bytes());
            return;
        }
    };

    // Validate http.mode
    if config.http.mode != "redirect" && config.http.mode != "off" {
        let _ = http::write_response(
            &mut writer,
            400,
            "application/json",
            br#"{"error":"http.mode must be 'redirect' or 'off'"}"#,
        );
        return;
    }

    match config.save(storage) {
        Ok(()) => {
            let persistent = storage.is_persistent();
            let msg = if persistent {
                "saved - restart required for changes to take effect"
            } else {
                "saved to memory (no VirtioFS) - will be lost on restart. Use --virtiofs mode for persistent config."
            };
            let body = format!(r#"{{"status":"{}","persistent":{}}}"#, msg, persistent);
            let _ = http::write_response(&mut writer, 200, "application/json", body.as_bytes());
        }
        Err(e) => {
            let body = format!(r#"{{"error":"{}"}}"#, e);
            let _ = http::write_response(&mut writer, 500, "application/json", body.as_bytes());
        }
    }
}
