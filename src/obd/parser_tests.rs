/// Tests para obd::parser — convención: test_DADO_CUANDO_ENTONCES
#[cfg(test)]
mod tests {
    use crate::obd::parser::{mode01, mode21, parse_response};
    use crate::obd::{ObdError, ObdResult};

    // ── parse_response ──────────────────────────────────────────────────────

    #[test]
    fn test_parse_response_bytes_validos_cuando_hex_limpio_entonces_vec_bytes() {
        let raw = "41 0C 1A F8\r\n>";
        let result = parse_response(raw);
        assert_eq!(result.unwrap(), vec![0x41, 0x0C, 0x1A, 0xF8]);
    }

    #[test]
    fn test_parse_response_no_data_cuando_elm_devuelve_no_data_entonces_err_nodata() {
        let result = parse_response("NO DATA\r\n>");
        assert!(matches!(result, Err(ObdError::NoData)));
    }

    #[test]
    fn test_parse_response_stopped_cuando_elm_devuelve_stopped_entonces_err_stopped() {
        let result = parse_response("STOPPED\r\n>");
        assert!(matches!(result, Err(ObdError::Stopped)));
    }

    #[test]
    fn test_parse_response_device_error_cuando_elm_devuelve_error_entonces_err_deviceerror() {
        let result = parse_response("ERROR\r\n>");
        assert!(matches!(result, Err(ObdError::DeviceError)));
    }

    #[test]
    fn test_parse_response_bus_init_cuando_elm_devuelve_bus_init_entonces_err_businit() {
        let result = parse_response("BUS INIT: ...ERROR\r\n>");
        assert!(matches!(result, Err(ObdError::BusInit)));
    }

    #[test]
    fn test_parse_response_device_error_cuando_elm_devuelve_signo_interrogacion_entonces_err() {
        let result = parse_response("?\r\n>");
        assert!(matches!(result, Err(ObdError::DeviceError)));
    }

    #[test]
    fn test_parse_response_corrupto_cuando_hex_invalido_entonces_err_corrupted() {
        let result = parse_response("ZZ XX\r\n>");
        assert!(matches!(result, Err(ObdError::Corrupted(_))));
    }

    #[test]
    fn test_parse_response_ignora_linea_prompt_cuando_raw_tiene_angulo_entonces_toma_primer_dato() {
        // El prompt '>' puede aparecer en la misma línea o separado
        let raw = "\r\n41 05 69\r\n>";
        let result = parse_response(raw);
        assert_eq!(result.unwrap(), vec![0x41, 0x05, 0x69]);
    }

    // ── mode01 ──────────────────────────────────────────────────────────────

    #[test]
    fn test_rpm_bytes_validos_cuando_a_1a_b_f8_entonces_1726() {
        // RPM = ((0x1A * 256) + 0xF8) / 4 = (6656 + 248) / 4 = 1726.0
        let bytes = [0x41, 0x0C, 0x1A, 0xF8];
        let rpm: f64 = mode01::rpm(&bytes).unwrap();
        assert!((rpm - 1726.0).abs() < 0.1, "rpm={}", rpm);
    }

    #[test]
    fn test_rpm_bytes_insuficientes_cuando_solo_3_bytes_entonces_err_corrupted() {
        let bytes = [0x41, 0x0C, 0x1A];
        assert!(matches!(mode01::rpm(&bytes), Err(ObdError::Corrupted(_))));
    }

    #[test]
    fn test_coolant_temp_bytes_validos_cuando_a_7d_entonces_85_grados() {
        // coolant = 0x7D - 40 = 125 - 40 = 85°C
        let bytes = [0x41, 0x05, 0x7D];
        let temp = mode01::coolant_temp(&bytes).unwrap();
        assert!((temp - 85.0).abs() < 0.1, "temp={}", temp);
    }

    #[test]
    fn test_tps_pct_bytes_validos_cuando_a_80_entonces_50_porciento() {
        // TPS = 0x80 * 100 / 255 ≈ 50.196%
        let bytes = [0x41, 0x11, 0x80];
        let tps = mode01::tps_pct(&bytes).unwrap();
        assert!((tps - 50.196).abs() < 0.01, "tps={}", tps);
    }

