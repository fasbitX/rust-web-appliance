// ═══════════════════════════════════════════════════════════════════
// Admin SMTP Manager — configure and test SMTP email sending
// ═══════════════════════════════════════════════════════════════════

use std::io::Write;

use crate::http::{self, HttpRequest};
use crate::smtp::SmtpConfig;
use crate::storage::Storage;

/// GET /admin/api/smtp — return current SMTP configuration (password masked)
pub fn handle_get(mut writer: Box<dyn Write + Send>, storage: &'static Storage) {
    match SmtpConfig::load(storage) {
        Some(config) => {
            let masked_password = if config.password.is_empty() {
                String::new()
            } else {
                let len = config.password.len();
                if len > 4 {
                    format!("{}...{}", &config.password[..2], "*".repeat(6))
                } else {
                    "*".repeat(len)
                }
            };

            let body = serde_json::json!({
                "configured": true,
                "host": config.host,
                "port": config.port,
                "username": config.username,
                "password_preview": masked_password,
                "from_address": config.from_address,
                "from_name": config.from_name,
                "encryption": config.encryption,
            });
            let json = serde_json::to_string(&body).unwrap_or_default();
            let _ = http::write_response(&mut writer, 200, "application/json", json.as_bytes());
        }
        None => {
            let _ = http::write_response(
                &mut writer,
                200,
                "application/json",
                br#"{"configured":false}"#,
            );
        }
    }
}

/// PUT /admin/api/smtp — save SMTP configuration
pub fn handle_put(
    request: HttpRequest,
    mut writer: Box<dyn Write + Send>,
    storage: &'static Storage,
) {
    let body_str = String::from_utf8_lossy(&request.body).to_string();

    let config: SmtpConfig = match serde_json::from_str(&body_str) {
        Ok(c) => c,
        Err(e) => {
            let body = format!(r#"{{"error":"invalid JSON: {}"}}"#, e);
            let _ = http::write_response(&mut writer, 400, "application/json", body.as_bytes());
            return;
        }
    };

    if let Err(e) = config.validate() {
        let body = format!(r#"{{"error":"{}"}}"#, e);
        let _ = http::write_response(&mut writer, 400, "application/json", body.as_bytes());
        return;
    }

    match config.save(storage) {
        Ok(()) => {
            println!("[smtp] Configuration saved (host={}:{})", config.host, config.port);
            let _ = http::write_response(
                &mut writer,
                200,
                "application/json",
                br#"{"status":"SMTP configuration saved"}"#,
            );
        }
        Err(e) => {
            let body = format!(r#"{{"error":"{}"}}"#, e);
            let _ = http::write_response(&mut writer, 500, "application/json", body.as_bytes());
        }
    }
}

/// POST /admin/api/smtp/test — send a test email
pub fn handle_test(
    request: HttpRequest,
    mut writer: Box<dyn Write + Send>,
    storage: &'static Storage,
) {
    // Parse the test request: {"to": "someone@example.com"}
    let body_str = String::from_utf8_lossy(&request.body).to_string();
    let parsed: serde_json::Value = match serde_json::from_str(&body_str) {
        Ok(v) => v,
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

    let to = match parsed.get("to").and_then(|v| v.as_str()) {
        Some(addr) if !addr.is_empty() => addr.to_string(),
        _ => {
            let _ = http::write_response(
                &mut writer,
                400,
                "application/json",
                br#"{"error":"'to' email address is required"}"#,
            );
            return;
        }
    };

    let config = match SmtpConfig::load(storage) {
        Some(c) => c,
        None => {
            let _ = http::write_response(
                &mut writer,
                400,
                "application/json",
                br#"{"error":"SMTP not configured - save configuration first"}"#,
            );
            return;
        }
    };

    println!("[smtp] Sending test email to {}...", to);

    match crate::smtp::send_email(
        &config,
        &to,
        "Rust Web Appliance — SMTP Test",
        "This is a test email from Rust Web Appliance.\n\nIf you received this, your SMTP configuration is working correctly.",
    ) {
        Ok(()) => {
            println!("[smtp] Test email sent successfully to {}", to);
            let body = format!(
                r#"{{"status":"Test email sent successfully to {}"}}"#,
                to.replace('"', "'")
            );
            let _ = http::write_response(&mut writer, 200, "application/json", body.as_bytes());
        }
        Err(e) => {
            println!("[smtp] Test email failed: {}", e);
            let safe_err = e.replace('"', "'").replace('\\', "/");
            let body = format!(r#"{{"error":"{}"}}"#, safe_err);
            let _ = http::write_response(&mut writer, 500, "application/json", body.as_bytes());
        }
    }
}
