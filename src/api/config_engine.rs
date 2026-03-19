// ═══════════════════════════════════════════════════════════════════
// Config-Driven API Engine
// ═══════════════════════════════════════════════════════════════════
//
// Reads backend/endpoints.json at boot and auto-generates full
// REST CRUD endpoints for every collection defined. No Rust code
// needed — just edit the JSON, reboot, and your API exists.
//
// Generated endpoints per collection:
//   GET    /api/{collection}        → list all items
//   GET    /api/{collection}/{id}   → get one item
//   POST   /api/{collection}        → create (auto-generates ID)
//   PUT    /api/{collection}/{id}   → update (validates fields)
//   DELETE /api/{collection}/{id}   → delete
//
// Data is stored in the KV layer with namespaced keys:
//   {collection}__{id}       → the item JSON
//   {collection}__index      → JSON array of all IDs
//
// ═══════════════════════════════════════════════════════════════════

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::http::{self, HttpRequest};
use crate::storage::Storage;

// ── Configuration Schema ────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EndpointsConfig {
    pub collections: HashMap<String, CollectionDef>,
}

#[derive(Deserialize)]
pub struct CollectionDef {
    pub fields: HashMap<String, FieldDef>,
}

#[derive(Deserialize)]
pub struct FieldDef {
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub required: bool,
}

// ── Engine ──────────────────────────────────────────────────────────

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct ConfigEngine {
    config: EndpointsConfig,
}

impl ConfigEngine {
    /// Load config from VirtioFS or embedded fallback.
    pub fn load() -> Option<Self> {
        // Try runtime path first (VirtioFS)
        let json = fs::read_to_string("/backend/endpoints.json")
            // Fall back to embedded default
            .unwrap_or_else(|_| include_str!("../../backend/endpoints.json").to_string());

        match serde_json::from_str::<EndpointsConfig>(&json) {
            Ok(config) => {
                let count = config.collections.len();
                if count == 0 {
                    println!("[config-api] No collections defined, engine disabled");
                    return None;
                }
                for (name, col) in &config.collections {
                    let required: Vec<&String> = col
                        .fields
                        .iter()
                        .filter(|(_, f)| f.required)
                        .map(|(k, _)| k)
                        .collect();
                    println!(
                        "[config-api]   /{} ({} fields, {} required)",
                        name,
                        col.fields.len(),
                        required.len()
                    );
                }
                println!(
                    "[config-api] {} collections loaded, 5 endpoints each",
                    count
                );
                Some(ConfigEngine { config })
            }
            Err(e) => {
                eprintln!("[config-api] Failed to parse endpoints.json: {}", e);
                None
            }
        }
    }

    /// List collection names (for /api/collections discovery endpoint).
    #[allow(dead_code)]
    pub fn collection_names(&self) -> Vec<&str> {
        self.config.collections.keys().map(|s| s.as_str()).collect()
    }

