// ═══════════════════════════════════════════════════════════════════
// System Routes — built-in health check and appliance info
// ═══════════════════════════════════════════════════════════════════

use super::Context;

pub fn health(ctx: Context) {
    ctx.json(200, r#"{"status":"ok","appliance":"rust-web-appliance"}"#);
}

pub fn info(ctx: Context) {
    let body = format!(
        r#"{{"name":"{}","version":"{}","os":"hermit"}}"#,
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );
    ctx.json(200, &body);
}
