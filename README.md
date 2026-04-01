# tapiti

Monitor OBD-II para Subaru Impreza EJ205 (2007), escrito en Rust. Corre como
proceso nativo ARM64 en Android con UI en WebView HTML/SVG.

El nombre viene del guarani *tapiti* — el conejo silvestre del Chaco.

## Stack

```
Hardware:  Subaru EJ205 ECU → OBD-II J1962 → Viecar V2.1 (BT SPP)
Protocolo: ISO 15765-4 CAN 11/500
Transport: BluetoothSocket SPP → TcpProxy 127.0.0.1:35000 → Rust TcpTransport
Core:      Rust tokio async — binario ARM64 embebido en APK
UI:        WebView → ws://127.0.0.1:8080 → HTML/SVG gauges
DB:        rusqlite bundled (SQLite)
Android:   Kotlin — BluetoothService + TcpProxyServer + WebViewActivity
```

## Estructura

```
tapiti/
├── src/
│   ├── main.rs              entry point, args, tokio::try_join
│   ├── transport/mod.rs     TCP con retry, init ELM327, reconnect
│   ├── obd/                 parser, comandos AT, errores
│   ├── subaru/mod.rs        PIDs EJ205, prioridades, make_reading
│   ├── scheduler/mod.rs     polling ciclico con auto-reconexion
│   ├── logger/mod.rs        SQLite store — insert, export CSV
│   ├── dtc/mod.rs           read/clear DTCs (modos 03/04)
│   ├── server/mod.rs        HTTP + WebSocket RFC 6455
│   └── broker/mod.rs        broadcast de readings
├── android/
│   └── app/src/main/
│       ├── java/com/tapiti/obd/
│       │   ├── BluetoothService.kt
│       │   ├── TcpProxyServer.kt
│       │   └── WebViewActivity.kt
│       └── assets/ui/index.html
├── Makefile
├── Cargo.toml
└── LICENSE
```

## Requisitos

- Rust 1.75+ con target `aarch64-linux-android`
- Android NDK (para cross-compile)
- Gradle 8.4+ (o ejecutar `gradle wrapper` en `android/`)
- Dispositivo Android API 26+ con Bluetooth

## Build

```bash
# Verificar codigo
make check

# Compilar binario ARM64
make build

# Copiar binario a assets y compilar APK
make android-debug
```

La primera vez en el directorio `android/` necesitas generar el Gradle wrapper:

```bash
cd android && gradle wrapper --gradle-version 8.4
```

## Hardware

Probado con:
- Subaru Impreza WRX 2007 (EJ205, USDM)
- Adaptador Viecar V2.1 Bluetooth SPP (ELM327 compatible)
- PIN de pareado: `1234`

### Verificacion de protocolo

Conectar al adaptador y enviar `AT DP`. La respuesta esperada es
`ISO 15765-4 (CAN 11/500)`. Si responde otro protocolo, verificar que el
adaptador soporta CAN y que esta conectado al puerto OBD-II correcto.

## Seguridad

- Todos los sockets bindeados en loopback (127.0.0.1) — nunca en 0.0.0.0
- Comandos AT hardcodeados — sin interpolacion de input externo
- WebView sin acceso a filesystem ni content:// URIs
- Modo 04 (clear DTC) requiere confirmacion desde la UI
- Comandos de escritura/flash ECU (AT 34/35/36/37/3B) bloqueados

## Licencia

MIT — ver [LICENSE](LICENSE).