    /// Try to handle a request. Returns true if handled, false to pass through.
    pub fn try_handle(
        &self,
        request: &HttpRequest,
        writer: &mut dyn Write,
        storage: &Storage,
    ) -> bool {
        let url = &request.url;
        let method = &request.method;

        // Must start with /api/
        let path = match url.strip_prefix("/api/") {
            Some(p) => p.split('?').next().unwrap_or(p),
            None => return false,
        };

        // Parse: collection_name / optional_id
        let mut parts = path.splitn(2, '/');
        let collection_name = parts.next().unwrap_or("");
        let item_id = parts.next().unwrap_or("");

        // Look up collection
        let collection = match self.config.collections.get(collection_name) {
            Some(c) => c,
            None => return false,
        };

        match (method.as_str(), item_id) {
            ("GET", "") => self.handle_list(writer, storage, collection_name),
            ("GET", id) => self.handle_get(writer, storage, collection_name, id),
            ("POST", "") => {
                self.handle_create(request, writer, storage, collection_name, collection)
            }
            ("PUT", id) => {
                self.handle_update(request, writer, storage, collection_name, collection, id)
            }
            ("DELETE", id) => self.handle_delete(writer, storage, collection_name, id),
            _ => {
                respond_json(writer, 405, r#"{"error":"method not allowed"}"#);
            }
        }

        true
    }

    // ── CRUD Handlers ───────────────────────────────────────────────

    fn handle_list(&self, writer: &mut dyn Write, storage: &Storage, collection: &str) {
        let index_key = format!("{}__index", collection);
        let ids: Vec<String> = match storage.get(&index_key) {
            Some(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            None => Vec::new(),
        };

        let mut items = Vec::new();
        for id in &ids {
            let key = format!("{}__{}", collection, id);
            if let Some(data) = storage.get(&key) {
                items.push(data);
            }
        }

        let body = format!(
            r#"{{"collection":"{}","count":{},"items":[{}]}}"#,
            collection,
            items.len(),
            items.join(",")
        );
        respond_json(writer, 200, &body);
    }

    fn handle_get(&self, writer: &mut dyn Write, storage: &Storage, collection: &str, id: &str) {
        let key = format!("{}__{}", collection, id);
        match storage.get(&key) {
            Some(data) => respond_json(writer, 200, &data),
            None => respond_json(writer, 404, r#"{"error":"not found"}"#),
        }
    }

    fn handle_create(
        &self,
        request: &HttpRequest,
        writer: &mut dyn Write,
        storage: &Storage,
        collection: &str,
        col_def: &CollectionDef,
    ) {
        let body = String::from_utf8_lossy(&request.body).to_string();
        if body.is_empty() {
            respond_json(writer, 400, r#"{"error":"request body required"}"#);
            return;
        }

        // Parse as JSON object
        let mut obj: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(&body)
        {
            Ok(serde_json::Value::Object(m)) => m,
            _ => {
                respond_json(writer, 400, r#"{"error":"body must be a JSON object"}"#);
                return;
            }
        };

        // Validate fields
        if let Some(err) = validate_fields(&obj, col_def) {
            respond_json(writer, 400, &format!(r#"{{"error":"{}"}}"#, err));
            return;
        }

        // Generate ID
        let id = generate_id();
        obj.insert("id".to_string(), serde_json::Value::String(id.clone()));

        // Add timestamp
        if let Ok(ts) = SystemTime::now().duration_since(UNIX_EPOCH) {
            obj.insert(
                "created_at".to_string(),
                serde_json::Value::Number(serde_json::Number::from(ts.as_secs())),
            );
        }

        // Store item
        let item_json = serde_json::to_string(&obj).unwrap_or_default();
        let key = format!("{}__{}", collection, id);
        if let Err(e) = storage.set(&key, &item_json) {
            respond_json(writer, 500, &format!(r#"{{"error":"{}"}}"#, e));
            return;
        }

        // Update index
        update_index(storage, collection, &id, IndexOp::Add);

        respond_json(writer, 201, &item_json);
    }

    fn handle_update(
        &self,
        request: &HttpRequest,
        writer: &mut dyn Write,
        storage: &Storage,
        collection: &str,
        col_def: &CollectionDef,
        id: &str,
    ) {
        let key = format!("{}__{}", collection, id);

        // Check exists
        let existing = match storage.get(&key) {
            Some(data) => data,
            None => {
                respond_json(writer, 404, r#"{"error":"not found"}"#);
                return;
            }
        };

        // Read body
        let body = String::from_utf8_lossy(&request.body).to_string();
        if body.is_empty() {
            respond_json(writer, 400, r#"{"error":"request body required"}"#);
            return;
        }

        // Parse update fields
        let updates: serde_json::Map<String, serde_json::Value> = match serde_json::from_str(&body)
        {
            Ok(serde_json::Value::Object(m)) => m,
            _ => {
                respond_json(writer, 400, r#"{"error":"body must be a JSON object"}"#);
                return;
            }
        };

        // Merge with existing
        let mut obj: serde_json::Map<String, serde_json::Value> =
            match serde_json::from_str(&existing) {
                Ok(serde_json::Value::Object(m)) => m,
                _ => serde_json::Map::new(),
            };

        for (k, v) in updates {
            if k != "id" && k != "created_at" {
                obj.insert(k, v);
            }
        }

        // Validate merged result
        if let Some(err) = validate_fields(&obj, col_def) {
            respond_json(writer, 400, &format!(r#"{{"error":"{}"}}"#, err));
            return;
        }

        // Add updated_at timestamp
        if let Ok(ts) = SystemTime::now().duration_since(UNIX_EPOCH) {
            obj.insert(
                "updated_at".to_string(),
                serde_json::Value::Number(serde_json::Number::from(ts.as_secs())),
            );
        }

        // Store
        let item_json = serde_json::to_string(&obj).unwrap_or_default();
        if let Err(e) = storage.set(&key, &item_json) {
            respond_json(writer, 500, &format!(r#"{{"error":"{}"}}"#, e));
            return;
        }

        respond_json(writer, 200, &item_json);
    }

    fn handle_delete(
        &self,
        writer: &mut dyn Write,
        storage: &Storage,
        collection: &str,
        id: &str,
    ) {
        let key = format!("{}__{}", collection, id);
        match storage.delete(&key) {
            Ok(true) => {
                update_index(storage, collection, id, IndexOp::Remove);
                respond_json(writer, 200, r#"{"status":"deleted"}"#);
            }
            Ok(false) => respond_json(writer, 404, r#"{"error":"not found"}"#),
            Err(e) => respond_json(writer, 500, &format!(r#"{{"error":"{}"}}"#, e)),
        }
    }
}

// ── Field Validation ────────────────────────────────────────────────

fn validate_fields(
    obj: &serde_json::Map<String, serde_json::Value>,
    col_def: &CollectionDef,
) -> Option<String> {
    for (field_name, field_def) in &col_def.fields {
        match obj.get(field_name) {
            None if field_def.required => {
                return Some(format!("missing required field: {}", field_name));
            }
            None => continue,
            Some(value) => {
                let type_ok = match field_def.field_type.as_str() {
                    "string" => value.is_string(),
                    "number" => value.is_number(),
                    "bool" => value.is_boolean(),
                    _ => true,
                };
                if !type_ok {
                    return Some(format!(
                        "field '{}' must be type '{}', got {:?}",
                        field_name, field_def.field_type, value
                    ));
                }
            }
        }
    }
    None
}

// ── Index Management ────────────────────────────────────────────────

enum IndexOp {
    Add,
    Remove,
}

fn update_index(storage: &Storage, collection: &str, id: &str, op: IndexOp) {
    let index_key = format!("{}__index", collection);
    let mut ids: Vec<String> = storage
        .get(&index_key)
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    match op {
        IndexOp::Add => {
            if !ids.contains(&id.to_string()) {
                ids.push(id.to_string());
            }
        }
        IndexOp::Remove => {
            ids.retain(|i| i != id);
        }
    }

    let index_json = serde_json::to_string(&ids).unwrap_or_else(|_| "[]".to_string());
    let _ = storage.set(&index_key, &index_json);
}

// ── ID Generation ───────────────────────────────────────────────────

fn generate_id() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let seq = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}{:04x}", ts, seq)
}

// ── Response Helper ─────────────────────────────────────────────────

fn respond_json(writer: &mut dyn Write, status: u16, body: &str) {
    if let Err(e) = http::write_response(writer, status, "application/json", body.as_bytes()) {
        eprintln!("[config-api] Response error: {}", e);
    }
}
