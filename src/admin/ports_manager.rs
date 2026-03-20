// ═══════════════════════════════════════════════════════════════════
// Admin Ports Manager — read/write port configuration
// ═══════════════════════════════════════════════════════════════════

use std::io::Write;

use crate::http::{self, HttpRequest};
use crate::ports::PortConfig;

pub fn handle_get(mut writer: Box<dyn Write + Send>) {
    let config = PortConfig::load();
    match serde_json::to_string_pretty(&config) {
        Ok(json) => {
            let _ = http::write_response(&mut writer, 200, "application/json", json.as_bytes());
        }
        Err(e) => {
            let body = format!(r#"{{"error":"{}"}}"#, e);
            let _ = http::write_response(&mut writer, 500, "application/json", body.as_bytes());
        }
    }
}

pub fn handle_put(request: HttpRequest, mut writer: Box<dyn Write + Send>) {
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

    match config.save() {
        Ok(()) => {
            let _ = http::write_response(
                &mut writer,
                200,
                "application/json",
                br#"{"status":"saved - restart required for changes to take effect"}"#,
            );
        }
        Err(e) => {
            let body = format!(r#"{{"error":"{}"}}"#, e);
            let _ = http::write_response(&mut writer, 500, "application/json", body.as_bytes());
        }
    }
}
