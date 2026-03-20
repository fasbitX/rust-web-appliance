// ═══════════════════════════════════════════════════════════════════
// Storage — JSON flat-file key-value store via VirtioFS
// ═══════════════════════════════════════════════════════════════════
//
// Since redb cannot run on HermitOS (requires mmap) and virtio-blk
// is not implemented in the kernel, we use VirtioFS for persistence.
//
// VirtioFS shares a host directory into the guest. The kernel mounts
// it and exposes it through std::fs. Data is stored as individual
// JSON files: one file per key in a `data/` directory.
//
// When VirtioFS is not available (e.g., basic QEMU testing), the
// storage falls back to an in-memory HashMap.
// ═══════════════════════════════════════════════════════════════════

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

const DATA_DIR: &str = "/data";

enum Backend {
    /// Persistent storage via VirtioFS-backed filesystem
    Filesystem { base: PathBuf },
    /// In-memory fallback when no filesystem is available
    Memory { map: RwLock<HashMap<String, String>> },
}

pub struct Storage {
    backend: Backend,
}

impl Storage {
    pub fn init() -> Result<Self, Box<dyn std::error::Error>> {
        let data_path = PathBuf::from(DATA_DIR);

        // Try to use filesystem storage (VirtioFS)
        if fs::create_dir_all(&data_path).is_ok() {
            // Verify we can actually write
            let test_file = data_path.join(".storage_test");
            if fs::write(&test_file, b"ok").is_ok() {
                let _ = fs::remove_file(&test_file);
                println!("[storage] Using filesystem backend at {}", DATA_DIR);
                return Ok(Storage {
                    backend: Backend::Filesystem { base: data_path },
                });
            }
        }

        // Fall back to in-memory
        println!("[storage] VirtioFS not available, using in-memory backend");
        println!("[storage] WARNING: Data will not persist across reboots!");
        Ok(Storage {
            backend: Backend::Memory {
                map: RwLock::new(HashMap::new()),
            },
        })
    }

    pub fn get(&self, key: &str) -> Option<String> {
        match &self.backend {
            Backend::Filesystem { base } => {
                let path = key_to_path(base, key);
                fs::read_to_string(path).ok()
            }
            Backend::Memory { map } => {
                let map = map.read().ok()?;
                map.get(key).cloned()
            }
        }
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        if !is_valid_key(key) {
            return Err("invalid key: must be alphanumeric, hyphens, underscores".into());
        }

        match &self.backend {
            Backend::Filesystem { base } => {
                let path = key_to_path(base, key);
                fs::write(path, value)?;
            }
            Backend::Memory { map } => {
                let mut map = map.write().map_err(|e| format!("lock: {}", e))?;
                map.insert(key.to_string(), value.to_string());
            }
        }
        Ok(())
    }

    pub fn delete(&self, key: &str) -> Result<bool, Box<dyn std::error::Error>> {
        match &self.backend {
            Backend::Filesystem { base } => {
                let path = key_to_path(base, key);
                if path.exists() {
                    fs::remove_file(path)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Backend::Memory { map } => {
                let mut map = map.write().map_err(|e| format!("lock: {}", e))?;
                Ok(map.remove(key).is_some())
            }
        }
    }

    /// Returns true if storage is backed by VirtioFS (persistent across restarts).
    pub fn is_persistent(&self) -> bool {
        matches!(&self.backend, Backend::Filesystem { .. })
    }

    pub fn list_keys(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        match &self.backend {
            Backend::Filesystem { base } => {
                let mut keys = Vec::new();
                for entry in fs::read_dir(base)? {
                    let entry = entry?;
                    if let Some(name) = entry.file_name().to_str() {
                        if !name.starts_with('.') && name.ends_with(".json") {
                            keys.push(name.trim_end_matches(".json").to_string());
                        }
                    }
                }
                keys.sort();
                Ok(keys)
            }
            Backend::Memory { map } => {
                let map = map.read().map_err(|e| format!("lock: {}", e))?;
                let mut keys: Vec<String> = map.keys().cloned().collect();
                keys.sort();
                Ok(keys)
            }
        }
    }
}

/// Sanitize key and build a filesystem path
fn key_to_path(base: &PathBuf, key: &str) -> PathBuf {
    base.join(format!("{}.json", key))
}

/// Keys must be alphanumeric with hyphens/underscores (no path traversal)
fn is_valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 128
        && key.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}
