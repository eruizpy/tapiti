package com.tapiti.obd

import android.Manifest
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Bundle
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.BaseAdapter
import android.widget.ListView
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.core.app.ActivityCompat

class DevicePickerActivity : AppCompatActivity() {

    companion object {
        private const val PREFS_NAME = "tapiti_prefs"
        private const val KEY_DEVICE_ADDRESS = "bt_device_address"
        private const val REQUEST_BT_CONNECT = 100

        fun getSavedAddress(context: Context): String? {
            return context.getSharedPreferences(PREFS_NAME, MODE_PRIVATE)
                .getString(KEY_DEVICE_ADDRESS, null)
        }
    }

    private lateinit var listView: ListView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        if (!hasBtPermission()) {
            ActivityCompat.requestPermissions(
                this,
                arrayOf(Manifest.permission.BLUETOOTH_CONNECT),
                REQUEST_BT_CONNECT
            )
            return
        }

        showDeviceList()
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == REQUEST_BT_CONNECT) {
            if (grantResults.firstOrNull() == PackageManager.PERMISSION_GRANTED) {
                showDeviceList()
            } else {
                Toast.makeText(this, "Permiso Bluetooth requerido", Toast.LENGTH_LONG).show()
                finish()
            }
        }
    }

    private fun showDeviceList() {
        val adapter = BluetoothAdapter.getDefaultAdapter()
        if (adapter == null || !adapter.isEnabled) {
            Toast.makeText(this, "Bluetooth no disponible", Toast.LENGTH_LONG).show()
            finish()
            return
        }

        val devices = adapter.bondedDevices.toList()
        if (devices.isEmpty()) {
            Toast.makeText(this, "Sin dispositivos emparejados", Toast.LENGTH_LONG).show()
            finish()
            return
        }

        val header = TextView(this).apply {
            text = "Seleccionar adaptador OBD-II"
            textSize = 20f
            setPadding(48, 48, 48, 24)
            setTextColor(0xFFFFFFFF.toInt())
        }

        listView = ListView(this).apply {
            addHeaderView(header, null, false)
            setBackgroundColor(0xFF1A1A2E.toInt())
            this.adapter = DeviceAdapter(devices)
            setOnItemClickListener { _, _, position, _ ->
                val index = position - 1 // header offset
                if (index in devices.indices) {
                    saveAndLaunch(devices[index])
                }
            }
        }

        setContentView(listView)
    }

    private fun saveAndLaunch(device: BluetoothDevice) {
        getSharedPreferences(PREFS_NAME, MODE_PRIVATE)
            .edit()
            .putString(KEY_DEVICE_ADDRESS, device.address)
            .apply()

        startActivity(Intent(this, WebViewActivity::class.java))
        finish()
    }

    private fun hasBtPermission(): Boolean {
        return ActivityCompat.checkSelfPermission(
            this, Manifest.permission.BLUETOOTH_CONNECT
        ) == PackageManager.PERMISSION_GRANTED
    }

    private inner class DeviceAdapter(
        private val devices: List<BluetoothDevice>
    ) : BaseAdapter() {

        override fun getCount(): Int = devices.size
        override fun getItem(position: Int): BluetoothDevice = devices[position]
        override fun getItemId(position: Int): Long = position.toLong()

        override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
            val device = devices[position]
            val name = device.name ?: "Desconocido"
            val address = device.address

            return TextView(this@DevicePickerActivity).apply {
                text = "$name\n$address"
                textSize = 16f
                setPadding(48, 32, 48, 32)
                setTextColor(0xFFE0E0E0.toInt())
                gravity = Gravity.START
            }
        }
    }
}
