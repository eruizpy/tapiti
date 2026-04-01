package com.tapiti.obd

import android.bluetooth.BluetoothSocket
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.launch
import java.io.Closeable
import java.io.InputStream
import java.io.OutputStream
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket

/**
 * Bridge bidireccional: BluetoothSocket ↔ ServerSocket TCP loopback.
 *
 * El proceso Rust se conecta a 127.0.0.1:35000.
 * backlog=1 — solo el proceso Rust local debe conectarse.
 */
class TcpProxyServer(private val btSocket: BluetoothSocket) {

    companion object {
        private const val TAG = "tapiti/Proxy"
        private const val PORT = 35000
        private const val BUF = 4096
    }

    @Volatile private var running = false
    private var serverSocket: ServerSocket? = null
    private var clientSocket: Socket? = null
    private var job: Job? = null

    fun start(scope: CoroutineScope) {
        running = true
        job = scope.launch(Dispatchers.IO) { acceptLoop() }
        Log.i(TAG, "Proxy TCP escuchando en 127.0.0.1:$PORT")
    }

    private suspend fun acceptLoop() {
        val server = ServerSocket(PORT, 1, InetAddress.getLoopbackAddress())
        serverSocket = server
        server.use {
            // Espera la conexión del proceso Rust — solo aceptamos una
            val client = try {
                server.accept()
            } catch (e: Exception) {
                if (running) Log.e(TAG, "accept: ${e.message}")
                return
            }
            clientSocket = client
            Log.i(TAG, "Rust conectado desde ${client.remoteSocketAddress}")
            bridge(client)
        }
    }

    /**
     * Lanza dos coroutines simétricas:
     *   BT → TCP  (datos que el ELM327 envía al proceso Rust)
     *   TCP → BT  (comandos AT/OBD que Rust envía al ELM327)
     * Cuando cualquiera se cierra, cancela la otra.
     */
    private suspend fun bridge(tcpClient: Socket) {
        val btIn:  InputStream  = btSocket.inputStream
        val btOut: OutputStream = btSocket.outputStream
        val tcpIn: InputStream  = tcpClient.inputStream
        val tcpOut: OutputStream = tcpClient.outputStream

        val btToTcp = CoroutineScope(Dispatchers.IO).launch {
            pump(source = btIn, sink = tcpOut, label = "BT→TCP")
        }
        val tcpToBt = CoroutineScope(Dispatchers.IO).launch {
            pump(source = tcpIn, sink = btOut, label = "TCP→BT")
        }

        // Esperar a que cualquiera termine y cerrar la otra
        try {
            btToTcp.join()
        } finally {
            tcpToBt.cancelAndJoin()
            closeQuietly(tcpClient)
            Log.i(TAG, "Bridge cerrado")
        }
    }

    private fun pump(source: InputStream, sink: OutputStream, label: String) {
        val buf = ByteArray(BUF)
        try {
            var n: Int
            while (source.read(buf).also { n = it } != -1) {
                sink.write(buf, 0, n)
                sink.flush()
            }
        } catch (e: Exception) {
            if (running) Log.d(TAG, "$label cerrado: ${e.message}")
        }
    }

    fun stop() {
        running = false
        closeQuietly(clientSocket)
        closeQuietly(serverSocket)
        Log.i(TAG, "Proxy detenido")
    }

    private fun closeQuietly(c: Closeable?) {
        try { c?.close() } catch (_: Exception) {}
    }
}
