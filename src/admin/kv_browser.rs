// ═══════════════════════════════════════════════════════════════════
// Admin KV Browser — view/edit/delete storage entries
// ═══════════════════════════════════════════════════════════════════

use std::io::Write;

use crate::http::{self, HttpRequest};
use crate::storage::Storage;

pub fn handle_list(mut writer: Box<dyn Write + Send>, storage: &'static Storage) {
    match storage.list_keys() {
        Ok(keys) => {
            let keys_json: Vec<String> = keys.iter().map(|k| format!("\"{}\"", k)).collect();
            let body = format!(
                r#"{{"count":{},"keys":[{}]}}"#,
                keys.len(),
                keys_json.join(",")
            );
            let _ = http::write_response(&mut writer, 200, "application/json", body.as_bytes());
        }
        Err(e) => {
            let body = format!(r#"{{"error":"{}"}}"#, e);
            let _ = http::write_response(&mut writer, 500, "application/json", body.as_bytes());
        }
    }
}

pub fn handle_get(mut writer: Box<dyn Write + Send>, storage: &'static Storage, key: &str) {
    if key.is_empty() {
        let _ = http::write_response(
            &mut writer,
            400,
            "application/json",
            br#"{"error":"key is required"}"#,
        );
        return;
    }

    match storage.get(key) {
        Some(value) => {
            // Try to return the value as-is if it's valid JSON, otherwise wrap it
            if serde_json::from_str::<serde_json::Value>(&value).is_ok() {
                let body = format!(r#"{{"key":"{}","value":{}}}"#, key, value);
                let _ =
                    http::write_response(&mut writer, 200, "application/json", body.as_bytes());
            } else {
                let body = format!(r#"{{"key":"{}","value":"{}"}}"#, key, value);
                let _ =
                    http::write_response(&mut writer, 200, "application/json", body.as_bytes());
            }
        }
        None => {
            let _ = http::write_response(
                &mut writer,
                404,
                "application/json",
                br#"{"error":"key not found"}"#,
            );
        }
    }
}

pub fn handle_put(
    request: HttpRequest,
    mut writer: Box<dyn Write + Send>,
    storage: &'static Storage,
    key: &str,
) {
    if key.is_empty() {
        let _ = http::write_response(
            &mut writer,
            400,
            "application/json",
            br#"{"error":"key is required"}"#,
        );
        return;
    }

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

    match storage.set(key, &body_str) {
        Ok(()) => {
            let _ = http::write_response(
                &mut writer,
                200,
                "application/json",
                br#"{"status":"ok"}"#,
            );
        }
        Err(e) => {
            let body = format!(r#"{{"error":"{}"}}"#, e);
            let _ = http::write_response(&mut writer, 500, "application/json", body.as_bytes());
        }
    }
}

pub fn handle_delete(mut writer: Box<dyn Write + Send>, storage: &'static Storage, key: &str) {
    if key.is_empty() {
        let _ = http::write_response(
            &mut writer,
            400,
            "application/json",
            br#"{"error":"key is required"}"#,
        );
        return;
    }

    match storage.delete(key) {
        Ok(true) => {
            let _ = http::write_response(
                &mut writer,
                200,
                "application/json",
                br#"{"status":"deleted"}"#,
            );
        }
        Ok(false) => {
            let _ = http::write_response(
                &mut writer,
                404,
                "application/json",
                br#"{"error":"key not found"}"#,
            );
        }
        Err(e) => {
            let body = format!(r#"{{"error":"{}"}}"#, e);
            let _ = http::write_response(&mut writer, 500, "application/json", body.as_bytes());
        }
    }
}
