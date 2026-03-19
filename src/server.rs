// ═══════════════════════════════════════════════════════════════════
// HTTP Server — tiny_http with thread-per-connection
// ═══════════════════════════════════════════════════════════════════

use std::sync::Arc;
use std::thread;
use tiny_http::Server;

use crate::api::{self, ConfigEngine, Route};
use crate::router;
use crate::storage::Storage;

const NUM_WORKERS: usize = 4;

pub fn run(bind_addr: &str, storage: &'static Storage) -> Result<(), Box<dyn std::error::Error>> {
    let server = Server::http(bind_addr)
        .map_err(|e| format!("Failed to bind {}: {}", bind_addr, e))?;

    println!("[http] Listening on http://{}", bind_addr);
    println!("[http] Spawning {} worker threads", NUM_WORKERS);

    // Tier 1: Compiled Rust routes
    let routes: Arc<Vec<Route>> = Arc::new(api::routes());
    println!("[http] {} compiled Rust routes registered", routes.len());

    // Tier 2: Config-driven engine (reads backend/endpoints.json)
    println!("[http] Loading config-driven API engine...");
    let config_engine: Arc<Option<ConfigEngine>> = Arc::new(ConfigEngine::load());
    if config_engine.is_some() {
        println!("[http] Config engine active");
    } else {
        println!("[http] Config engine inactive (no collections)");
    }

    let server = Arc::new(server);

    let mut handles = Vec::new();

    for worker_id in 0..NUM_WORKERS {
        let server = Arc::clone(&server);
        let routes = Arc::clone(&routes);
        let config_engine = Arc::clone(&config_engine);

        let handle = thread::spawn(move || {
            println!("[http] Worker {} started", worker_id);
            loop {
                let request = match server.recv() {
                    Ok(rq) => rq,
                    Err(e) => {
                        eprintln!("[http] Worker {} recv error: {}", worker_id, e);
                        continue;
                    }
                };

                let method = request.method().to_string();
                let url = request.url().to_string();
                println!("[http] {} {} {}", worker_id, method, url);

                router::handle_request(request, &routes, &config_engine, storage);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    Ok(())
}
