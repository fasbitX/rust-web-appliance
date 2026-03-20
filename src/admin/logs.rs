// ═══════════════════════════════════════════════════════════════════
// Admin Log Buffer — ring buffer capturing output for web viewer
// ═══════════════════════════════════════════════════════════════════

use std::collections::VecDeque;
use std::io::Write;
use std::sync::Mutex;

use crate::http;

use super::AdminState;

const LOG_BUFFER_SIZE: usize = 2000;

pub struct LogBuffer {
    buffer: Mutex<VecDeque<String>>,
}

impl LogBuffer {
    pub fn new() -> Self {
        LogBuffer {
            buffer: Mutex::new(VecDeque::with_capacity(LOG_BUFFER_SIZE)),
        }
    }

    /// Write a line to both serial (println!) and the ring buffer.
    pub fn capture(&self, line: &str) {
        println!("{}", line);
        if let Ok(mut buf) = self.buffer.lock() {
            if buf.len() >= LOG_BUFFER_SIZE {
                buf.pop_front();
            }
            buf.push_back(line.to_string());
        }
    }

    /// Return the last `n` lines from the buffer.
    pub fn recent(&self, n: usize) -> Vec<String> {
        if let Ok(buf) = self.buffer.lock() {
            let skip = buf.len().saturating_sub(n);
            buf.iter().skip(skip).cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Clear the log buffer.
    pub fn clear(&self) {
        if let Ok(mut buf) = self.buffer.lock() {
            buf.clear();
        }
    }

    /// Number of lines currently in the buffer.
    pub fn len(&self) -> usize {
        self.buffer.lock().map(|b| b.len()).unwrap_or(0)
    }
}

// ── HTTP handlers ────────────────────────────────────────────────

pub fn handle_get(
    mut writer: Box<dyn Write + Send>,
    admin_state: &'static AdminState,
    lines: usize,
) {
    let logs = admin_state.log_buffer.recent(lines);
    let logs_json: Vec<String> = logs
        .iter()
        .map(|l| format!("\"{}\"", l.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    let body = format!(
        r#"{{"count":{},"total":{},"lines":[{}]}}"#,
        logs.len(),
        admin_state.log_buffer.len(),
        logs_json.join(",")
    );
    let _ = http::write_response(&mut writer, 200, "application/json", body.as_bytes());
}

pub fn handle_clear(mut writer: Box<dyn Write + Send>, admin_state: &'static AdminState) {
    admin_state.log_buffer.clear();
    let _ = http::write_response(
        &mut writer,
        200,
        "application/json",
        br#"{"status":"log buffer cleared"}"#,
    );
}
