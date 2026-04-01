/// Tests para server — RFC 6455, Origin validation, query params
#[cfg(test)]
mod tests {
    use crate::server::{
        base64_encode, extract_query_param, sha1, ws_accept_key, ws_origin_allowed,
    };

    // Vector de test oficial — RFC 6455 §1.3
    // Input key:  "dGhlIHNhbXBsZSBub25jZQ=="
    // Expected:   "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    #[test]
    fn test_ws_accept_key_dado_key_rfc_ejemplo_cuando_se_calcula_entonces_valor_correcto() {
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let accept = ws_accept_key(key);
        assert_eq!(
            accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=",
            "El accept key no coincide con el vector de test RFC 6455"
        );
    }

    #[test]
    fn test_sha1_dado_abc_cuando_se_hashea_entonces_a9993e36() {
        // SHA-1("abc") = a9993e364706816aba3e25717850c26c9cd0d89d
        let hash = sha1(b"abc");
        let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(hex, "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn test_sha1_dado_string_vacio_cuando_se_hashea_entonces_da39a3ee() {
        // SHA-1("") = da39a3ee5e6b4b0d3255bfef95601890afd80709
        let hash = sha1(b"");
        let hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(hex, "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn test_base64_encode_dado_man_cuando_se_codifica_entonces_tw_a_n() {
        // base64("Man") = "TWFu"
        assert_eq!(base64_encode(b"Man"), "TWFu");
    }

    #[test]
    fn test_base64_encode_dado_un_byte_cuando_se_codifica_entonces_padding_correcto() {
        // base64("M") = "TQ=="
        assert_eq!(base64_encode(b"M"), "TQ==");
    }

    #[test]
    fn test_base64_encode_dado_dos_bytes_cuando_se_codifica_entonces_un_padding() {
        // base64("Ma") = "TWE="
        assert_eq!(base64_encode(b"Ma"), "TWE=");
    }

    // ── ws_origin_allowed ────────────────────────────────────────────────────

    #[test]
    fn test_origin_dado_sin_header_cuando_upgrade_ws_entonces_permitido() {
        let req = "GET / HTTP/1.1\r\nUpgrade: websocket\r\n\r\n";
        assert!(ws_origin_allowed(req));
    }

    #[test]
    fn test_origin_dado_null_cuando_webview_android_entonces_permitido() {
        let req = "GET / HTTP/1.1\r\nOrigin: null\r\nUpgrade: websocket\r\n\r\n";
        assert!(ws_origin_allowed(req));
    }

    #[test]
    fn test_origin_dado_loopback_cuando_webview_con_url_entonces_permitido() {
        let req = "GET / HTTP/1.1\r\nOrigin: http://127.0.0.1\r\nUpgrade: websocket\r\n\r\n";
        assert!(ws_origin_allowed(req));
    }

    #[test]
    fn test_origin_dado_localhost_cuando_desarrollo_local_entonces_permitido() {
        let req = "GET / HTTP/1.1\r\nOrigin: http://localhost\r\nUpgrade: websocket\r\n\r\n";
        assert!(ws_origin_allowed(req));
    }

    #[test]
    fn test_origin_dado_externo_cuando_otra_pagina_entonces_rechazado() {
        let req = "GET / HTTP/1.1\r\nOrigin: http://evil.com\r\nUpgrade: websocket\r\n\r\n";
        assert!(!ws_origin_allowed(req));
    }

    #[test]
    fn test_origin_dado_loopback_con_puerto_cuando_no_es_raiz_entonces_rechazado() {
        // http://127.0.0.1:8080 es distinto de http://127.0.0.1 — rechazar
        let req = "GET / HTTP/1.1\r\nOrigin: http://127.0.0.1:8080\r\nUpgrade: websocket\r\n\r\n";
        assert!(!ws_origin_allowed(req));
    }

    #[test]
    fn test_origin_case_insensitive_cuando_origin_en_mayusculas_entonces_permitido() {
        let req = "GET / HTTP/1.1\r\nORIGIN: null\r\nUpgrade: websocket\r\n\r\n";
        assert!(ws_origin_allowed(req));
    }

    // ── extract_query_param ──────────────────────────────────────────────────

    #[test]
    fn test_query_param_dado_session_cuando_presente_entonces_retorna_valor() {
        let req = "GET /export?session=20240101_120000 HTTP/1.1\r\n\r\n";
        assert_eq!(
            extract_query_param(req, "session"),
            Some("20240101_120000".to_string())
        );
    }

    #[test]
    fn test_query_param_dado_multiples_cuando_busca_segundo_entonces_retorna_correcto() {
        let req = "GET /export?foo=bar&session=20240202_090000 HTTP/1.1\r\n\r\n";
        assert_eq!(
            extract_query_param(req, "session"),
            Some("20240202_090000".to_string())
        );
    }

    #[test]
    fn test_query_param_dado_sin_query_cuando_busca_param_entonces_none() {
        let req = "GET /export HTTP/1.1\r\n\r\n";
        assert_eq!(extract_query_param(req, "session"), None);
    }
}
