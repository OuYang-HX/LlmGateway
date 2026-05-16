package com.oyhx.llmgateway

import android.content.Intent
import android.os.Bundle
import android.view.MenuItem
import android.widget.Button
import android.widget.ProgressBar
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class SettingsActivity : AppCompatActivity() {

    private lateinit var gatewayManager: GatewayManager
    private lateinit var etLanUrl: com.google.android.material.textfield.TextInputEditText
    private lateinit var etPublicUrl: com.google.android.material.textfield.TextInputEditText
    private lateinit var btnTestLan: Button
    private lateinit var btnTestPublic: Button
    private lateinit var tvLanStatus: TextView
    private lateinit var tvPublicStatus: TextView
    private lateinit var btnSave: Button
    private lateinit var progressBar: ProgressBar

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_settings)

        gatewayManager = GatewayManager(this)

        // Setup toolbar
        setSupportActionBar(findViewById(R.id.toolbar))
        supportActionBar?.setDisplayHomeAsUpEnabled(true)
        supportActionBar?.title = getString(R.string.settings_title)

        // Find views
        etLanUrl = findViewById(R.id.etLanUrl)
        etPublicUrl = findViewById(R.id.etPublicUrl)
        btnTestLan = findViewById(R.id.btnTestLan)
        btnTestPublic = findViewById(R.id.btnTestPublic)
        tvLanStatus = findViewById(R.id.tvLanStatus)
        tvPublicStatus = findViewById(R.id.tvPublicStatus)
        btnSave = findViewById(R.id.btnSave)
        progressBar = findViewById(R.id.progressBar)

        // Load saved values
        etLanUrl.setText(gatewayManager.lanUrl)
        etPublicUrl.setText(gatewayManager.publicUrl)

        // Show local IP hint
        val localIp = gatewayManager.getLocalIpAddress()
        if (localIp != null) {
            val hint = "当前设备IP: $localIp"
            findViewById<TextView>(R.id.tvLocalIp).text = hint
        }

        // Test LAN button
        btnTestLan.setOnClickListener { testLanConnection() }

        // Test Public button
        btnTestPublic.setOnClickListener { testPublicConnection() }

        // Save button
        btnSave.setOnClickListener { saveSettings() }
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        if (item.itemId == android.R.id.home) {
            finish()
            return true
        }
        return super.onOptionsItemSelected(item)
    }

    private fun testLanConnection() {
        val url = etLanUrl.text.toString().trim()
        if (url.isBlank()) {
            Toast.makeText(this, "请输入局域网地址", Toast.LENGTH_SHORT).show()
            return
        }

        btnTestLan.isEnabled = false
        tvLanStatus.text = "测试中..."
        tvLanStatus.setTextColor(getColor(R.color.text_secondary))

        lifecycleScope.launch {
            val reachable = gatewayManager.testConnectivity(url)
            withContext(Dispatchers.Main) {
                btnTestLan.isEnabled = true
                if (reachable) {
                    tvLanStatus.text = "✅ 连接成功"
                    tvLanStatus.setTextColor(getColor(R.color.lan_green))
                } else {
                    tvLanStatus.text = "❌ 连接失败"
                    tvLanStatus.setTextColor(getColor(android.R.color.holo_red_dark))
                }
            }
        }
    }

    private fun testPublicConnection() {
        val url = etPublicUrl.text.toString().trim()
        if (url.isBlank()) {
            Toast.makeText(this, "请输入公网地址", Toast.LENGTH_SHORT).show()
            return
        }

        btnTestPublic.isEnabled = false
        tvPublicStatus.text = "测试中..."
        tvPublicStatus.setTextColor(getColor(R.color.text_secondary))

        lifecycleScope.launch {
            val reachable = gatewayManager.testConnectivity(url)
            withContext(Dispatchers.Main) {
                btnTestPublic.isEnabled = true
                if (reachable) {
                    tvPublicStatus.text = "✅ 连接成功"
                    tvPublicStatus.setTextColor(getColor(R.color.lan_green))
                } else {
                    tvPublicStatus.text = "❌ 连接失败"
                    tvPublicStatus.setTextColor(getColor(android.R.color.holo_red_dark))
                }
            }
        }
    }

    private fun saveSettings() {
        val lanUrl = etLanUrl.text.toString().trim()
        val publicUrl = etPublicUrl.text.toString().trim()

        if (lanUrl.isBlank() && publicUrl.isBlank()) {
            Toast.makeText(this, "请至少配置一个网关地址", Toast.LENGTH_SHORT).show()
            return
        }

        // Validate URL format
        if (lanUrl.isNotBlank() && !isValidUrl(lanUrl)) {
            Toast.makeText(this, "局域网地址格式不正确", Toast.LENGTH_SHORT).show()
            return
        }
        if (publicUrl.isNotBlank() && !isValidUrl(publicUrl)) {
            Toast.makeText(this, "公网地址格式不正确", Toast.LENGTH_SHORT).show()
            return
        }

        gatewayManager.lanUrl = lanUrl
        gatewayManager.publicUrl = publicUrl

        Toast.makeText(this, "设置已保存", Toast.LENGTH_SHORT).show()

        // Navigate to main activity
        val intent = Intent(this, MainActivity::class.java)
        intent.flags = Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_NEW_TASK
        startActivity(intent)
        finish()
    }

    private fun isValidUrl(url: String): Boolean {
        return try {
            val parsed = java.net.URL(url)
            parsed.protocol in listOf("http", "https") && parsed.host.isNotBlank()
        } catch (e: Exception) {
            false
        }
    }
}
