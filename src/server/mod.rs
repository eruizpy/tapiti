use crate::dtc;
use crate::logger::SqliteStore;
use crate::obd::Reading;
use crate::transport::TcpTransport;
use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tracing::{info, warn};

#[cfg(test)]
mod ws_tests;

pub struct HttpServer {
    addr: String,
    tx: broadcast::Sender<Reading>,
    store: SqliteStore,
    transport: Arc<Mutex<TcpTransport>>,
}

impl HttpServer {
    pub fn new(
        addr: &str,
        tx: broadcast::Sender<Reading>,
        store: SqliteStore,
        transport: Arc<Mutex<TcpTransport>>,
    ) -> Self {
        Self {
            addr: addr.to_string(),
            tx,
            store,
            transport,
        }
    }

    pub async fn run(self) -> Result<()> {
        let listener = TcpListener::bind(&self.addr).await?;
        info!("tapiti escuchando en {}", self.addr);
        let tx = Arc::new(self.tx);
        let store = Arc::new(self.store);
        let transport = self.transport;

        loop {
            let (stream, peer) = listener.accept().await?;
            let tx2 = Arc::clone(&tx);
            let store2 = Arc::clone(&store);
            let transport2 = Arc::clone(&transport);
            tokio::spawn(async move {
                if let Err(e) = handle(stream, (*tx2).clone(), store2, transport2).await {
                    warn!("conn {}: {}", peer, e);
                }
            });
        }
    }
}

async fn handle(
    mut stream: TcpStream,
    tx: broadcast::Sender<Reading>,
    store: Arc<SqliteStore>,
    transport: Arc<Mutex<TcpTransport>>,
) -> Result<()> {
    let mut buf = vec![0u8; 2048];
    let n = stream.read(&mut buf).await?;
    let req = String::from_utf8_lossy(&buf[..n]).into_owned();

    if req.contains("Upgrade: websocket") {
        handle_websocket(&mut stream, &req, tx).await
    } else if req.starts_with("GET / ") || req.starts_with("GET /index.html ") {
        serve_ui(&mut stream).await
    } else if req.starts_with("GET /export") {
        handle_export(&mut stream, &req, &store).await
    } else if req.starts_with("GET /dtc ") {
        handle_dtc_read(&mut stream, transport).await
    } else if req.starts_with("POST /dtc/clear ") {
        handle_dtc_clear(&mut stream, transport).await
    } else {
        serve_status(&mut stream).await
    }
}

// ── WebSocket ────────────────────────────────────────────────────────────────

async fn handle_websocket(
    stream: &mut TcpStream,
    req: &str,
    tx: broadcast::Sender<Reading>,
) -> Result<()> {
    if !ws_origin_allowed(req) {
        stream
            .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
            .await?;
        return Ok(());
    }

    let key = extract_ws_key(req).unwrap_or_default();
    let accept = ws_accept_key(&key);
    stream
        .write_all(
            format!(
                "HTTP/1.1 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: {}\r\n\r\n",
                accept
            )
            .as_bytes(),
        )
        .await?;

    let mut rx = tx.subscribe();
    loop {
        match rx.recv().await {
            Ok(reading) => {
                let json = serde_json::to_string(&reading)?;
                let frame = ws_text_frame(json.as_bytes());
                if stream.write_all(&frame).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => warn!("WS lagged {}", n),
            Err(_) => break,
        }
    }
    Ok(())
}

/// Origin válido para WebSocket upgrade:
///   - Sin header Origin (cliente nativo / herramienta de debug local)
///   - "null"                → WebView Android con file:// o data: URI
///   - "http://127.0.0.1"   → WebView con URL cargada desde el servidor
///   - "http://localhost"    → desarrollo local en PC
///
/// Cualquier otro valor de Origin → rechazar.
fn ws_origin_allowed(req: &str) -> bool {
    let origin_line = req
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("origin:"));

    match origin_line {
        None => true, // sin header Origin — cliente nativo, permitir
        Some(line) => {
            let value = line["origin:".len()..].trim();
            matches!(value, "null" | "http://127.0.0.1" | "http://localhost")
        }
    }
}

// ── Rutas HTTP ───────────────────────────────────────────────────────────────

async fn serve_ui(stream: &mut TcpStream) -> Result<()> {
    let html = include_str!("../../android/app/src/main/assets/ui/index.html");
    stream
        .write_all(
            format!(
                "HTTP/1.1 200 OK\r\n\
             Content-Type: text/html; charset=utf-8\r\n\
             Cache-Control: no-store\r\n\
             Content-Length: {}\r\n\r\n{}",
                html.len(),
                html
            )
            .as_bytes(),
        )
        .await?;
    Ok(())
}

async fn serve_status(stream: &mut TcpStream) -> Result<()> {
    let body = r#"{"status":"tapiti running","version":"0.1.0"}"#;
    json_response(stream, 200, body).await
}