    #[test]
    fn test_maf_gs_bytes_validos_cuando_a_01_b_90_entonces_4_grams() {
        // MAF = ((0x01 * 256) + 0x90) / 100 = (256 + 144) / 100 = 4.0 g/s
        let bytes = [0x41, 0x10, 0x01, 0x90];
        let maf = mode01::maf_gs(&bytes).unwrap();
        assert!((maf - 4.0).abs() < 0.01, "maf={}", maf);
    }

    #[test]
    fn test_map_kpa_bytes_validos_cuando_a_78_entonces_120_kpa() {
        let bytes = [0x41, 0x0B, 0x78];
        let map = mode01::map_kpa(&bytes).unwrap();
        assert!((map - 120.0).abs() < 0.1, "map={}", map);
    }

    #[test]
    fn test_fuel_pressure_bytes_validos_cuando_a_5a_entonces_270_kpa() {
        // fuel = 0x5A * 3 = 90 * 3 = 270 kPa
        let bytes = [0x41, 0x0A, 0x5A];
        let p = mode01::fuel_pressure_kpa(&bytes).unwrap();
        assert!((p - 270.0).abs() < 0.1, "fuel_pres={}", p);
    }

    #[test]
    fn test_engine_load_bytes_validos_cuando_a_80_entonces_50_porciento() {
        let bytes = [0x41, 0x04, 0x80];
        let load = mode01::engine_load(&bytes).unwrap();
        assert!((load - 50.196).abs() < 0.01, "load={}", load);
    }

    // ── mode21 ──────────────────────────────────────────────────────────────

    #[test]
    fn test_knock_fine_bytes_validos_cuando_byte22_es_128_entonces_cero_grados() {
        // knock_fine = (128 - 128) * 0.352 = 0.0°
        let mut bytes = vec![0u8; 31];
        bytes[22] = 128;
        let knock = mode21::knock_fine(&bytes).unwrap();
        assert!((knock - 0.0).abs() < 0.001, "knock_fine={}", knock);
    }

    #[test]
    fn test_knock_fine_bytes_validos_cuando_byte22_es_114_entonces_negativo() {
        // knock_fine = (114 - 128) * 0.352 = -4.928°
        let mut bytes = vec![0u8; 31];
        bytes[22] = 114;
        let knock = mode21::knock_fine(&bytes).unwrap();
        assert!((knock - (-4.928)).abs() < 0.001, "knock_fine={}", knock);
    }

    #[test]
    fn test_iam_bytes_validos_cuando_byte30_es_16_entonces_uno() {
        // IAM = 16 / 16 = 1.0
        let mut bytes = vec![0u8; 31];
        bytes[30] = 16;
        let iam = mode21::iam(&bytes).unwrap();
        assert!((iam - 1.0).abs() < 0.001, "iam={}", iam);
    }

    #[test]
    fn test_iam_bytes_validos_cuando_byte30_es_14_entonces_0_875() {
        // IAM = 14 / 16 = 0.875 — límite de alerta
        let mut bytes = vec![0u8; 31];
        bytes[30] = 14;
        let iam = mode21::iam(&bytes).unwrap();
        assert!((iam - 0.875).abs() < 0.001, "iam={}", iam);
    }

    #[test]
    fn test_boost_psi_bytes_validos_cuando_byte16_es_216_entonces_approx_nominal() {
        // boost = 216 * 0.0681818 - 14.696 ≈ 0.0°
        let mut bytes = vec![0u8; 31];
        bytes[16] = 216; // ~14.7 psi → ~0 psi gauge
        let boost = mode21::boost_psi(&bytes).unwrap();
        // 216 * 0.0681818 = 14.727 - 14.696 = 0.031 psi ≈ atmósfera
        assert!(boost.abs() < 0.1, "boost={}", boost);
    }

    #[test]
    fn test_iam_bytes_insuficientes_cuando_menos_de_31_bytes_entonces_err_corrupted() {
        let bytes = vec![0u8; 20];
        assert!(matches!(mode21::iam(&bytes), Err(ObdError::Corrupted(_))));
    }

    // ── Vectores de test case-insensitive ───────────────────────────────────

    #[test]
    fn test_parse_response_no_data_minusculas_cuando_no_data_minusculas_entonces_err_nodata() {
        let result = parse_response("no data\r\n>");
        assert!(matches!(result, Err(ObdError::NoData)));
    }
}
