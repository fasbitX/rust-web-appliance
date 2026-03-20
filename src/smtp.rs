// ═══════════════════════════════════════════════════════════════════
// SMTP Client — pure Rust, synchronous email sending
// ═══════════════════════════════════════════════════════════════════
//
// Implements SMTP over plain TCP, STARTTLS, and direct TLS.
// Uses std::net::TcpStream for connectivity and rustls for TLS.
// No async, no C FFI — compatible with HermitOS.
//
// Supports AUTH PLAIN for authentication.
// Server certificate verification is disabled (many SMTP servers
// use self-signed certs); this is acceptable for an internal appliance.
// ═══════════════════════════════════════════════════════════════════

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::storage::Storage;

const SMTP_CONFIG_KEY: &str = "smtp__config";
const SMTP_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_address: String,
    pub from_name: String,
    /// "starttls", "tls", or "none"
    pub encryption: String,
}

impl SmtpConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.host.is_empty() {
            return Err("host is required".into());
        }
        if self.port == 0 {
            return Err("port must be greater than 0".into());
        }
        if self.from_address.is_empty() {
            return Err("from_address is required".into());
        }
        validate_email_addr(&self.from_address)?;
        match self.encryption.as_str() {
            "starttls" | "tls" | "none" => Ok(()),
            other => Err(format!("encryption must be 'starttls', 'tls', or 'none', got '{}'", other)),
        }
    }

    pub fn load(storage: &Storage) -> Option<Self> {
        let json = storage.get(SMTP_CONFIG_KEY)?;
        serde_json::from_str(&json).ok()
    }

    pub fn save(&self, storage: &Storage) -> Result<(), String> {
        let json = serde_json::to_string(self).map_err(|e| format!("serialize: {}", e))?;
        storage.set(SMTP_CONFIG_KEY, &json).map_err(|e| format!("storage: {}", e))
    }
}

/// Send an email using the given SMTP configuration.
pub fn send_email(
    config: &SmtpConfig,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), String> {
    config.validate()?;
    validate_email_addr(to)?;

    let addr = format!("{}:{}", config.host, config.port);
    let tcp = TcpStream::connect(&addr).map_err(|e| format!("connect to {}: {}", addr, e))?;
    tcp.set_read_timeout(Some(Duration::from_secs(SMTP_TIMEOUT_SECS))).ok();
    tcp.set_write_timeout(Some(Duration::from_secs(SMTP_TIMEOUT_SECS))).ok();

    match config.encryption.as_str() {
        "none" => send_plain(tcp, config, to, subject, body),
        "tls" => send_direct_tls(tcp, config, to, subject, body),
        "starttls" => send_starttls(tcp, config, to, subject, body),
        _ => Err("unsupported encryption mode".into()),
    }
}

// ── Plain (no encryption) ────────────────────────────────────────

fn send_plain(
    mut stream: TcpStream,
    config: &SmtpConfig,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), String> {
    smtp_session(&mut stream, config, to, subject, body)
}

// ── Direct TLS (port 465) ────────────────────────────────────────

fn send_direct_tls(
    tcp: TcpStream,
    config: &SmtpConfig,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), String> {
    let mut tls = make_tls_stream(tcp, &config.host)?;
    smtp_session(&mut tls, config, to, subject, body)
}

// ── STARTTLS (port 587) ─────────────────────────────────────────

