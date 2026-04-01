use crate::obd::parser::parse_response;
use crate::obd::{ObdError, ObdResult};
use crate::transport::TcpTransport;

#[derive(Debug, serde::Serialize)]
pub struct FaultCode {
    pub code: String,
    pub description: String,
}

pub async fn read_dtcs(transport: &mut TcpTransport) -> ObdResult<Vec<FaultCode>> {
    let raw = transport.send("03").await.map_err(ObdError::Transport)?;
    let bytes = parse_response(&raw)?;
    let mut codes = Vec::new();
    let mut i = 2;
    while i + 1 < bytes.len() {
        let a = bytes[i];
        let b = bytes[i + 1];
        if a == 0 && b == 0 {
            break;
        }
        let prefix = match (a >> 6) & 0x03 {
            0 => 'P',
            1 => 'C',
            2 => 'B',
            _ => 'U',
        };
        let code = format!(
            "{}{}{:02X}",
            prefix,
            (a >> 4) & 0x03,
            (((a & 0x0F) as u16) << 8) | (b as u16)
        );
        codes.push(FaultCode {
            description: generic_description(&code),
            code,
        });
        i += 2;
    }
    Ok(codes)
}

pub async fn clear_dtcs(transport: &mut TcpTransport) -> ObdResult<()> {
    let raw = transport.send("04").await.map_err(ObdError::Transport)?;
    let resp = raw.to_uppercase();
    if resp.contains("44") || resp.contains("OK") {
        Ok(())
    } else {
        Err(ObdError::DeviceError)
    }
}

fn generic_description(code: &str) -> String {
    match code {
        "P0300" => "Misfire detectado — múltiples cilindros",
        "P0301" => "Misfire cilindro 1",
        "P0302" => "Misfire cilindro 2",
        "P0303" => "Misfire cilindro 3",
        "P0304" => "Misfire cilindro 4",
        "P0325" => "Sensor de knock — circuito banco 1",
        "P0326" => "Sensor de knock — rango/performance banco 1",
        "P0327" => "Sensor de knock — entrada baja banco 1",
        "P0328" => "Sensor de knock — entrada alta banco 1",
        "P0335" => "Sensor de posición cigüeñal — circuito A",
        "P0340" => "Sensor de posición árbol de levas — circuito A",
        "P0500" => "Sensor de velocidad vehicular",
        "P0600" => "Comunicación serial — link ECU",
        _ => "Código genérico — consultar manual EJ205",
    }
    .to_string()
}
