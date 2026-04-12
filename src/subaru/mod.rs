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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ej205_pids_contains_expected_core_commands() {
        let pids = ej205_pids();
        assert!(!pids.is_empty());

        let rpm = pids.iter().find(|p| p.name == "rpm").expect("rpm exists");
        assert_eq!(rpm.cmd, "010C");
        assert_eq!(rpm.unit, "rpm");
        assert_eq!(rpm.priority, Priority::Critical);

        let iam = pids.iter().find(|p| p.name == "iam").expect("iam exists");
        assert_eq!(iam.cmd, "2101");
        assert_eq!(iam.priority, Priority::Low);
    }

    #[test]
    fn test_make_reading_maps_pid_metadata() {
        let pid = PidDef {
            name: "test_pid",
            cmd: "0100",
            unit: "u",
            priority: Priority::Normal,
            decode: mode01::map_kpa,
        };
        let before = now_ms();
        let reading = make_reading(&pid, 12.34);
        let after = now_ms();

        assert_eq!(reading.pid, "test_pid");
        assert_eq!(reading.unit, "u");
        assert_eq!(reading.value, 12.34);
        assert!(reading.ts_ms >= before);
        assert!(reading.ts_ms <= after);
    }

    #[test]
    fn test_now_ms_is_monotonic_enough_for_runtime_usage() {
        let a = now_ms();
        let b = now_ms();
        assert!(b >= a);
    }
}
