use crate::logger::SqliteStore;
use crate::obd::parser::parse_response;
use crate::obd::ObdError;
use crate::obd::Reading;
use crate::subaru::{ej205_pids, make_reading, Priority};
use crate::transport::TcpTransport;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tokio::time::{interval, Duration, MissedTickBehavior};
use tracing::{debug, error, info, warn};

/// Cuántos STOPPED consecutivos disparan una reconexión completa del ELM327.
const STOPPED_RECONNECT_THRESHOLD: u32 = 3;

pub struct PidScheduler {
    transport: Arc<Mutex<TcpTransport>>,
    tx: broadcast::Sender<Reading>,
    store: SqliteStore,
    base_ms: u64,
}

impl PidScheduler {
    pub fn new(
        transport: Arc<Mutex<TcpTransport>>,
        tx: broadcast::Sender<Reading>,
        store: SqliteStore,
        base_ms: u64,
    ) -> Self {
        Self {
            transport,
            tx,
            store,
            base_ms,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        let pids = ej205_pids();
        let mut tick = 0u64;
        let mut stopped_count = 0u32;

        let mut timer = interval(Duration::from_millis(self.base_ms));
        // Skip — no acumula ticks perdidos si un ciclo tarda más de lo esperado
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

        self.init_elm327().await;

        loop {
            timer.tick().await;

            for pid in &pids {
                let divisor = match pid.priority {
                    Priority::Critical => 1,
                    Priority::High => 3,
                    Priority::Normal => 5,
                    Priority::Low => 10,
                };
                if !tick.is_multiple_of(divisor) {
                    continue;
                }

                let send_result = self.transport.lock().await.send(pid.cmd).await;
                match send_result {
                    Ok(raw) => match parse_response(&raw) {
                        Ok(bytes) => {
                            stopped_count = 0;
                            match (pid.decode)(&bytes) {
                                Ok(value) => {
                                    let reading = make_reading(pid, value);
                                    debug!("{} = {} {}", reading.pid, reading.value, reading.unit);
                                    let _ = self.tx.send(reading.clone());
                                    if let Err(e) = self.store.insert(&reading).await {
                                        warn!("logger: {}", e);
                                    }
                                }
                                Err(ObdError::Corrupted(s)) => {
                                    warn!("PID {} corrupted: {}", pid.name, s)
                                }
                                Err(e) => warn!("PID {} decode: {}", pid.name, e),
                            }
                        }
                        Err(ObdError::NoData) => debug!("PID {} NO DATA — skip", pid.name),
                        Err(ObdError::Stopped) => {
                            stopped_count += 1;
                            warn!(
                                "PID {} STOPPED ({}/{})",
                                pid.name, stopped_count, STOPPED_RECONNECT_THRESHOLD
                            );
                            if stopped_count >= STOPPED_RECONNECT_THRESHOLD {
                                stopped_count = 0;
                                self.reconnect().await;
                                // Resetear tick para evitar desfase tras pausa
                                tick = 0;
                                break;
                            }
                        }
                        Err(e) => error!("PID {} parse: {}", pid.name, e),
                    },
                    Err(e) => {
                        error!("transport error {}: {}", pid.name, e);
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        self.reconnect().await;
                        tick = 0;
                        break;
                    }
                }
            }
            tick = tick.wrapping_add(1);
        }
    }

    async fn init_elm327(&mut self) {
        // Habilitar modo 21 — puede fallar en Viecar clon, continúa con modo 01
        match self.transport.lock().await.send("AT SH 7E0").await {
            Ok(r) => debug!("AT SH 7E0 → {}", r.trim()),
            Err(e) => warn!("modo 21 no disponible: {}", e),
        }
    }

    async fn reconnect(&mut self) {
        warn!("Reconectando al ELM327…");
        tokio::time::sleep(Duration::from_millis(1000)).await;
        let result = self.transport.lock().await.reconnect().await;
        match result {
            Ok(()) => {
                info!("Reconexión exitosa");
                self.init_elm327().await;
            }
            Err(e) => error!("Reconexión fallida: {}", e),
        }
    }
}
