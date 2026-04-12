use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};
use tracing::{debug, warn};

const INIT_SEQUENCE: &[&str] = &[
    "ATZ",   // reset — esperar >1s
    "ATE0",  // echo off
    "ATL0",  // linefeed off
    "ATS0",  // espacios off
    "ATH0",  // headers off
    "ATSP0", // auto-detect protocol (CAN o KWP2000)
    "ATAT1", // adaptive timing on
    "0100",  // warm-up
];

pub struct TcpTransport {
    stream: TcpStream,
    addr: String,
}

impl TcpTransport {
    /// Conecta al proxy TCP. Reintenta x10 con backoff 1s.
    pub async fn connect(addr: &str) -> Result<Self> {
        let mut last_err = None;
        for attempt in 1..=10 {
            match TcpStream::connect(addr).await {
                Ok(stream) => {
                    stream.set_nodelay(true)?;
                    let mut t = TcpTransport {
                        stream,
                        addr: addr.to_string(),
                    };
                    t.init().await?;
                    return Ok(t);
                }
                Err(e) => {
                    warn!("intento {}/10 — proxy BT no disponible: {}", attempt, e);
                    last_err = Some(e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
        Err(last_err.unwrap().into())
    }

    async fn init(&mut self) -> Result<()> {
        // ATZ necesita 1500ms — el ELM327 reinicia su firmware completo
        self.send_raw("ATZ\r").await?;
        tokio::time::sleep(Duration::from_millis(1500)).await;
        self.drain().await?;

        for cmd in &INIT_SEQUENCE[1..] {
            let resp = self.send(cmd).await?;
            debug!("init {} → {}", cmd, resp.trim());
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok(())
    }

    /// Reconecta al mismo proxy TCP y re-inicializa el ELM327.
    /// Llamar cuando el ELM327 devuelve STOPPED o tras error de transporte.
    pub async fn reconnect(&mut self) -> Result<()> {
        let addr = self.addr.clone();
        let new = Self::connect(&addr).await?;
        self.stream = new.stream;
        Ok(())
    }

    pub async fn send(&mut self, cmd: &str) -> Result<String> {
        self.send_raw(&format!("{}\r", cmd)).await?;
        self.read_response().await
    }

    async fn send_raw(&mut self, raw: &str) -> Result<()> {
        self.stream
            .write_all(raw.as_bytes())
            .await
            .context("error escribiendo al proxy BT")
    }

    async fn read_response(&mut self) -> Result<String> {
        let mut buf = Vec::with_capacity(64);
        let mut reader = BufReader::new(&mut self.stream);

        timeout(Duration::from_secs(5), async {
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).await?;
                buf.extend_from_slice(line.as_bytes());
                if line.contains('>') {
                    break;
                }
            }
            Ok::<String, anyhow::Error>(String::from_utf8_lossy(&buf).into_owned())
        })
        .await
        .context("timeout esperando respuesta del ELM327")?
    }

    async fn drain(&mut self) -> Result<()> {
        let _ = timeout(Duration::from_millis(200), async {
            let mut tmp = [0u8; 256];
            loop {
                use tokio::io::AsyncReadExt;
                let _ = self.stream.read(&mut tmp).await;
            }
        })
        .await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_read_response_reads_until_prompt() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local_addr");

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            socket
                .write_all(b"41 0C 1A F8\r\n>\r\n")
                .await
                .expect("write response");
        });

        let stream = TcpStream::connect(addr).await.expect("connect");
        let mut t = TcpTransport {
            stream,
            addr: addr.to_string(),
        };
        let resp = t.read_response().await.expect("read_response");
        assert!(resp.contains("41 0C 1A F8"));
        assert!(resp.contains('>'));
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn test_send_appends_carriage_return() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local_addr");

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut buf = [0u8; 64];
            let n = socket.read(&mut buf).await.expect("read cmd");
            let received = String::from_utf8_lossy(&buf[..n]).to_string();
            socket.write_all(b"OK\r\n>\r\n").await.expect("write ack");
            received
        });

        let stream = TcpStream::connect(addr).await.expect("connect");
        let mut t = TcpTransport {
            stream,
            addr: addr.to_string(),
        };
        let resp = t.send("ATI").await.expect("send");
        assert!(resp.contains("OK"));
        let received = server.await.expect("server task");
        assert_eq!(received, "ATI\r");
    }
}
