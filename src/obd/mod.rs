#[allow(dead_code)]
pub mod commands;
pub mod parser;
#[cfg(test)]
mod parser_tests;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ObdError {
    #[error("NO DATA — ECU no respondió al PID")]
    NoData,
    #[error("STOPPED — ELM327 interrumpió la búsqueda")]
    Stopped,
    #[error("ERROR — respuesta de error genérica del ELM327")]
    DeviceError,
    #[error("BUS INIT — fallo de inicialización del bus CAN/KWP")]
    BusInit,
    #[error("frame corrupto: {0}")]
    Corrupted(String),
    #[error("timeout — sin respuesta en 5s")]
    #[allow(dead_code)]
    Timeout,
    #[error("transporte: {0}")]
    Transport(#[from] anyhow::Error),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Reading {
    pub pid: &'static str,
    pub value: f64,
    pub unit: &'static str,
    pub ts_ms: u64,
}

pub type ObdResult<T> = Result<T, ObdError>;
