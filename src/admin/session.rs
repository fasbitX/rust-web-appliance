// ═══════════════════════════════════════════════════════════════════
// Admin Session Store — token management with expiry
// ═══════════════════════════════════════════════════════════════════

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::Rng;

const SESSION_TTL_SECS: u64 = 3600; // 1 hour
const MAX_SESSIONS: usize = 16;

struct Session {
    expires_at: u64,
}

pub struct SessionStore {
    sessions: Mutex<HashMap<String, Session>>,
}

impl SessionStore {
    pub fn new() -> Self {
        SessionStore {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Create a new session, return the token.
    pub fn create(&self) -> String {
        let token = generate_token();
        let now = now_secs();

        if let Ok(mut sessions) = self.sessions.lock() {
            // Purge expired sessions
            sessions.retain(|_, s| s.expires_at > now);

            // If still at capacity, remove oldest
            if sessions.len() >= MAX_SESSIONS {
                if let Some(oldest_key) = sessions
                    .iter()
                    .min_by_key(|(_, s)| s.expires_at)
                    .map(|(k, _)| k.clone())
                {
                    sessions.remove(&oldest_key);
                }
            }

            sessions.insert(
                token.clone(),
                Session {
                    expires_at: now + SESSION_TTL_SECS,
                },
            );
        }

        token
    }

    /// Validate a session token. Returns true if valid and not expired.
    pub fn validate(&self, token: &str) -> bool {
        let now = now_secs();
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(session) = sessions.get(token) {
                if session.expires_at > now {
                    return true;
                }
                // Expired — remove it
                sessions.remove(token);
            }
        }
        false
    }

    /// Revoke a specific session.
    pub fn revoke(&self, token: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.remove(token);
        }
    }

    /// Revoke all sessions.
    pub fn revoke_all(&self) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.clear();
        }
    }

    /// Number of active (non-expired) sessions.
    pub fn active_count(&self) -> usize {
        let now = now_secs();
        self.sessions
            .lock()
            .map(|s| s.values().filter(|s| s.expires_at > now).count())
            .unwrap_or(0)
    }
}

fn generate_token() -> String {
    let bytes: [u8; 32] = rand::thread_rng().r#gen();
    hex_encode(&bytes)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("odd length hex string".to_string());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}
