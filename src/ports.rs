// ═══════════════════════════════════════════════════════════════════
// Port Configuration — multi-port listener settings
// ═══════════════════════════════════════════════════════════════════
//
// Default ports (aligned with Cloudflare proxied HTTPS ports):
//   80   — HTTP redirect to HTTPS (default: redirect)
//   443  — Primary HTTPS traffic  (default: on)
//   8443 — API / mobile app HTTPS (default: off)
//
// Configuration load order:
//   1. Storage KV entry "_ports" (admin UI saves go here)
//   2. /backend/ports.json       (pre-configured file via VirtioFS)
//   3. Built-in defaults
//
// Changes require restart to take effect.
// ═══════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};

use crate::storage::Storage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortConfig {
    /// Virtual host name (e.g., "example.com"). Empty = accept any Host.
    #[serde(default)]
    pub vhost: String,

    /// Port 80 — HTTP
    #[serde(default = "default_http")]
    pub http: HttpPortConfig,

    /// Port 443 — Primary HTTPS
    #[serde(default = "default_https")]
    pub https: HttpsPortConfig,

    /// Port 8443 — API / Mobile HTTPS
    #[serde(default = "default_api")]
    pub api: ApiPortConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpPortConfig {
    #[serde(default = "default_port_80")]
    pub port: u16,
    /// "redirect" (301 to HTTPS) or "off"
    #[serde(default = "default_mode_redirect")]
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpsPortConfig {
    #[serde(default = "default_port_443")]
    pub port: u16,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiPortConfig {
    #[serde(default = "default_port_8443")]
    pub port: u16,
    #[serde(default)]
    pub enabled: bool,
}

fn default_http() -> HttpPortConfig {
    HttpPortConfig {
        port: 80,
        mode: "redirect".to_string(),
    }
}

fn default_https() -> HttpsPortConfig {
    HttpsPortConfig {
        port: 443,
        enabled: true,
    }
}

fn default_api() -> ApiPortConfig {
    ApiPortConfig {
        port: 8443,
        enabled: false,
    }
}

fn default_port_80() -> u16 {
    80
}
fn default_port_443() -> u16 {
    443
}
fn default_port_8443() -> u16 {
    8443
}
fn default_mode_redirect() -> String {
    "redirect".to_string()
}
fn default_true() -> bool {
    true
}

impl Default for PortConfig {
    fn default() -> Self {
        PortConfig {
            vhost: String::new(),
            http: default_http(),
            https: default_https(),
            api: default_api(),
        }
    }
}

const STORAGE_KEY: &str = "_ports";

impl PortConfig {
    /// Load port configuration.
    /// Priority: Storage KV → /backend/ports.json → defaults.
    pub fn load(storage: &Storage) -> Self {
        // 1. Storage KV (admin UI saves)
        if let Some(json) = storage.get(STORAGE_KEY) {
            if let Ok(config) = serde_json::from_str::<PortConfig>(&json) {
                println!("[ports] Loaded configuration from storage");
                return config;
            }
        }

        // 2. Pre-configured file (VirtioFS backend/ directory)
        if let Ok(contents) = std::fs::read_to_string("/backend/ports.json") {
            if let Ok(config) = serde_json::from_str::<PortConfig>(&contents) {
                println!("[ports] Loaded configuration from /backend/ports.json");
                return config;
            }
        }

        // 3. Defaults
        println!("[ports] Using default configuration");
        PortConfig::default()
    }

    /// Save port configuration to storage.
    pub fn save(&self, storage: &Storage) -> Result<(), String> {
        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("serialize: {}", e))?;
        storage
            .set(STORAGE_KEY, &json)
            .map_err(|e| format!("save: {}", e))?;
        Ok(())
    }
}
