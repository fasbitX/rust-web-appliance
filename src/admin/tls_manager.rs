// ═══════════════════════════════════════════════════════════════════
// Admin TLS Manager — cert info and hot-reload
// ═══════════════════════════════════════════════════════════════════

use std::io::Write;

use crate::http::{self, HttpRequest};
use crate::tls;

use super::AdminState;

pub fn handle_get(mut writer: Box<dyn Write + Send>, _admin_state: &AdminState) {
    let source = if std::fs::metadata("/data/tls/cert.pem").is_ok() {
        "VirtioFS (/data/tls/)"
    } else {
        "embedded dev certificate"
    };

    let has_custom_certs = std::fs::metadata("/data/tls/cert.pem").is_ok()
        && std::fs::metadata("/data/tls/key.pem").is_ok();

    let body = format!(
        r#"{{
  "source": "{}",
  "has_custom_certs": {},
  "cert_path": "/data/tls/cert.pem",
  "key_path": "/data/tls/key.pem"
}}"#,
        source, has_custom_certs,
    );

    let _ = http::write_response(&mut writer, 200, "application/json", body.as_bytes());
}

pub fn handle_upload(
    request: HttpRequest,
    mut writer: Box<dyn Write + Send>,
    admin_state: &AdminState,
) {
    // Parse JSON body: {"cert": "PEM...", "key": "PEM..."}
    let body_str = String::from_utf8_lossy(&request.body).to_string();
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&body_str);

    let (cert_pem, key_pem) = match parsed {
        Ok(v) => {
            let c = v.get("cert").and_then(|v| v.as_str()).unwrap_or("");
            let k = v.get("key").and_then(|v| v.as_str()).unwrap_or("");
            (c.to_string(), k.to_string())
        }
        Err(_) => {
            let _ = http::write_response(
                &mut writer,
                400,
                "application/json",
                br#"{"error":"invalid JSON body"}"#,
            );
            return;
        }
    };

    if cert_pem.is_empty() || key_pem.is_empty() {
        let _ = http::write_response(
            &mut writer,
            400,
            "application/json",
            br#"{"error":"both cert and key PEM fields are required"}"#,
        );
        return;
    }

    // Write PEM files to /data/tls/
    if let Err(e) = std::fs::create_dir_all("/data/tls") {
        let body = format!(r#"{{"error":"failed to create /data/tls/: {}"}}"#, e);
        let _ = http::write_response(&mut writer, 500, "application/json", body.as_bytes());
        return;
    }

    if let Err(e) = std::fs::write("/data/tls/cert.pem", &cert_pem) {
        let body = format!(r#"{{"error":"failed to write cert.pem: {}"}}"#, e);
        let _ = http::write_response(&mut writer, 500, "application/json", body.as_bytes());
        return;
    }

    if let Err(e) = std::fs::write("/data/tls/key.pem", &key_pem) {
        let body = format!(r#"{{"error":"failed to write key.pem: {}"}}"#, e);
        let _ = http::write_response(&mut writer, 500, "application/json", body.as_bytes());
        return;
    }

    // Hot-reload TLS config
    match tls::reload_from_files() {
        Ok(new_config) => {
            // Swap the TLS config — next connection will use the new certs
            if let Ok(mut config) = admin_state.tls_config.write() {
                *config = new_config;
            }
            admin_state
                .log_buffer
                .capture("[admin] TLS certificates updated and hot-reloaded");
            let _ = http::write_response(
                &mut writer,
                200,
                "application/json",
                br#"{"status":"certificates uploaded and reloaded"}"#,
            );
        }
        Err(e) => {
            let body = format!(
                r#"{{"error":"certs written but reload failed: {}. Previous certs still active."}}"#,
                e
            );
            admin_state
                .log_buffer
                .capture(&format!("[admin] TLS reload failed: {}", e));
            let _ = http::write_response(&mut writer, 500, "application/json", body.as_bytes());
        }
    }
}
