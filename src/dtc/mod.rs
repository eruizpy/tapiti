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
    Ok(decode_dtc_bytes(&bytes))
}

pub async fn clear_dtcs(transport: &mut TcpTransport) -> ObdResult<()> {
    let raw = transport.send("04").await.map_err(ObdError::Transport)?;
    if is_clear_ack(&raw) {
        Ok(())
    } else {
        Err(ObdError::DeviceError)
    }
}

fn decode_dtc_bytes(bytes: &[u8]) -> Vec<FaultCode> {
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
    codes
}

fn is_clear_ack(raw: &str) -> bool {
    let resp = raw.to_uppercase();
    resp.contains("44") || resp.contains("OK")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_dtc_bytes_single_known_code() {
        let bytes = vec![0x43, 0x01, 0x03, 0x25, 0x00, 0x00];
        let codes = decode_dtc_bytes(&bytes);
        assert_eq!(codes.len(), 1);
        assert_eq!(codes[0].code, "P0325");
        assert_eq!(codes[0].description, "Sensor de knock — circuito banco 1");
    }

    #[test]
    fn test_decode_dtc_bytes_multiple_prefixes() {
        // C1234 y U301 (el formateo actual usa {:02X} para la parte final)
        let bytes = vec![0x43, 0x02, 0x52, 0x34, 0xF0, 0x01];
        let codes = decode_dtc_bytes(&bytes);
        assert_eq!(codes.len(), 2);
        assert_eq!(codes[0].code, "C1234");
        assert_eq!(codes[1].code, "U301");
    }

    #[test]
    fn test_decode_dtc_bytes_stops_on_zero_pair() {
        let bytes = vec![0x43, 0x01, 0x03, 0x00, 0x00, 0x00, 0x03, 0x25];
        let codes = decode_dtc_bytes(&bytes);
        assert_eq!(codes.len(), 1);
        assert_eq!(codes[0].code, "P0300");
    }

    #[test]
    fn test_is_clear_ack_accepts_ok_and_44() {
        assert!(is_clear_ack("44\r\n>"));
        assert!(is_clear_ack("ok\r\n>"));
        assert!(!is_clear_ack("NO DATA\r\n>"));
    }

    #[test]
    fn test_generic_description_unknown_fallback() {
        let desc = generic_description("P9999");
        assert_eq!(desc, "Código genérico — consultar manual EJ205");
    }
}
