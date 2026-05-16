package com.oyhx.llmgateway

import android.content.Intent
import android.os.Bundle
import android.widget.Button
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity

class WelcomeActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_welcome)

        val gatewayManager = GatewayManager(this)

        // If already configured, skip to main
        if (gatewayManager.isConfigured()) {
            startActivity(Intent(this, MainActivity::class.java))
            finish()
            return
        }

        // Show local IP hint
        val localIp = gatewayManager.getLocalIpAddress()
        if (localIp != null) {
            findViewById<TextView>(R.id.tvDeviceIp).text = "当前设备IP: $localIp"
        }

        // Setup button
        findViewById<Button>(R.id.btnSetup).setOnClickListener {
            startActivity(Intent(this, SettingsActivity::class.java))
        }

        // Skip button (use without gateway)
        findViewById<Button>(R.id.btnSkip).setOnClickListener {
            // Mark as "skipped" so we don't show welcome again, but don't set any URL
            // User can configure later from settings
            startActivity(Intent(this, MainActivity::class.java))
            finish()
        }
    }
}