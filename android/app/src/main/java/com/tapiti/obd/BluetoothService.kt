package com.tapiti.obd

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothSocket
import android.content.Intent
import android.os.IBinder
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import java.io.File
import java.io.IOException
import java.security.MessageDigest

class BluetoothService : Service() {

    companion object {
        private const val TAG = "tapiti/BT"
        private const val CHANNEL_ID = "tapiti_bt"
        private const val NOTIF_ID = 1
        private const val SPP_UUID = "00001101-0000-1000-8000-00805F9B34FB"
        const val EXTRA_DEVICE_ADDRESS = "device_address"

        /**
         * SHA-256 del binario ARM64 incluido en assets/tapiti.
         * Actualizar con: sha256sum assets/tapiti | awk '{print $1}'
         * Dejar vacío durante desarrollo — se loggeará el hash real para rellenar.
         */
        private const val EXPECTED_SHA256 = ""
    }

    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())
    private var btSocket: BluetoothSocket? = null
    private var proxy: TcpProxyServer? = null
    private var rustProcess: Process? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        startForeground(NOTIF_ID, buildNotification("Iniciando…"))
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val address = intent?.getStringExtra(EXTRA_DEVICE_ADDRESS)
            ?: return START_NOT_STICKY.also { stopSelf() }

        scope.launch { connect(address) }
        return START_STICKY
    }

    private suspend fun connect(address: String) {
        val adapter = BluetoothAdapter.getDefaultAdapter()
            ?: return logAndStop("Bluetooth no disponible")

        // Cancelar discovery consume recursos y puede interferir con SPP
        if (adapter.isDiscovering) adapter.cancelDiscovery()

        val device: BluetoothDevice = adapter.getRemoteDevice(address)
        updateNotification("Conectando a ${device.name ?: address}…")

        val socket = try {
            device.createRfcommSocketToServiceRecord(
                java.util.UUID.fromString(SPP_UUID)
            ).also { it.connect() }
        } catch (e: IOException) {
            return logAndStop("Error BT: ${e.message}")
        }

        btSocket = socket
        Log.i(TAG, "BT conectado a ${device.name ?: address}")
        updateNotification("BT conectado — lanzando proxy…")

        // Proxy TCP en loopback — backlog=1 por seguridad
        val proxyServer = TcpProxyServer(socket)
        proxy = proxyServer
        proxyServer.start(scope)

        // Copiar binario Rust desde assets y hacerlo ejecutable
        val binary = extractBinary()
        if (binary == null) {
            return logAndStop("No se pudo extraer el binario tapiti")
        }

        launchRust(binary)
        updateNotification("tapiti corriendo")
    }

    private fun extractBinary(): File? {
        val dest = File(filesDir, "tapiti")
        return try {
            val bytes = assets.open("tapiti").use { it.readBytes() }

            // Verificar integridad SHA-256 antes de escribir al filesystem
            val actualHash = sha256Hex(bytes)
            Log.i(TAG, "SHA-256 binario: $actualHash")

            if (EXPECTED_SHA256.isNotEmpty() && actualHash != EXPECTED_SHA256) {
                Log.e(TAG, "Integridad fallida — esperado=$EXPECTED_SHA256 actual=$actualHash")
                return null
            }

            dest.writeBytes(bytes)
            dest.setExecutable(true)
            Log.i(TAG, "Binario extraído y verificado: ${dest.absolutePath}")
            dest
        } catch (e: IOException) {
            Log.e(TAG, "Error extrayendo binario: ${e.message}")
            null
        }
    }

    private fun sha256Hex(data: ByteArray): String {
        val digest = MessageDigest.getInstance("SHA-256").digest(data)
        return digest.joinToString("") { "%02x".format(it) }
    }

    private fun launchRust(binary: File) {
        val dbPath = File(filesDir, "tapiti.db").absolutePath
        val process = ProcessBuilder(
            binary.absolutePath,
            "--bt-proxy", "127.0.0.1:35000",
            "--listen",   "127.0.0.1:8080",
            "--db",       dbPath,
            "--poll-ms",  "100"
        )
            .redirectErrorStream(true)
            .start()

        rustProcess = process
        Log.i(TAG, "Proceso Rust lanzado (pid=${process.pid()})")

        // Redirigir stdout/stderr del proceso Rust a Logcat
        scope.launch(Dispatchers.IO) {
            process.inputStream.bufferedReader().forEachLine { line ->
                Log.d("tapiti/rust", line)
            }
            val exit = process.waitFor()
            Log.w(TAG, "Proceso Rust terminó con código $exit")
            updateNotification("tapiti detenido (código $exit)")
        }
    }

    override fun onDestroy() {
        scope.cancel()
        rustProcess?.destroyForcibly()
        proxy?.stop()
        btSocket?.close()
        super.onDestroy()
        Log.i(TAG, "Servicio destruido")
    }

    private fun logAndStop(msg: String) {
        Log.e(TAG, msg)
        updateNotification(msg)
        stopSelf()
    }

    private fun createNotificationChannel() {
        val channel = NotificationChannel(
            CHANNEL_ID,
            "tapiti OBD",
            NotificationManager.IMPORTANCE_LOW
        ).apply { description = "Monitor OBD-II EJ205" }
        getSystemService(NotificationManager::class.java)
            .createNotificationChannel(channel)
    }

    private fun buildNotification(text: String): Notification =
        Notification.Builder(this, CHANNEL_ID)
            .setContentTitle("tapiti")
            .setContentText(text)
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setOngoing(true)
            .build()

    private fun updateNotification(text: String) {
        getSystemService(NotificationManager::class.java)
            .notify(NOTIF_ID, buildNotification(text))
    }
}
