package com.tapiti.obd

import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.content.Intent
import android.os.Bundle
import android.util.Log
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

class WebViewActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "tapiti/WebView"
        private const val TAPITI_URL = "http://127.0.0.1:8080"
        private const val LAUNCH_DELAY_MS = 1500L
        private const val RETRY_DELAY_MS  = 2000L
    }

    private lateinit var webView: WebView
    private val scope = CoroutineScope(Dispatchers.Main)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Si no hay dispositivo guardado, ir al picker
        val savedAddress = DevicePickerActivity.getSavedAddress(this)
        if (savedAddress == null) {
            startActivity(Intent(this, DevicePickerActivity::class.java))
            finish()
            return
        }

        webView = WebView(this).apply {
            settings.apply {
                javaScriptEnabled        = true
                domStorageEnabled        = false
                allowFileAccess          = false
                allowContentAccess       = false
                allowFileAccessFromFileURLs = false
                allowUniversalAccessFromFileURLs = false
                // Sin acceso a geolocalización, cámara, micrófono
                setGeolocationEnabled(false)
            }
            webViewClient = TapitiWebViewClient()
        }
        setContentView(webView)

        startBluetoothService()
        loadWithDelay()
    }

    private fun startBluetoothService() {
        val adapter = BluetoothAdapter.getDefaultAdapter()
        if (adapter == null || !adapter.isEnabled) {
            Toast.makeText(this, "Bluetooth no disponible", Toast.LENGTH_LONG).show()
            return
        }

        val savedAddress = DevicePickerActivity.getSavedAddress(this)
        val device: BluetoothDevice? = if (savedAddress != null) {
            adapter.bondedDevices.firstOrNull { it.address == savedAddress }
        } else {
            adapter.bondedDevices.firstOrNull { it.name?.contains("VIECAR", ignoreCase = true) == true }
                ?: adapter.bondedDevices.firstOrNull { it.name?.contains("OBD", ignoreCase = true) == true }
        }

        if (device == null) {
            Toast.makeText(this, "Dispositivo OBD no encontrado — seleccionar en ajustes", Toast.LENGTH_LONG).show()
            Log.w(TAG, "Dispositivo OBD no encontrado entre los emparejados")
            return
        }

        Log.i(TAG, "Lanzando BluetoothService para ${device.name} (${device.address})")
        Intent(this, BluetoothService::class.java).also { intent ->
            intent.putExtra(BluetoothService.EXTRA_DEVICE_ADDRESS, device.address)
            startForegroundService(intent)
        }
    }

    private fun loadWithDelay() {
        scope.launch {
            // Espera a que Rust levante el servidor HTTP/WS
            delay(LAUNCH_DELAY_MS)
            loadTapiti()
        }
    }

    private fun loadTapiti() {
        Log.i(TAG, "Cargando $TAPITI_URL")
        webView.loadUrl(TAPITI_URL)
    }

    private inner class TapitiWebViewClient : WebViewClient() {

        override fun shouldOverrideUrlLoading(
            view: WebView,
            request: WebResourceRequest
        ): Boolean {
            val host = request.url.host ?: return true
            // Bloquear cualquier navegación fuera de loopback
            if (host != "127.0.0.1") {
                Log.w(TAG, "Bloqueada navegación a: ${request.url}")
                return true
            }
            return false
        }

        override fun onReceivedError(
            view: WebView,
            errorCode: Int,
            description: String,
            failingUrl: String
        ) {
            Log.w(TAG, "Error WebView ($errorCode) — reintentando en ${RETRY_DELAY_MS}ms")
            // El servidor Rust puede no haber levantado aún — reintenta
            scope.launch {
                delay(RETRY_DELAY_MS)
                loadTapiti()
            }
        }
    }

    override fun onDestroy() {
        stopService(Intent(this, BluetoothService::class.java))
        webView.destroy()
        super.onDestroy()
    }
}
