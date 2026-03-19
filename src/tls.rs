// ═══════════════════════════════════════════════════════════════════
// TLS — rustls configuration with pure-Rust crypto (rustls-rustcrypto)
// ═══════════════════════════════════════════════════════════════════
//
// Certificate loading priority:
//   1. VirtioFS: /data/tls/cert.pem + /data/tls/key.pem
//   2. Embedded: compiled-in self-signed dev certificate
//
// Generate production certs: scripts/gen-cert.sh
// ═══════════════════════════════════════════════════════════════════

use std::fs;
use std::io::BufReader;
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;

const CERT_PATH: &str = "/data/tls/cert.pem";
const KEY_PATH: &str = "/data/tls/key.pem";

// Embedded self-signed dev certificate (EC P-256, valid 10 years)
const DEV_CERT_PEM: &str = include_str!("../certs/dev-cert.pem");
const DEV_KEY_PEM: &str = include_str!("../certs/dev-key.pem");

/// Initialize TLS with rustls + pure-Rust crypto provider.
pub fn init() -> Result<Arc<ServerConfig>, Box<dyn std::error::Error>> {
    let (certs, key, source) = load_certs()?;

    println!("[tls] Certificate source: {}", source);
    println!("[tls] Certificate chain: {} cert(s)", certs.len());

    if source == "embedded dev certificate" {
        println!("[tls] +--------------------------------------------------+");
        println!("[tls] |  WARNING: Using embedded dev certificate!         |");
        println!("[tls] |  For production, provide your own certs at:       |");
        println!("[tls] |    /data/tls/cert.pem                            |");
        println!("[tls] |    /data/tls/key.pem                             |");
        println!("[tls] |  Or run: scripts/gen-cert.sh                     |");
        println!("[tls] +--------------------------------------------------+");
    }

    // Build rustls config with pure-Rust crypto provider
    let provider = rustls_rustcrypto::provider();
    println!("[tls] Crypto provider: rustls-rustcrypto (pure Rust)");
    println!("[tls] Cipher suites: {}", provider.cipher_suites.len());
    for cs in &provider.cipher_suites {
        println!("[tls]   - {:?}", cs.suite());
    }

    let config = ServerConfig::builder_with_provider(Arc::new(provider))
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    println!("[tls] TLS configuration ready (pure-Rust crypto)");

    Ok(Arc::new(config))
}

fn load_certs()
    -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>, &'static str), Box<dyn std::error::Error>>
{
    // Try VirtioFS first
    if let (Ok(cert_pem), Ok(key_pem)) =
        (fs::read_to_string(CERT_PATH), fs::read_to_string(KEY_PATH))
    {
        let certs: Vec<CertificateDer<'static>> =
            rustls_pemfile::certs(&mut BufReader::new(cert_pem.as_bytes()))
                .collect::<Result<Vec<_>, _>>()?;

        let key = rustls_pemfile::private_key(&mut BufReader::new(key_pem.as_bytes()))?
            .ok_or("no private key found in PEM file")?;

        if certs.is_empty() {
            return Err("no certificates found in PEM file".into());
        }

        return Ok((certs, key, "VirtioFS (/data/tls/)"));
    }

    // Fall back to embedded dev certificate
    let certs: Vec<CertificateDer<'static>> =
        rustls_pemfile::certs(&mut BufReader::new(DEV_CERT_PEM.as_bytes()))
            .collect::<Result<Vec<_>, _>>()?;

    let key = rustls_pemfile::private_key(&mut BufReader::new(DEV_KEY_PEM.as_bytes()))?
        .ok_or("embedded dev key is invalid")?;

    Ok((certs, key, "embedded dev certificate"))
}
