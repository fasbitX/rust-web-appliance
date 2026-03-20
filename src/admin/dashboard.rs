// ═══════════════════════════════════════════════════════════════════
// Admin Dashboard — system stats and health info
// ═══════════════════════════════════════════════════════════════════

use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::http;
use crate::storage::Storage;

use super::AdminState;

pub fn handle(
    mut writer: Box<dyn Write + Send>,
    admin_state: &'static AdminState,
    storage: &'static Storage,
) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let uptime = now.saturating_sub(admin_state.boot_time);

    let keys = storage.list_keys().unwrap_or_default();
    let active_sessions = admin_state.sessions.active_count();
    let log_lines = admin_state.log_buffer.len();
    let total_requests = admin_state.total_requests();

    let body = format!(
        r#"{{
  "version": "{}",
  "os": "hermit",
  "uptime_seconds": {},
  "boot_time": {},
  "total_requests": {},
  "active_sessions": {},
  "log_buffer_lines": {},
  "storage_keys": {},
  "tls_source": "{}"
}}"#,
        env!("CARGO_PKG_VERSION"),
        uptime,
        admin_state.boot_time,
        total_requests,
        active_sessions,
        log_lines,
        keys.len(),
        tls_source(admin_state),
    );

    let _ = http::write_response(&mut writer, 200, "application/json", body.as_bytes());
}

fn tls_source(_admin_state: &AdminState) -> &'static str {
    // Check if custom certs are loaded by trying to read the files
    if std::fs::metadata("/data/tls/cert.pem").is_ok() {
        "VirtioFS (/data/tls/)"
    } else {
        "embedded dev certificate"
    }
}
