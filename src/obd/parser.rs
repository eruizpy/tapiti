use super::{ObdError, ObdResult};

pub fn parse_response(raw: &str) -> ObdResult<Vec<u8>> {
    let clean = raw
        .lines()
        .find(|l| !l.trim().is_empty() && !l.contains('>'))
        .unwrap_or("")
        .trim();

    match clean.to_uppercase().as_str() {
        s if s.contains("NO DATA") => return Err(ObdError::NoData),
        s if s.contains("STOPPED") => return Err(ObdError::Stopped),
        s if s.contains("BUS INIT") => return Err(ObdError::BusInit),
        s if s.contains("ERROR") => return Err(ObdError::DeviceError),
        s if s.contains('?') => return Err(ObdError::DeviceError),
        _ => {}
    }

    let bytes: Result<Vec<u8>, _> = clean
        .split_whitespace()
        .filter(|s| s.len() == 2)
        .map(|s| u8::from_str_radix(s, 16))
        .collect();

    bytes.map_err(|_| ObdError::Corrupted(clean.to_string()))
}

pub mod mode01 {
    use super::*;

    pub fn rpm(bytes: &[u8]) -> ObdResult<f64> {
        check_len(bytes, 4)?;
        Ok(((bytes[2] as f64 * 256.0) + bytes[3] as f64) / 4.0)
    }
    pub fn coolant_temp(bytes: &[u8]) -> ObdResult<f64> {
        check_len(bytes, 3)?;
        Ok(bytes[2] as f64 - 40.0)
    }
    pub fn intake_temp(bytes: &[u8]) -> ObdResult<f64> {
        check_len(bytes, 3)?;
        Ok(bytes[2] as f64 - 40.0)
    }
    pub fn map_kpa(bytes: &[u8]) -> ObdResult<f64> {
        check_len(bytes, 3)?;
        Ok(bytes[2] as f64)
    }
    pub fn tps_pct(bytes: &[u8]) -> ObdResult<f64> {
        check_len(bytes, 3)?;
        Ok(bytes[2] as f64 * 100.0 / 255.0)
    }
    pub fn maf_gs(bytes: &[u8]) -> ObdResult<f64> {
        check_len(bytes, 4)?;
        Ok(((bytes[2] as f64 * 256.0) + bytes[3] as f64) / 100.0)
    }
    pub fn fuel_pressure_kpa(bytes: &[u8]) -> ObdResult<f64> {
        check_len(bytes, 3)?;
        Ok(bytes[2] as f64 * 3.0)
    }
    pub fn engine_load(bytes: &[u8]) -> ObdResult<f64> {
        check_len(bytes, 3)?;
        Ok(bytes[2] as f64 * 100.0 / 255.0)
    }
}

/// Modo 21 Subaru EJ205 USDM 2007
/// ADVERTENCIA: verificar offsets contra RomRaider antes de usar en producción
pub mod mode21 {
    use super::*;

    pub fn knock_fine(bytes: &[u8]) -> ObdResult<f64> {
        check_len(bytes, 23)?;
        Ok((bytes[22] as f64 - 128.0) * 0.352)
    }
    pub fn knock_learn(bytes: &[u8]) -> ObdResult<f64> {
        check_len(bytes, 27)?;
        Ok((bytes[26] as f64 - 128.0) * 0.352)
    }
    pub fn timing_advance(bytes: &[u8]) -> ObdResult<f64> {
        check_len(bytes, 24)?;
        Ok((bytes[23] as f64 - 128.0) * 0.352)
    }
    /// IAM: 1.0 = sin knock activo. <0.875 = revisar. <0.75 = detener sesión
    pub fn iam(bytes: &[u8]) -> ObdResult<f64> {
        check_len(bytes, 31)?;
        Ok(bytes[30] as f64 / 16.0)
    }
    pub fn boost_psi(bytes: &[u8]) -> ObdResult<f64> {
        check_len(bytes, 17)?;
        Ok((bytes[16] as f64 * 0.0681818) - 14.696)
    }
}

fn check_len(bytes: &[u8], min: usize) -> ObdResult<()> {
    if bytes.len() < min {
        Err(ObdError::Corrupted(format!(
            "esperaba {} bytes, recibí {}",
            min,
            bytes.len()
        )))
    } else {
        Ok(())
    }
}
