mod broker;
mod dtc;
mod logger;
mod obd;
mod scheduler;
mod server;
mod subaru;
mod transport;

use anyhow::Result;
use tracing::info;

struct Args {
    bt_proxy: String,
    listen: String,
    db: String,
    poll_ms: u64,
}

impl Args {
    fn parse() -> Self {
        let mut args = std::env::args().skip(1);
        let mut bt_proxy = "127.0.0.1:35000".to_string();
        let mut listen = "127.0.0.1:8080".to_string();
        let mut db = "/data/data/com.tapiti.obd/files/tapiti.db".to_string();
        let mut poll_ms = 100u64;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--bt-proxy" => {
                    if let Some(v) = args.next() {
                        bt_proxy = v;
                    }
                }
                "--listen" => {
                    if let Some(v) = args.next() {
                        listen = v;
                    }
                }
                "--db" => {
                    if let Some(v) = args.next() {
                        db = v;
                    }
                }
                "--poll-ms" => {
                    if let Some(v) = args.next() {
                        poll_ms = v.parse().unwrap_or(100);
                    }
                }
                _ => {}
            }
        }
        Self {
            bt_proxy,
            listen,
            db,
            poll_ms,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    let args = Args::parse();
    info!(
        "tapiti v0.1.0 — bt_proxy={} listen={}",
        args.bt_proxy, args.listen
    );

    let (tx, _) = tokio::sync::broadcast::channel::<obd::Reading>(256);

    let store = logger::SqliteStore::new(&args.db).await?;
    let conn = transport::TcpTransport::connect(&args.bt_proxy).await?;
    let transport = std::sync::Arc::new(tokio::sync::Mutex::new(conn));

    let scheduler = scheduler::PidScheduler::new(
        std::sync::Arc::clone(&transport),
        tx.clone(),
        store.clone(),
        args.poll_ms,
    );
    let srv = server::HttpServer::new(&args.listen, tx, store, transport);

    tokio::try_join!(scheduler.run(), srv.run(),)?;

    Ok(())
}
