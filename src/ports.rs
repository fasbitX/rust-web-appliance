// ═══════════════════════════════════════════════════════════════════
// Port Configuration — multi-port listener settings
// ═══════════════════════════════════════════════════════════════════
//
// Default ports (aligned with Cloudflare proxied HTTPS ports):
//   80   — HTTP redirect to HTTPS (default: redirect)
//   443  — Primary HTTPS traffic  (default: on)
//   8443 — API / mobile app HTTPS (default: off)
//
// Configuration stored in /data/ports.json (VirtioFS).
// Falls back to defaults when file is absent.
// Changes require restart to take effect.
// ═══════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};

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

impl PortConfig {
    /// Load port configuration from /data/ports.json or use defaults.
    pub fn load() -> Self {
        match std::fs::read_to_string("/data/ports.json") {
            Ok(contents) => match serde_json::from_str(&contents) {
                Ok(config) => {
                    println!("[ports] Loaded configuration from /data/ports.json");
                    config
                }
                Err(e) => {
                    println!("[ports] Failed to parse /data/ports.json: {}", e);
                    println!("[ports] Using default configuration");
                    PortConfig::default()
                }
            },
            Err(_) => {
                println!("[ports] No /data/ports.json found, using defaults");
                PortConfig::default()
            }
        }
    }

    /// Save port configuration to /data/ports.json.
    pub fn save(&self) -> Result<(), String> {
        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("serialize: {}", e))?;
        std::fs::write("/data/ports.json", json.as_bytes())
            .map_err(|e| format!("write /data/ports.json: {}", e))?;
        Ok(())
    }
}
