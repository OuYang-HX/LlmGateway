package com.oyhx.llmgateway

import android.annotation.SuppressLint
import android.content.Intent
import android.os.Bundle
import android.view.Menu
import android.view.MenuItem
import android.view.View
import android.webkit.ConsoleMessage
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import android.widget.ProgressBar
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.lifecycleScope
import androidx.swiperefreshlayout.widget.SwipeRefreshLayout
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MainActivity : AppCompatActivity() {

    internal lateinit var gatewayManager: GatewayManager
    private lateinit var webView: WebView
    private lateinit var progressBar: ProgressBar
    private lateinit var swipeRefreshLayout: SwipeRefreshLayout
    private lateinit var connectionBadge: TextView
    private lateinit var errorOverlay: View

    private var connectivityCheckJob: Job? = null
    internal var currentUrl: String = ""

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        gatewayManager = GatewayManager(this)

        // Find views
        webView = findViewById(R.id.webView)
        progressBar = findViewById(R.id.progressBar)
        swipeRefreshLayout = findViewById(R.id.swipeRefreshLayout)
        connectionBadge = findViewById(R.id.connectionBadge)
        errorOverlay = findViewById(R.id.errorOverlay)

        // Setup toolbar
        setSupportActionBar(findViewById(R.id.toolbar))
        supportActionBar?.title = getString(R.string.app_name)

        // Setup WebView
        setupWebView()

        // Setup SwipeRefreshLayout
        swipeRefreshLayout.setOnRefreshListener {
            loadGateway()
        }
        swipeRefreshLayout.setColorSchemeResources(
            android.R.color.holo_blue_bright,
            android.R.color.holo_green_light,
            android.R.color.holo_orange_light
        )

        // Setup error overlay retry button
        errorOverlay.findViewById<View>(R.id.btnRetry)?.setOnClickListener {
            errorOverlay.visibility = View.GONE
            loadGateway()
        }

        // Check configuration and load
        if (!gatewayManager.isConfigured()) {
            // Should not reach here normally - WelcomeActivity handles this
            // But just in case, redirect to welcome
            startActivity(Intent(this, WelcomeActivity::class.java))
            finish()
        } else {
            loadGateway()
        }
    }

    override fun onResume() {
        super.onResume()
        if (gatewayManager.isConfigured() && currentUrl.isEmpty()) {
            loadGateway()
        }
        startConnectivityMonitoring()
    }

    override fun onPause() {
        super.onPause()
        stopConnectivityMonitoring()
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun setupWebView() {
        webView.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            databaseEnabled = true
            allowFileAccess = false
            allowContentAccess = false
            loadWithOverviewMode = true
            useWideViewPort = true
            builtInZoomControls = true
            displayZoomControls = false
            setSupportZoom(true)
            cacheMode = android.webkit.WebSettings.LOAD_DEFAULT
            mixedContentMode = android.webkit.WebSettings.MIXED_CONTENT_NEVER_ALLOW
            userAgentString = userAgentString + " LlmGatewayApp/1.0"
        }

        webView.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(
                view: WebView?,
                request: WebResourceRequest?
            ): Boolean {
                // Only allow loading from the configured gateway
                val url = request?.url?.toString() ?: return false
                val allowedHosts = listOfNotNull(
                    gatewayManager.lanUrl.toHttpHost(),
                    gatewayManager.publicUrl.toHttpHost()
                ).filter { it.isNotEmpty() }

                // Allow if it matches one of our gateways
                val requestHost = url.toHttpHost()
                if (allowedHosts.any { requestHost.contains(it, ignoreCase = true) }) {
                    return false // Load in WebView
                }

                // Open external links in browser
                val intent = Intent(Intent.ACTION_VIEW, android.net.Uri.parse(url))
                startActivity(intent)
                return true
            }

            override fun onPageFinished(view: WebView?, url: String?) {
                super.onPageFinished(view, url)
                progressBar.visibility = View.GONE
                swipeRefreshLayout.isRefreshing = false
                errorOverlay.visibility = View.GONE
            }

            override fun onReceivedError(
                view: WebView?,
                errorCode: Int,
                description: String?,
                failingUrl: String?
            ) {
                super.onReceivedError(view, errorCode, description, failingUrl)
                if (errorCode == ERROR_HOST_LOOKUP || errorCode == ERROR_CONNECT ||
                    errorCode == ERROR_TIMEOUT
                ) {
                    showErrorOverlay()
                }
            }
        }

        webView.webChromeClient = object : WebChromeClient() {
            override fun onConsoleMessage(consoleMessage: ConsoleMessage?): Boolean {
                android.util.Log.d(
                    "WebView",
                    "${consoleMessage?.messageLevel()?.name}: ${consoleMessage?.message()}"
                )
                return true
            }

            override fun onProgressChanged(view: WebView?, newProgress: Int) {
                if (newProgress < 100) {
                    progressBar.visibility = View.VISIBLE
                } else {
                    progressBar.visibility = View.GONE
                }
            }
        }

        // Add JavaScript interface for native integration
        webView.addJavascriptInterface(WebAppInterface(this), "AndroidApp")
    }

    private fun loadGateway() {
        if (!gatewayManager.isConfigured()) {
            openSettings()
            return
        }

        lifecycleScope.launch {
            progressBar.visibility = View.VISIBLE
            errorOverlay.visibility = View.GONE

            val url = gatewayManager.resolveBestUrl()
            if (url.isBlank()) {
                showErrorOverlay()
                return@launch
            }

            currentUrl = url
            updateConnectionBadge()

            withContext(Dispatchers.Main) {
                webView.loadUrl(url)
            }
        }
    }

    private fun updateConnectionBadge() {
        val isLan = gatewayManager.isLanReachable && gatewayManager.lanUrl.isNotBlank()
        connectionBadge.text = if (isLan) "🏠 局域网" else "🌐 公网"
        connectionBadge.setBackgroundColor(
            if (isLan) getColor(R.color.lan_green) else getColor(R.color.public_blue)
        )
        connectionBadge.visibility = View.VISIBLE
    }

    private fun showErrorOverlay() {
        progressBar.visibility = View.GONE
        swipeRefreshLayout.isRefreshing = false
        errorOverlay.visibility = View.VISIBLE
    }

    private fun startConnectivityMonitoring() {
        connectivityCheckJob?.cancel()
        connectivityCheckJob = lifecycleScope.launch {
            while (isActive) {
                delay(30_000) // Check every 30 seconds
                if (!gatewayManager.isConfigured()) continue

                val newUrl = gatewayManager.refreshConnectivity()
                if (newUrl != currentUrl && newUrl.isNotBlank()) {
                    // Gateway switched (LAN <-> Public)
                    withContext(Dispatchers.Main) {
                        currentUrl = newUrl
                        updateConnectionBadge()
                        webView.loadUrl(newUrl)
                        Toast.makeText(
                            this@MainActivity,
                            if (gatewayManager.isLanReachable) "已切换到局域网连接" else "已切换到公网连接",
                            Toast.LENGTH_SHORT
                        ).show()
                    }
                } else if (newUrl == currentUrl) {
                    // Same gateway, just update badge
                    withContext(Dispatchers.Main) {
                        updateConnectionBadge()
                    }
                }
            }
        }
    }

    private fun stopConnectivityMonitoring() {
        connectivityCheckJob?.cancel()
        connectivityCheckJob = null
    }

    override fun onCreateOptionsMenu(menu: Menu): Boolean {
        menuInflater.inflate(R.menu.main_menu, menu)
        return true
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        return when (item.itemId) {
            R.id.action_settings -> {
                openSettings()
                true
            }
            R.id.action_switch_gateway -> {
                showSwitchGatewayDialog()
                true
            }
            R.id.action_refresh -> {
                loadGateway()
                true
            }
            else -> super.onOptionsItemSelected(item)
        }
    }

    internal fun openSettings() {
        val intent = Intent(this, SettingsActivity::class.java)
        startActivity(intent)
    }

    private fun showSwitchGatewayDialog() {
        val options = mutableListOf<String>()
        if (gatewayManager.lanUrl.isNotBlank()) {
            options.add("🏠 局域网: ${gatewayManager.lanUrl}")
        }
        if (gatewayManager.publicUrl.isNotBlank()) {
            options.add("🌐 公网: ${gatewayManager.publicUrl}")
        }
        options.add("🔄 自动选择")

        if (options.size <= 1) {
            Toast.makeText(this, "请先在设置中配置网关地址", Toast.LENGTH_SHORT).show()
            return
        }

        AlertDialog.Builder(this)
            .setTitle("切换网关连接")
            .setItems(options.toTypedArray()) { _, which ->
                lifecycleScope.launch {
                    val url = when (which) {
                        0 -> if (gatewayManager.lanUrl.isNotBlank()) gatewayManager.lanUrl.trimEnd('/') else gatewayManager.resolveBestUrl()
                        1 -> if (gatewayManager.publicUrl.isNotBlank() && gatewayManager.lanUrl.isNotBlank()) gatewayManager.publicUrl.trimEnd('/') else gatewayManager.resolveBestUrl()
                        else -> gatewayManager.resolveBestUrl()
                    }
                    currentUrl = url
                    updateConnectionBadge()
                    webView.loadUrl(url)
                }
            }
            .show()
    }

    @Deprecated("Use OnBackPressedDispatcher instead")
    override fun onBackPressed() {
        if (webView.canGoBack()) {
            webView.goBack()
        } else {
            @Suppress("DEPRECATION")
            super.onBackPressed()
        }
    }

    /**
     * Extract host from URL string for comparison.
     */
    private fun String.toHttpHost(): String {
        return try {
            val url = java.net.URL(this.trimEnd('/'))
            url.host + if (url.port != -1 && url.port != url.defaultPort) ":${url.port}" else ""
        } catch (e: Exception) {
            ""
        }
    }
}

/**
 * JavaScript interface for native-WebView communication.
 */
class WebAppInterface(private val activity: MainActivity) {
    @android.webkit.JavascriptInterface
    fun getActiveGatewayUrl(): String {
        return activity.currentUrl
    }

    @android.webkit.JavascriptInterface
    fun isLanConnection(): Boolean {
        return activity.gatewayManager.isLanReachable
    }

    @android.webkit.JavascriptInterface
    fun openAppSettings() {
        activity.runOnUiThread {
            activity.openSettings()
        }
    }
}
