// ═══════════════════════════════════════════════════════════════════
// HTTPS Server — TLS + HTTP with thread-per-connection workers
// ═══════════════════════════════════════════════════════════════════
//
// Accepts TCP connections, performs TLS handshake via rustls with
// pure-Rust crypto, parses HTTP/1.1 requests, then dispatches
// through the authentication + three-tier routing pipeline.
// ═══════════════════════════════════════════════════════════════════

use std::net::TcpListener;
use std::sync::Arc;
use std::thread;

use rustls::ServerConnection;
use rustls::StreamOwned;

use crate::api::{self, ConfigEngine, Route};
use crate::http::HttpRequest;
use crate::router;
use crate::security::SecurityConfig;
use crate::storage::Storage;

const NUM_WORKERS: usize = 4;

pub fn run(
    bind_addr: &str,
    tls_config: Arc<rustls::ServerConfig>,
    storage: &'static Storage,
    security: &'static SecurityConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(bind_addr)
        .map_err(|e| format!("Failed to bind {}: {}", bind_addr, e))?;

    println!("[https] Listening on https://{}", bind_addr);
    println!("[https] Spawning {} worker threads", NUM_WORKERS);

    // Tier 1: Compiled Rust routes
    let routes: Arc<Vec<Route>> = Arc::new(api::routes());
    println!("[https] {} compiled Rust routes registered", routes.len());

    // Tier 2: Config-driven engine (reads backend/endpoints.json)
    println!("[https] Loading config-driven API engine...");
    let config_engine: Arc<Option<ConfigEngine>> = Arc::new(ConfigEngine::load());
    if config_engine.is_some() {
        println!("[https] Config engine active");
    } else {
        println!("[https] Config engine inactive (no collections)");
    }

    let listener = Arc::new(listener);
    let mut handles = Vec::new();

    for worker_id in 0..NUM_WORKERS {
        let listener = Arc::clone(&listener);
        let tls_config = Arc::clone(&tls_config);
        let routes = Arc::clone(&routes);
        let config_engine = Arc::clone(&config_engine);

        let handle = thread::spawn(move || {
            println!("[https] Worker {} started", worker_id);
            loop {
                // Accept TCP connection
                let (tcp_stream, peer_addr) = match listener.accept() {
                    Ok(conn) => conn,
                    Err(e) => {
                        eprintln!("[https] Worker {} accept error: {}", worker_id, e);
                        continue;
                    }
                };

                // TLS handshake
                let conn = match ServerConnection::new(Arc::clone(&tls_config)) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("[https] Worker {} TLS setup error: {}", worker_id, e);
                        continue;
                    }
                };

                let mut tls_stream = StreamOwned::new(conn, tcp_stream);

                // Parse HTTP request over TLS
                let request = match HttpRequest::parse(&mut tls_stream) {
                    Ok(req) => req,
                    Err(e) => {
                        eprintln!(
                            "[https] Worker {} parse error from {}: {}",
                            worker_id, peer_addr, e
                        );
                        let _ = crate::http::write_response(
                            &mut tls_stream,
                            400,
                            "application/json",
                            br#"{"error":"bad request"}"#,
                        );
                        continue;
                    }
                };

                println!(
                    "[https] {} {} {} {}",
                    worker_id, request.method, request.url, peer_addr
                );

                // Dispatch through auth + three-tier router
                let writer: Box<dyn std::io::Write + Send> = Box::new(tls_stream);
                router::handle_request(
                    request,
                    writer,
                    &routes,
                    &config_engine,
                    storage,
                    security,
                );
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    Ok(())
}