/// GET /export?session=20240101_120000
/// Devuelve el CSV de la sesión indicada.
/// Si no se especifica session, devuelve la más reciente.
async fn handle_export(stream: &mut TcpStream, req: &str, store: &SqliteStore) -> Result<()> {
    let session = match extract_query_param(req, "session") {
        Some(s) => s,
        None => match store.latest_session().await {
            Ok(Some(s)) => s,
            Ok(None) => {
                return json_response(stream, 404, r#"{"error":"no sessions recorded"}"#).await;
            }
            Err(e) => {
                let body = format!(r#"{{"error":"{}"}}"#, e);
                return json_response(stream, 500, &body).await;
            }
        },
    };

    match store.export_csv(&session).await {
        Ok(csv) => {
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\n\
                     Content-Type: text/csv; charset=utf-8\r\n\
                     Content-Disposition: attachment; filename=\"tapiti_{}.csv\"\r\n\
                     Content-Length: {}\r\n\r\n{}",
                        session,
                        csv.len(),
                        csv
                    )
                    .as_bytes(),
                )
                .await?;
        }
        Err(e) => {
            let body = format!(r#"{{"error":"{}"}}"#, e);
            json_response(stream, 500, &body).await?;
        }
    }
    Ok(())
}

/// GET /dtc — lee los códigos de falla activos de la ECU.
async fn handle_dtc_read(
    stream: &mut TcpStream,
    transport: Arc<Mutex<TcpTransport>>,
) -> Result<()> {
    let mut t = transport.lock().await;
    match dtc::read_dtcs(&mut t).await {
        Ok(codes) => {
            let body = serde_json::to_string(&codes)?;
            json_response(stream, 200, &body).await?;
        }
        Err(e) => {
            let body = format!(r#"{{"error":"{}"}}"#, e);
            json_response(stream, 502, &body).await?;
        }
    }
    Ok(())
}

/// POST /dtc/clear — borra los DTCs (modo 04).
/// La confirmación viene desde la UI — este endpoint solo ejecuta.
async fn handle_dtc_clear(
    stream: &mut TcpStream,
    transport: Arc<Mutex<TcpTransport>>,
) -> Result<()> {
    let mut t = transport.lock().await;
    match dtc::clear_dtcs(&mut t).await {
        Ok(()) => json_response(stream, 200, r#"{"cleared":true}"#).await?,
        Err(e) => {
            let body = format!(r#"{{"error":"{}"}}"#, e);
            json_response(stream, 502, &body).await?;
        }
    }
    Ok(())
}

async fn json_response(stream: &mut TcpStream, status: u16, body: &str) -> Result<()> {
    let reason = match status {
        200 => "OK",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        _ => "Unknown",
    };
    stream
        .write_all(
            format!(
                "HTTP/1.1 {} {}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\r\n{}",
                status,
                reason,
                body.len(),
                body
            )
            .as_bytes(),
        )
        .await?;
    Ok(())
}

// ── Helpers HTTP ─────────────────────────────────────────────────────────────

fn extract_query_param(req: &str, key: &str) -> Option<String> {
    let first_line = req.lines().next()?;
    let path = first_line.split_whitespace().nth(1)?;
    let query = path.split_once('?')?.1;
    query
        .split('&')
        .find(|p| p.starts_with(key) && p[key.len()..].starts_with('='))
        .map(|p| p[key.len() + 1..].to_string())
}

fn extract_ws_key(req: &str) -> Option<String> {
    req.lines()
        .find(|l| l.to_ascii_lowercase().starts_with("sec-websocket-key:"))
        .map(|l| {
            let prefix_len = "sec-websocket-key:".len();
            l[prefix_len..].trim().to_string()
        })
}

// ── WebSocket framing ────────────────────────────────────────────────────────

/// RFC 6455 §4.2.2 — ACCEPT = base64(SHA1(key + GUID))
fn ws_accept_key(key: &str) -> String {
    const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let input = format!("{}{}", key, GUID);
    let hash = sha1(input.as_bytes());
    base64_encode(&hash)
}

/// SHA-1 — FIPS 180-4, sin dependencias externas.
/// Solo para el handshake WebSocket (no material criptográfico sensible).
fn sha1(msg: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];

    let bit_len = (msg.len() as u64).wrapping_mul(8);
    let mut padded = msg.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0x00);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for block in padded.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);

        #[allow(clippy::needless_range_loop)]
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDCu32),
                _ => (b ^ c ^ d, 0xCA62C1D6u32),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// Base64 estándar (RFC 4648).
fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[(n >> 18) as usize & 0x3F] as char);
        out.push(CHARS[(n >> 12) as usize & 0x3F] as char);
        out.push(if chunk.len() > 1 {
            CHARS[(n >> 6) as usize & 0x3F] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            CHARS[n as usize & 0x3F] as char
        } else {
            '='
        });
    }
    out
}

fn ws_text_frame(payload: &[u8]) -> Vec<u8> {
    let len = payload.len();
    let mut frame = Vec::with_capacity(len + 10);
    frame.push(0x81);
    if len < 126 {
        frame.push(len as u8);
    } else if len < 65536 {
        frame.push(126);
        frame.push((len >> 8) as u8);
        frame.push((len & 0xFF) as u8);
    } else {
        frame.push(127);
        for i in (0..8).rev() {
            frame.push((len >> (i * 8)) as u8);
        }
    }
    frame.extend_from_slice(payload);
    frame
}
