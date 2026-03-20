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
//   1. Storage KV entry "_ports" (admin UI saves, persistent with VirtioFS)
//   2. /backend/ports.json       (pre-configured file via VirtioFS)
//   3. Embedded backend/ports.json (compiled into binary — always available)
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

/// backend/ports.json compiled into the binary at build time.
/// Edit this file, rebuild, and your config is baked in — no VirtioFS needed.
const EMBEDDED_CONFIG: &str = include_str!("../backend/ports.json");

impl PortConfig {
    /// Load port configuration.
    /// Priority: Storage KV → VirtioFS /backend/ports.json → embedded (compiled-in).
    pub fn load(storage: &Storage) -> Self {
        // 1. Storage KV (admin UI saves — persistent with VirtioFS)
        if let Some(json) = storage.get(STORAGE_KEY) {
            if let Ok(config) = serde_json::from_str::<PortConfig>(&json) {
                println!("[ports] Loaded configuration from storage");
                return config;
            }
        }

        // 2. VirtioFS file (runtime override without rebuild)
        if let Ok(contents) = std::fs::read_to_string("/backend/ports.json") {
            if let Ok(config) = serde_json::from_str::<PortConfig>(&contents) {
                println!("[ports] Loaded configuration from /backend/ports.json");
                return config;
            }
        }

        // 3. Embedded config (compiled into binary from backend/ports.json)
        match serde_json::from_str::<PortConfig>(EMBEDDED_CONFIG) {
            Ok(config) => {
                println!("[ports] Loaded embedded configuration (compiled-in)");
                config
            }
            Err(_) => {
                println!("[ports] Using hardcoded defaults");
                PortConfig::default()
            }
        }
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