fn send_starttls(
    mut tcp: TcpStream,
    config: &SmtpConfig,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), String> {
    // Phase 1: plaintext greeting + EHLO + STARTTLS command
    smtp_expect(&mut tcp, 220)?;
    smtp_write(&mut tcp, "EHLO localhost\r\n")?;
    smtp_expect(&mut tcp, 250)?;
    smtp_write(&mut tcp, "STARTTLS\r\n")?;
    smtp_expect(&mut tcp, 220)?;

    // Phase 2: upgrade connection to TLS
    let mut tls = make_tls_stream(tcp, &config.host)?;

    // Phase 3: EHLO again over TLS, then auth + send
    smtp_write(&mut tls, "EHLO localhost\r\n")?;
    smtp_expect(&mut tls, 250)?;

    if !config.username.is_empty() {
        smtp_auth_plain(&mut tls, &config.username, &config.password)?;
    }

    smtp_mail_transaction(&mut tls, config, to, subject, body)?;

    smtp_write(&mut tls, "QUIT\r\n")?;
    let _ = smtp_expect(&mut tls, 221);

    Ok(())
}

// ── Full SMTP session (greeting through QUIT) ───────────────────

fn smtp_session<S: Read + Write>(
    stream: &mut S,
    config: &SmtpConfig,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), String> {
    smtp_expect(stream, 220)?;

    smtp_write(stream, "EHLO localhost\r\n")?;
    smtp_expect(stream, 250)?;

    if !config.username.is_empty() {
        smtp_auth_plain(stream, &config.username, &config.password)?;
    }

    smtp_mail_transaction(stream, config, to, subject, body)?;

    smtp_write(stream, "QUIT\r\n")?;
    let _ = smtp_expect(stream, 221);

    Ok(())
}

// ── MAIL FROM / RCPT TO / DATA ──────────────────────────────────

fn smtp_mail_transaction<S: Read + Write>(
    stream: &mut S,
    config: &SmtpConfig,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), String> {
    // MAIL FROM
    smtp_write(stream, &format!("MAIL FROM:<{}>\r\n", config.from_address))?;
    smtp_expect(stream, 250)?;

    // RCPT TO
    smtp_write(stream, &format!("RCPT TO:<{}>\r\n", to))?;
    smtp_expect(stream, 250)?;

    // DATA
    smtp_write(stream, "DATA\r\n")?;
    smtp_expect(stream, 354)?;

    // Headers
    let from_header = if config.from_name.is_empty() {
        format!("<{}>", config.from_address)
    } else {
        format!("{} <{}>", config.from_name, config.from_address)
    };
    smtp_write(stream, &format!("From: {}\r\n", from_header))?;
    smtp_write(stream, &format!("To: <{}>\r\n", to))?;
    smtp_write(stream, &format!("Subject: {}\r\n", subject))?;
    smtp_write(stream, "MIME-Version: 1.0\r\n")?;
    smtp_write(stream, "Content-Type: text/plain; charset=utf-8\r\n")?;
    smtp_write(stream, "\r\n")?;

    // Body (escape leading dots per RFC 5321 section 4.5.2)
    for line in body.lines() {
        if line.starts_with('.') {
            smtp_write(stream, &format!(".{}\r\n", line))?;
        } else {
            smtp_write(stream, &format!("{}\r\n", line))?;
        }
    }

    // End of data
    smtp_write(stream, ".\r\n")?;
    smtp_expect(stream, 250)?;

    Ok(())
}

// ── AUTH PLAIN ──────────────────────────────────────────────────

fn smtp_auth_plain<S: Read + Write>(
    stream: &mut S,
    username: &str,
    password: &str,
) -> Result<(), String> {
    // AUTH PLAIN: base64("\0username\0password")
    let mut auth_bytes = Vec::new();
    auth_bytes.push(0);
    auth_bytes.extend_from_slice(username.as_bytes());
    auth_bytes.push(0);
    auth_bytes.extend_from_slice(password.as_bytes());
    let encoded = base64_encode(&auth_bytes);

    smtp_write(stream, &format!("AUTH PLAIN {}\r\n", encoded))?;
    smtp_expect(stream, 235)?;

    Ok(())
}

// ── TLS Setup ───────────────────────────────────────────────────

