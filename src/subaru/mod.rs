use crate::obd::parser::{mode01, mode21};
use crate::obd::{ObdResult, Reading};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Critical = 0, // 100ms  — RPM, boost, knock
    High = 1,     // 250ms  — TPS, MAF, timing
    Normal = 2,   // 500ms  — coolant, IAT, fuel pressure
    Low = 3,      // 1000ms — load, IAM
}

pub struct PidDef {
    pub name: &'static str,
    pub cmd: &'static str,
    pub unit: &'static str,
    pub priority: Priority,
    pub decode: fn(&[u8]) -> ObdResult<f64>,
}

pub fn ej205_pids() -> Vec<PidDef> {
    vec![
        PidDef {
            name: "rpm",
            cmd: "010C",
            unit: "rpm",
            priority: Priority::Critical,
            decode: mode01::rpm,
        },
        PidDef {
            name: "map",
            cmd: "010B",
            unit: "kPa",
            priority: Priority::Critical,
            decode: mode01::map_kpa,
        },
        PidDef {
            name: "tps",
            cmd: "0111",
            unit: "%",
            priority: Priority::High,
            decode: mode01::tps_pct,
        },
        PidDef {
            name: "maf",
            cmd: "0110",
            unit: "g/s",
            priority: Priority::High,
            decode: mode01::maf_gs,
        },
        PidDef {
            name: "coolant",
            cmd: "0105",
            unit: "°C",
            priority: Priority::Normal,
            decode: mode01::coolant_temp,
        },
        PidDef {
            name: "iat",
            cmd: "010F",
            unit: "°C",
            priority: Priority::Normal,
            decode: mode01::intake_temp,
        },
        PidDef {
            name: "fuel_pres",
            cmd: "010A",
            unit: "kPa",
            priority: Priority::Normal,
            decode: mode01::fuel_pressure_kpa,
        },
        PidDef {
            name: "load",
            cmd: "0104",
            unit: "%",
            priority: Priority::Low,
            decode: mode01::engine_load,
        },
        // Modo 21 Subaru — requiere AT SH 7E0 — puede fallar en Viecar clon
        PidDef {
            name: "knock_fine",
            cmd: "2101",
            unit: "°",
            priority: Priority::Critical,
            decode: mode21::knock_fine,
        },
        PidDef {
            name: "knock_learn",
            cmd: "2101",
            unit: "°",
            priority: Priority::High,
            decode: mode21::knock_learn,
        },
        PidDef {
            name: "timing",
            cmd: "2101",
            unit: "°",
            priority: Priority::High,
            decode: mode21::timing_advance,
        },
        PidDef {
            name: "boost_psi",
            cmd: "2101",
            unit: "psi",
            priority: Priority::Critical,
            decode: mode21::boost_psi,
        },
        PidDef {
            name: "iam",
            cmd: "2101",
            unit: "",
            priority: Priority::Low,
            decode: mode21::iam,
        },
    ]
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn make_reading(pid: &PidDef, value: f64) -> Reading {
    Reading {
        pid: pid.name,
        value,
        unit: pid.unit,
        ts_ms: now_ms(),
    }
}
