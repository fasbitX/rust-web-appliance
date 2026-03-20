// ═══════════════════════════════════════════════════════════════════
// HTTPS Server — TLS + HTTP with thread-per-connection workers
// ═══════════════════════════════════════════════════════════════════

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{Arc, RwLock};
use std::thread;

use rustls::ServerConnection;
use rustls::StreamOwned;

use crate::admin::AdminState;
use crate::api::{self, ConfigEngine, Route};
use crate::http::HttpRequest;
use crate::router;
use crate::security::SecurityConfig;
use crate::storage::Storage;

const NUM_WORKERS: usize = 4;

/// Wrapper around StreamOwned that sends TLS close_notify on drop.
/// Without this, smoltcp may RST the TCP connection before the
/// response data reaches the client.
struct TlsWriter {
    stream: Option<StreamOwned<ServerConnection, TcpStream>>,
}

impl TlsWriter {
    fn new(stream: StreamOwned<ServerConnection, TcpStream>) -> Self {
        TlsWriter {
            stream: Some(stream),
        }
    }
}

impl Read for TlsWriter {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stream.as_mut().unwrap().read(buf)
    }
}

impl Write for TlsWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.as_mut().unwrap().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.as_mut().unwrap().flush()
    }
}

impl Drop for TlsWriter {
    fn drop(&mut self) {
        if let Some(mut stream) = self.stream.take() {
            // Send TLS close_notify alert
            stream.conn.send_close_notify();
            // Flush the close_notify to the TCP socket
            let _ = stream.conn.complete_io(&mut stream.sock);
            // Graceful TCP shutdown (FIN, not RST)
            let _ = stream.sock.shutdown(Shutdown::Write);
        }
    }
}

pub fn run(
    bind_addr: &str,
    tls_config: Arc<RwLock<Arc<rustls::ServerConfig>>>,
    storage: &'static Storage,
    security: &'static SecurityConfig,
    admin_state: &'static AdminState,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(bind_addr)
        .map_err(|e| format!("Failed to bind {}: {}", bind_addr, e))?;

    println!("[https] Listening on https://{}", bind_addr);
    println!("[https] Spawning {} worker threads", NUM_WORKERS);

    // Tier 1: Compiled Rust routes
    let routes: Arc<Vec<Route>> = Arc::new(api::routes());
    println!("[https] {} compiled Rust routes registered", routes.len());

    // Tier 2: Config-driven engine
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
                let (mut tcp_stream, peer_addr) = match listener.accept() {
                    Ok(conn) => conn,
                    Err(e) => {
                        println!("[https] Worker {} accept error: {}", worker_id, e);
                        continue;
                    }
                };

                // Increment request counter
                admin_state.increment_requests();

                // Read current TLS config (supports hot-reload)
                let current_tls = tls_config.read().unwrap().clone();

                // Create TLS server connection
                let mut conn = match ServerConnection::new(current_tls) {
                    Ok(c) => c,
                    Err(e) => {
                        println!("[https] Worker {} TLS setup error: {}", worker_id, e);
                        continue;
                    }
                };

                // Explicit TLS handshake
                let handshake_ok = loop {
                    if !conn.is_handshaking() {
                        break true;
                    }
                    match conn.complete_io(&mut tcp_stream) {
                        Ok((rd, wr)) => {
                            if rd == 0 && wr == 0 {
                                break false;
                            }
                        }
                        Err(e) => {
                            println!(
                                "[https] Worker {} TLS handshake failed for {}: {}",
                                worker_id, peer_addr, e
                            );
                            break false;
                        }
                    }
                };

                if !handshake_ok {
                    continue;
                }

                // Wrap in TlsWriter (handles clean shutdown on drop)
                let tls_stream = StreamOwned::new(conn, tcp_stream);
                let mut tls_writer = TlsWriter::new(tls_stream);

                // Parse HTTP request
                let request = match HttpRequest::parse(&mut tls_writer) {
                    Ok(req) => req,
                    Err(e) => {
                        println!(
                            "[https] Worker {} parse error from {}: {}",
                            worker_id, peer_addr, e
                        );
                        let _ = crate::http::write_response(
                            &mut tls_writer,
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
                let writer: Box<dyn Write + Send> = Box::new(tls_writer);
                router::handle_request(
                    request,
                    writer,
                    &routes,
                    &config_engine,
                    storage,
                    security,
                    admin_state,
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
