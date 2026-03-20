// ═══════════════════════════════════════════════════════════════════
// Admin Security Manager — manage API keys and auth config
// ═══════════════════════════════════════════════════════════════════

use std::io::Write;

use crate::http::{self, HttpRequest};
use crate::storage::Storage;

const SECURITY_KEY: &str = "security__config";

pub fn handle_get(mut writer: Box<dyn Write + Send>, storage: &'static Storage) {
    match storage.get(SECURITY_KEY) {
        Some(config_json) => {
            // Parse to redact actual key values for safety
            if let Ok(mut config) = serde_json::from_str::<serde_json::Value>(&config_json) {
                if let Some(keys) = config.get_mut("api_keys").and_then(|v| v.as_array_mut()) {
                    for key_obj in keys.iter_mut() {
                        if let Some(k) = key_obj.get("key").and_then(|v| v.as_str()) {
                            // Show first 8 chars + masked remainder
                            let masked = if k.len() > 8 {
                                format!("{}...{}", &k[..8], "*".repeat(8))
                            } else {
                                "*".repeat(k.len())
                            };
                            key_obj
                                .as_object_mut()
                                .unwrap()
                                .insert("key_preview".to_string(), serde_json::json!(masked));
                        }
                    }
                }
                let body = serde_json::to_string_pretty(&config).unwrap_or_default();
                let _ =
                    http::write_response(&mut writer, 200, "application/json", body.as_bytes());
            } else {
                // Return raw if can't parse
                let _ = http::write_response(
                    &mut writer,
                    200,
                    "application/json",
                    config_json.as_bytes(),
                );
            }
        }
        None => {
            let _ = http::write_response(
                &mut writer,
                404,
                "application/json",
                br#"{"error":"security config not found in storage"}"#,
            );
        }
    }
}

pub fn handle_put(
    request: HttpRequest,
    mut writer: Box<dyn Write + Send>,
    storage: &'static Storage,
) {
    let body_str = String::from_utf8_lossy(&request.body).to_string();

    // Validate it's valid JSON
    let parsed = match serde_json::from_str::<serde_json::Value>(&body_str) {
        Ok(v) => v,
        Err(_) => {
            let _ = http::write_response(
                &mut writer,
                400,
                "application/json",
                br#"{"error":"body must be valid JSON"}"#,
            );
            return;
        }
    };

    // Validate structure: must have api_keys array
    if parsed.get("api_keys").and_then(|v| v.as_array()).is_none() {
        let _ = http::write_response(
            &mut writer,
            400,
            "application/json",
            br#"{"error":"config must contain an api_keys array"}"#,
        );
        return;
    }

    // Validate each key has required fields
    if let Some(keys) = parsed.get("api_keys").and_then(|v| v.as_array()) {
        for (i, key) in keys.iter().enumerate() {
            let has_key = key.get("key").and_then(|v| v.as_str()).is_some();
            let has_name = key.get("name").and_then(|v| v.as_str()).is_some();
            let has_role = key.get("role").and_then(|v| v.as_str()).is_some();

            if !has_key || !has_name || !has_role {
                let body = format!(
                    r#"{{"error":"api_keys[{}] must have key, name, and role fields"}}"#,
                    i
                );
                let _ =
                    http::write_response(&mut writer, 400, "application/json", body.as_bytes());
                return;
            }

            let role = key.get("role").unwrap().as_str().unwrap();
            if role != "admin" && role != "read" {
                let body = format!(
                    r#"{{"error":"api_keys[{}] role must be \"admin\" or \"read\""}}"#,
                    i
                );
                let _ =
                    http::write_response(&mut writer, 400, "application/json", body.as_bytes());
                return;
            }
        }
    }

    match storage.set(SECURITY_KEY, &body_str) {
        Ok(()) => {
            let _ = http::write_response(
                &mut writer,
                200,
                "application/json",
                br#"{"status":"saved","note":"changes take effect on next request"}"#,
            );
        }
        Err(e) => {
            let body = format!(r#"{{"error":"{}"}}"#, e);
            let _ = http::write_response(&mut writer, 500, "application/json", body.as_bytes());
        }
    }
}