fn make_tls_stream(
    tcp: TcpStream,
    host: &str,
) -> Result<rustls::StreamOwned<rustls::ClientConnection, TcpStream>, String> {
    let config = make_tls_client_config()?;
    let server_name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|_| format!("invalid server name: {}", host))?;
    let conn = rustls::ClientConnection::new(config, server_name)
        .map_err(|e| format!("TLS handshake: {}", e))?;
    Ok(rustls::StreamOwned::new(conn, tcp))
}

fn make_tls_client_config() -> Result<Arc<rustls::ClientConfig>, String> {
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls_rustcrypto::provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| format!("TLS config: {}", e))?
    .dangerous()
    .with_custom_certificate_verifier(Arc::new(NoVerifier))
    .with_no_client_auth();

    Ok(Arc::new(config))
}

/// Certificate verifier that accepts all server certificates.
/// Appropriate for internal SMTP relays that often use self-signed certs.
#[derive(Debug)]
struct NoVerifier;

impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls_rustcrypto::provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

// ── SMTP Protocol Helpers ───────────────────────────────────────

fn smtp_write<W: Write>(writer: &mut W, data: &str) -> Result<(), String> {
    writer.write_all(data.as_bytes()).map_err(|e| format!("SMTP write: {}", e))?;
    writer.flush().map_err(|e| format!("SMTP flush: {}", e))?;
    Ok(())
}

/// Read an SMTP response (possibly multi-line) and verify the status code.
fn smtp_expect<R: Read>(reader: &mut R, expected: u16) -> Result<String, String> {
    let (code, response) = read_smtp_response(reader)?;
    if code != expected {
        Err(format!(
            "SMTP error: expected {}, got {} — {}",
            expected,
            code,
            response.trim()
        ))
    } else {
        Ok(response)
    }
}

/// Read a full SMTP response (handles multi-line 250-continuation).
/// Returns (status_code, full_response_text).
fn read_smtp_response<R: Read>(reader: &mut R) -> Result<(u16, String), String> {
    let mut full = String::new();
    loop {
        let line = read_line_raw(reader)?;
        full.push_str(&line);
        full.push('\n');

        if line.len() < 3 {
            return Err(format!("invalid SMTP response: {}", line));
        }

        let code: u16 = line[..3]
            .parse()
            .map_err(|_| format!("invalid SMTP response code: {}", line))?;

        // Multi-line responses have '-' after the code; last line has ' ' or nothing
        if line.len() == 3 || line.as_bytes()[3] != b'-' {
            return Ok((code, full));
        }
    }
}

/// Read a single line from the stream, byte by byte, until CRLF.
/// This avoids buffering issues critical for STARTTLS upgrade.
fn read_line_raw<R: Read>(reader: &mut R) -> Result<String, String> {
    let mut buf = Vec::with_capacity(512);
    let mut byte = [0u8; 1];
    loop {
        match reader.read_exact(&mut byte) {
            Ok(()) => {}
            Err(e) => return Err(format!("SMTP read: {}", e)),
        }
        buf.push(byte[0]);
        if buf.len() >= 2 && buf[buf.len() - 2] == b'\r' && buf[buf.len() - 1] == b'\n' {
            buf.truncate(buf.len() - 2);
            break;
        }
        if buf.len() > 2048 {
            return Err("SMTP response line too long (>2048 bytes)".into());
        }
    }
    String::from_utf8(buf).map_err(|e| format!("SMTP response not UTF-8: {}", e))
}

// ── Validation ──────────────────────────────────────────────────

fn validate_email_addr(addr: &str) -> Result<(), String> {
    if addr.contains('\r') || addr.contains('\n') || addr.contains('>') || addr.contains('<') {
        return Err(format!("invalid email address: contains forbidden characters"));
    }
    if !addr.contains('@') {
        return Err(format!("invalid email address '{}': missing @", addr));
    }
    Ok(())
}

// ── Base64 Encoder ──────────────────────────────────────────────

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((n >> 18) & 63) as usize] as char);
        result.push(CHARS[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((n >> 6) & 63) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(n & 63) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
