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
