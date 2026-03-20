// ═══════════════════════════════════════════════════════════════════
// Multi-Port Server — HTTP redirect + HTTPS with thread-per-connection
// ═══════════════════════════════════════════════════════════════════
//
// Ports:
//   80   — HTTP redirect to HTTPS (configurable: redirect/off)
//   443  — Primary HTTPS (always on)
//   8443 — API / mobile app HTTPS (configurable: on/off)
//
// Each HTTPS port gets NUM_WORKERS threads sharing a TcpListener.
// Port 80 gets a single thread for lightweight 301 redirects.
// ═══════════════════════════════════════════════════════════════════

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{Arc, RwLock};
use std::thread;

use rustls::ServerConnection;
use rustls::StreamOwned;

use crate::admin::AdminState;
use crate::api::{self, ConfigEngine, Route};
use crate::http::{self, HttpRequest};
use crate::ports::PortConfig;
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
    port_config: &PortConfig,
    tls_config: Arc<RwLock<Arc<rustls::ServerConfig>>>,
    storage: &'static Storage,
    security: &'static SecurityConfig,
    admin_state: &'static AdminState,
) -> Result<(), Box<dyn std::error::Error>> {
    // Tier 1: Compiled Rust routes
    let routes: Arc<Vec<Route>> = Arc::new(api::routes());
    println!("[server] {} compiled Rust routes registered", routes.len());

    // Tier 2: Config-driven engine
    println!("[server] Loading config-driven API engine...");
    let config_engine: Arc<Option<ConfigEngine>> = Arc::new(ConfigEngine::load());
    if config_engine.is_some() {
        println!("[server] Config engine active");
    } else {
        println!("[server] Config engine inactive (no collections)");
    }

    let mut handles = Vec::new();

    // ── Port 80: HTTP redirect ────────────────────────────────────
    if port_config.http.mode == "redirect" {
        let http_port = port_config.http.port;
        let https_port = port_config.https.port;
        let vhost = port_config.vhost.clone();

        let handle = thread::spawn(move || {
            run_http_redirect(http_port, https_port, &vhost);
        });
        handles.push(handle);
    } else {
        println!("[http] Port 80 redirect is off");
    }

    // ── Port 443: Primary HTTPS ───────────────────────────────────
    if port_config.https.enabled {
        let bind_addr = format!("0.0.0.0:{}", port_config.https.port);
        let listener = TcpListener::bind(&bind_addr)
            .map_err(|e| format!("Failed to bind {}: {}", bind_addr, e))?;

        println!(
            "[https] Listening on https://{}  (primary HTTPS)",
            bind_addr
        );
        println!("[https] Spawning {} worker threads", NUM_WORKERS);

        let listener = Arc::new(listener);
        for worker_id in 0..NUM_WORKERS {
            let h = spawn_https_worker(
                worker_id,
                "https",
                Arc::clone(&listener),
                Arc::clone(&tls_config),
                Arc::clone(&routes),
                Arc::clone(&config_engine),
                storage,
                security,
                admin_state,
            );
            handles.push(h);
        }
    }

    // ── Port 8443: API / Mobile HTTPS ─────────────────────────────
    if port_config.api.enabled {
        let bind_addr = format!("0.0.0.0:{}", port_config.api.port);
        let listener = TcpListener::bind(&bind_addr)
            .map_err(|e| format!("Failed to bind {}: {}", bind_addr, e))?;

        println!(
            "[api] Listening on https://{}  (API / mobile)",
            bind_addr
        );
        println!("[api] Spawning {} worker threads", NUM_WORKERS);

        let listener = Arc::new(listener);
        for worker_id in 0..NUM_WORKERS {
            let h = spawn_https_worker(
                worker_id,
                "api",
                Arc::clone(&listener),
                Arc::clone(&tls_config),
                Arc::clone(&routes),
                Arc::clone(&config_engine),
                storage,
                security,
                admin_state,
            );
            handles.push(h);
        }
    } else {
        println!("[api] Port 8443 is off");
    }

    println!();
    println!("[server] All listeners started");

    for handle in handles {
        let _ = handle.join();
    }

    Ok(())
}

/// Spawn an HTTPS worker thread that accepts TLS connections on the
/// given listener and dispatches through the three-tier router.
fn spawn_https_worker(
    worker_id: usize,
    tag: &'static str,
    listener: Arc<TcpListener>,
    tls_config: Arc<RwLock<Arc<rustls::ServerConfig>>>,
    routes: Arc<Vec<Route>>,
    config_engine: Arc<Option<ConfigEngine>>,
    storage: &'static Storage,
    security: &'static SecurityConfig,
    admin_state: &'static AdminState,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        println!("[{}] Worker {} started", tag, worker_id);
        loop {
            // Accept TCP connection
            let (mut tcp_stream, peer_addr) = match listener.accept() {
                Ok(conn) => conn,
                Err(e) => {
                    println!("[{}] Worker {} accept error: {}", tag, worker_id, e);
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
                    println!("[{}] Worker {} TLS setup error: {}", tag, worker_id, e);
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
                            "[{}] Worker {} TLS handshake failed for {}: {}",
                            tag, worker_id, peer_addr, e
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
                        "[{}] Worker {} parse error from {}: {}",
                        tag, worker_id, peer_addr, e
                    );
                    let _ = http::write_response(
                        &mut tls_writer,
                        400,
                        "application/json",
                        br#"{"error":"bad request"}"#,
                    );
                    continue;
                }
            };

            println!(
                "[{}] {} {} {} {}",
                tag, worker_id, request.method, request.url, peer_addr
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
    })
}

/// HTTP redirect listener — sends 301 to HTTPS for all requests.
/// Runs on a single thread (redirects are lightweight).
fn run_http_redirect(http_port: u16, https_port: u16, vhost: &str) {
    let bind_addr = format!("0.0.0.0:{}", http_port);
    let listener = match TcpListener::bind(&bind_addr) {
        Ok(l) => l,
        Err(e) => {
            println!("[http] Failed to bind {}: {}", bind_addr, e);
            return;
        }
    };

    println!(
        "[http] Redirect listener on http://{} -> HTTPS :{}",
        bind_addr, https_port
    );

    loop {
        let (mut stream, peer_addr) = match listener.accept() {
            Ok(conn) => conn,
            Err(e) => {
                println!("[http] Accept error: {}", e);
                continue;
            }
        };

        // Parse the HTTP request to get the Host header and path
        let request = match HttpRequest::parse(&mut stream) {
            Ok(req) => req,
            Err(_) => {
                let _ = stream.shutdown(Shutdown::Both);
                continue;
            }
        };

        // Determine redirect host
        let host = if !vhost.is_empty() {
            vhost.to_string()
        } else {
            request
                .header("host")
                .map(|h| h.split(':').next().unwrap_or(h).to_string())
                .unwrap_or_else(|| "localhost".to_string())
        };

        // Build redirect URL (omit port if standard 443)
        let redirect_url = if https_port == 443 {
            format!("https://{}{}", host, request.url)
        } else {
            format!("https://{}:{}{}", host, https_port, request.url)
        };

        println!("[http] {} -> {}", peer_addr, redirect_url);

        let body = format!(
            "<html><body><a href=\"{}\">Moved permanently</a></body></html>",
            redirect_url
        );
        let response = format!(
            "HTTP/1.1 301 Moved Permanently\r\nLocation: {}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\nServer: RustWebAppliance\r\n\r\n{}",
            redirect_url,
            body.len(),
            body
        );

        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
        let _ = stream.shutdown(Shutdown::Both);
    }
}
