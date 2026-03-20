// ═══════════════════════════════════════════════════════════════════
// Admin Auth — Ed25519 challenge-response authentication
// ═══════════════════════════════════════════════════════════════════
//
// Both the admin public key and private key are embedded at build time
// from admin_keys/admin_pub.pem and admin_keys/admin_priv.pem.
//
// Login flow:
//   1. Browser fetches embedded private key via GET /admin/api/auth/key
//   2. POST /admin/api/auth/challenge → server returns random hex nonce
//   3. Browser signs nonce with private key (Web Crypto API Ed25519)
//   4. POST /admin/api/auth/verify → server verifies signature
//   5. Server returns session token on success
//
// Security model: whoever builds the binary controls the keypair.
// Network access to /admin/ is the security boundary.
//
// ═══════════════════════════════════════════════════════════════════

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rand::Rng;

use super::session::hex_encode;

const ADMIN_PUB_PEM: &str = include_str!("../../admin_keys/admin_pub.pem");
const ADMIN_PRIV_PEM: &str = include_str!("../../admin_keys/admin_priv.pem");
const CHALLENGE_TTL_SECS: u64 = 60;
const MAX_PENDING_CHALLENGES: usize = 32;

pub struct AdminAuth {
    verifying_key: VerifyingKey,
    pending: Mutex<HashMap<String, u64>>, // challenge_hex -> expires_at
}

impl AdminAuth {
    pub fn init() -> Self {
        let key_bytes = parse_ed25519_pub_pem(ADMIN_PUB_PEM)
            .expect("Failed to parse admin public key from admin_keys/admin_pub.pem");

        let verifying_key = VerifyingKey::from_bytes(&key_bytes)
            .expect("Invalid Ed25519 public key in admin_keys/admin_pub.pem");

        println!("[admin] Ed25519 keypair loaded from admin_keys/");

        AdminAuth {
            verifying_key,
            pending: Mutex::new(HashMap::new()),
        }
    }

    /// Return the embedded private key PEM (served to the browser for signing).
    pub fn private_key_pem(&self) -> &'static str {
        ADMIN_PRIV_PEM
    }

    /// Generate a new challenge nonce. Returns the hex-encoded nonce.
    pub fn create_challenge(&self) -> String {
        let bytes: [u8; 32] = rand::thread_rng().r#gen();
        let challenge = hex_encode(&bytes);
        let now = now_secs();

        if let Ok(mut pending) = self.pending.lock() {
            // Purge expired challenges
            pending.retain(|_, &mut exp| exp > now);

            // Cap pending challenges
            if pending.len() >= MAX_PENDING_CHALLENGES {
                if let Some(oldest_key) = pending
                    .iter()
                    .min_by_key(|(_, exp)| *exp)
                    .map(|(k, _)| k.clone())
                {
                    pending.remove(&oldest_key);
                }
            }

            pending.insert(challenge.clone(), now + CHALLENGE_TTL_SECS);
        }

        challenge
    }

    /// Verify a signed challenge. Returns true if the signature is valid.
    /// Consumes the challenge (one-time use).
    pub fn verify(&self, challenge_hex: &str, signature_hex: &str) -> Result<(), String> {
        let now = now_secs();

        // Check challenge exists and hasn't expired
        {
            let mut pending = self.pending.lock().map_err(|_| "lock error".to_string())?;
            match pending.remove(challenge_hex) {
                Some(expires_at) if expires_at > now => {} // Valid, consumed
                Some(_) => return Err("challenge expired".to_string()),
                None => return Err("unknown challenge".to_string()),
            }
        }

        // Decode challenge and signature from hex
        let challenge_bytes = super::session::hex_decode(challenge_hex)?;
        let sig_bytes = super::session::hex_decode(signature_hex)?;

        if sig_bytes.len() != 64 {
            return Err(format!("signature must be 64 bytes, got {}", sig_bytes.len()));
        }

        let signature = Signature::from_bytes(
            sig_bytes
                .as_slice()
                .try_into()
                .map_err(|_| "invalid signature length")?,
        );

        // Verify the signature against the embedded public key
        self.verifying_key
            .verify(&challenge_bytes, &signature)
            .map_err(|_| "signature verification failed".to_string())
    }
}

/// Parse an Ed25519 public key from SPKI PEM format.
/// The DER structure for Ed25519 SPKI is:
///   30 2a 30 05 06 03 2b 65 70 03 21 00 <32 bytes>
/// We extract the last 32 bytes.
fn parse_ed25519_pub_pem(pem: &str) -> Result<[u8; 32], String> {
    // Extract base64 content between PEM headers
    let b64: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");

    let der = base64_decode(&b64)?;

    // Ed25519 SPKI DER is exactly 44 bytes: 12-byte header + 32-byte key
    if der.len() != 44 {
        return Err(format!(
            "unexpected DER length {} (expected 44 for Ed25519 SPKI)",
            der.len()
        ));
    }

    // The raw key is at bytes 12..44
    let mut key = [0u8; 32];
    key.copy_from_slice(&der[12..44]);
    Ok(key)
}

/// Minimal base64 decoder (no external crate needed).
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("empty base64 input".to_string());
    }

    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;

    for ch in input.chars() {
        let val = match ch {
            'A'..='Z' => ch as u32 - 'A' as u32,
            'a'..='z' => ch as u32 - 'a' as u32 + 26,
            '0'..='9' => ch as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            '=' => continue,
            '\n' | '\r' | ' ' | '\t' => continue,
            _ => return Err(format!("invalid base64 character: {}", ch)),
        };

        buf = (buf << 6) | val;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            output.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }

    Ok(output)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
