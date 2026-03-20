// ═══════════════════════════════════════════════════════════════════
// Admin Endpoints Manager — view/edit endpoints.json config
// ═══════════════════════════════════════════════════════════════════
//
// Note: Changes to endpoints.json take effect on next boot.
// The config engine is loaded once at startup and is not hot-reloaded.
// ═══════════════════════════════════════════════════════════════════

use std::io::Write;

use crate::http::{self, HttpRequest};

const ENDPOINTS_PATH: &str = "/backend/endpoints.json";
const ENDPOINTS_HOST_PATH: &str = "backend/endpoints.json";

pub fn handle_get(mut writer: Box<dyn Write + Send>) {
    // Try VirtioFS path first, then host path
    let content = std::fs::read_to_string(ENDPOINTS_PATH)
        .or_else(|_| std::fs::read_to_string(ENDPOINTS_HOST_PATH));

    match content {
        Ok(json) => {
            let _ = http::write_response(&mut writer, 200, "application/json", json.as_bytes());
        }
        Err(_) => {
            let _ = http::write_response(
                &mut writer,
                404,
                "application/json",
                br#"{"error":"endpoints.json not found"}"#,
            );
        }
    }
}

pub fn handle_put(request: HttpRequest, mut writer: Box<dyn Write + Send>) {
    let body_str = String::from_utf8_lossy(&request.body).to_string();

    // Validate it's valid JSON
    if serde_json::from_str::<serde_json::Value>(&body_str).is_err() {
        let _ = http::write_response(
            &mut writer,
            400,
            "application/json",
            br#"{"error":"body must be valid JSON"}"#,
        );
        return;
    }

    // Try VirtioFS path first, then host path
    let result = std::fs::write(ENDPOINTS_PATH, &body_str)
        .or_else(|_| std::fs::write(ENDPOINTS_HOST_PATH, &body_str));

    match result {
        Ok(()) => {
            let _ = http::write_response(
                &mut writer,
                200,
                "application/json",
                br#"{"status":"saved","note":"restart required to apply changes"}"#,
            );
        }
        Err(e) => {
            let body = format!(r#"{{"error":"failed to write endpoints.json: {}"}}"#, e);
            let _ = http::write_response(&mut writer, 500, "application/json", body.as_bytes());
        }
    }
}
