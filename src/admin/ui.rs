// ═══════════════════════════════════════════════════════════════════
// Admin UI — serves the embedded admin console HTML
// ═══════════════════════════════════════════════════════════════════

use std::io::Write;

use crate::http;

const ADMIN_HTML: &str = include_str!("../../admin_ui/admin.html");

pub fn serve(_path: &str, mut writer: Box<dyn Write + Send>) {
    // All /admin/* GET requests serve the single-page admin UI
    let _ = http::write_response(&mut writer, 200, "text/html; charset=utf-8", ADMIN_HTML.as_bytes());
}
