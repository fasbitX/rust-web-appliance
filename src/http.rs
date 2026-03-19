// ═══════════════════════════════════════════════════════════════════
// HTTP — Request parsing and response writing (replaces tiny_http)
// ═══════════════════════════════════════════════════════════════════
//
// Pure-Rust HTTP/1.1 implementation using httparse for parsing.
// Works with any Read+Write stream (plain TCP or TLS).
// ═══════════════════════════════════════════════════════════════════

use std::io::{Read, Write};

const MAX_HEADER_SIZE: usize = 8192;  // 8KB for headers
const MAX_BODY_SIZE: usize = 1_048_576; // 1MB for body
const MAX_HEADERS: usize = 64;

/// A parsed HTTP/1.1 request.
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// Parse an HTTP/1.1 request from any readable stream.
    pub fn parse<R: Read>(reader: &mut R) -> Result<Self, String> {
        let mut buf = vec![0u8; 4096];
        let mut filled = 0;

        // Read until we find the end of headers (\r\n\r\n)
        let header_end;
        loop {
            if filled >= buf.len() {
                if buf.len() >= MAX_HEADER_SIZE {
                    return Err("request headers too large".into());
                }
                buf.resize(buf.len() * 2, 0);
            }

            let n = reader
                .read(&mut buf[filled..])
                .map_err(|e| format!("read: {}", e))?;
            if n == 0 {
                if filled == 0 {
                    return Err("connection closed".into());
                }
                return Err("unexpected EOF in headers".into());
            }
            filled += n;

            if let Some(pos) = find_subsequence(&buf[..filled], b"\r\n\r\n") {
                header_end = pos + 4;
                break;
            }
        }

        // Parse with httparse
        let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
        let mut req = httparse::Request::new(&mut headers);

        match req.parse(&buf[..header_end]) {
            Ok(httparse::Status::Complete(_)) => {}
            Ok(httparse::Status::Partial) => return Err("incomplete request headers".into()),
            Err(e) => return Err(format!("HTTP parse error: {}", e)),
        }

        let method = req.method.ok_or("missing HTTP method")?.to_string();
        let url = req.path.ok_or("missing request path")?.to_string();

        let mut parsed_headers = Vec::new();
        let mut content_length: usize = 0;

        for h in req.headers.iter() {
            let name = h.name.to_string();
            let value = String::from_utf8_lossy(h.value).to_string();
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
            parsed_headers.push((name, value));
        }

        // Read body based on Content-Length
        let mut body = Vec::new();
        if content_length > 0 {
            if content_length > MAX_BODY_SIZE {
                return Err("request body too large".into());
            }

            // Some body bytes may already be buffered after the headers
            let body_in_buf = filled.saturating_sub(header_end);
            if body_in_buf > 0 {
                body.extend_from_slice(&buf[header_end..filled]);
            }

            // Read remaining body bytes from stream
            if body.len() < content_length {
                let remaining = content_length - body.len();
                let mut rest = vec![0u8; remaining];
                reader
                    .read_exact(&mut rest)
                    .map_err(|e| format!("body read: {}", e))?;
                body.extend_from_slice(&rest);
            }

            body.truncate(content_length);
        }

        Ok(HttpRequest {
            method,
            url,
            headers: parsed_headers,
            body,
        })
    }

    /// Case-insensitive header lookup.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Write an HTTP/1.1 response to any writable stream.
pub fn write_response<W: Write + ?Sized>(
    writer: &mut W,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = status_reason(status);

    write!(writer, "HTTP/1.1 {} {}\r\n", status, reason)?;
    write!(writer, "Content-Type: {}\r\n", content_type)?;
    write!(writer, "Content-Length: {}\r\n", body.len())?;
    write!(writer, "Connection: close\r\n")?;
    write!(writer, "Server: RustWebAppliance\r\n")?;
    write!(writer, "\r\n")?;
    writer.write_all(body)?;
    writer.flush()?;

    Ok(())
}

fn status_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "OK",
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
