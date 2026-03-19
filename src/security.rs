// ═══════════════════════════════════════════════════════════════════
// Security — API key authentication and role-based access control
// ═══════════════════════════════════════════════════════════════════
//
// Configuration loaded from storage (key: security__config) or
// defaults to a development key printed to serial console.
//
// Supported auth methods:
//   - X-API-Key: <key>
//   - Authorization: Bearer <key>
//
// Roles:
//   - admin: full access (GET, POST, PUT, DELETE, PATCH)
//   - read:  read-only access (GET only)
//
// Public endpoints (no auth required):
//   - GET /api/health
// ═══════════════════════════════════════════════════════════════════

use serde::Deserialize;

use crate::http::HttpRequest;
use crate::storage::Storage;

const SECURITY_CONFIG_KEY: &str = "security__config";
const DEFAULT_DEV_KEY: &str = "rwa_dev_default_key_CHANGE_ME";

#[derive(Deserialize, Clone)]
pub struct SecurityConfig {
    pub api_keys: Vec<ApiKey>,
    pub public_endpoints: Vec<String>,
}

#[derive(Deserialize, Clone)]
pub struct ApiKey {
    pub key: String,
    pub name: String,
    pub role: String,
}

pub enum AuthResult {
    Allowed,
    Denied(u16, String),
}

impl SecurityConfig {
    /// Load security config from storage or create defaults.
    pub fn load(storage: &Storage) -> Self {
        // Try loading from storage
        if let Some(raw) = storage.get(SECURITY_CONFIG_KEY) {
            if let Ok(config) = serde_json::from_str::<SecurityConfig>(&raw) {
                println!(
                    "[security] Loaded {} API key(s) from storage",
                    config.api_keys.len()
                );
                for key in &config.api_keys {
                    println!("[security]   - {} (role: {})", key.name, key.role);
                }
                println!(
                    "[security] {} public endpoint(s)",
                    config.public_endpoints.len()
                );
                return config;
            }
        }

        // Create default config
        let config = SecurityConfig {
            api_keys: vec![ApiKey {
                key: DEFAULT_DEV_KEY.to_string(),
                name: "dev-admin".to_string(),
                role: "admin".to_string(),
            }],
            public_endpoints: vec!["/api/health".to_string()],
        };

        // Save default config to storage for visibility
        if let Ok(json) = serde_json::to_string_pretty(&serde_json::json!({
            "api_keys": [
                {"key": DEFAULT_DEV_KEY, "name": "dev-admin", "role": "admin"}
            ],
            "public_endpoints": ["/api/health"]
        })) {
            let _ = storage.set(SECURITY_CONFIG_KEY, &json);
        }

        println!("[security] +--------------------------------------------------+");
        println!("[security] |  No security config found — using dev defaults    |");
        println!("[security] |                                                   |");
        println!("[security] |  Default API Key:                                 |");
        println!(
            "[security] |    {}    |",
            DEFAULT_DEV_KEY
        );
        println!("[security] |                                                   |");
        println!("[security] |  CHANGE THIS KEY FOR PRODUCTION!                  |");
        println!("[security] |  Update via PUT /api/kv/security__config          |");
        println!("[security] +--------------------------------------------------+");

        config
    }

    /// Check if a request is authorized.
    pub fn check(&self, request: &HttpRequest) -> AuthResult {
        let path = request.url.split('?').next().unwrap_or(&request.url);

        // Check if endpoint is public
        for public in &self.public_endpoints {
            if path == public {
                return AuthResult::Allowed;
            }
        }

        // Extract API key from headers
        let api_key = request.header("X-API-Key").or_else(|| {
            request.header("Authorization").and_then(|auth| {
                auth.strip_prefix("Bearer ")
                    .or_else(|| auth.strip_prefix("bearer "))
            })
        });

        let api_key = match api_key {
            Some(k) => k.trim(),
            None => {
                return AuthResult::Denied(
                    401,
                    "authentication required".to_string(),
                );
            }
        };

        // Validate key
        match self.api_keys.iter().find(|k| k.key == api_key) {
            None => AuthResult::Denied(401, "invalid API key".to_string()),
            Some(key) => {
                // Role-based access: read role can only do GET
                if key.role == "read" && request.method != "GET" {
                    AuthResult::Denied(
                        403,
                        format!(
                            "key '{}' has read-only access",
                            key.name
                        ),
                    )
                } else {
                    AuthResult::Allowed
                }
            }
        }
    }
}
