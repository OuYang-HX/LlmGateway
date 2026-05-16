package com.oyhx.llmgateway

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.HttpURLConnection
import java.net.InetAddress
import java.net.Inet4Address
import java.net.NetworkInterface
import java.net.URL

/**
 * Gateway connection manager.
 * Handles LAN/public address detection, connectivity testing, and auto-switching.
 */
class GatewayManager(private val context: Context) {

    companion object {
        private const val TAG = "GatewayManager"
        private const val PREFS_NAME = "gateway_prefs"
        private const val KEY_LAN_URL = "lan_url"
        private const val KEY_PUBLIC_URL = "public_url"
        private const val KEY_ACTIVE_URL = "active_url"
        private const val KEY_LAN_REACHABLE = "lan_reachable"
        private const val CONNECT_TIMEOUT_MS = 3000
        private const val READ_TIMEOUT_MS = 3000
    }

    private val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    var lanUrl: String
        get() = prefs.getString(KEY_LAN_URL, "") ?: ""
        set(value) = prefs.edit().putString(KEY_LAN_URL, value).apply()

    var publicUrl: String
        get() = prefs.getString(KEY_PUBLIC_URL, "") ?: ""
        set(value) = prefs.edit().putString(KEY_PUBLIC_URL, value).apply()

    var activeUrl: String
        get() = prefs.getString(KEY_ACTIVE_URL, "") ?: ""
        set(value) = prefs.edit().putString(KEY_ACTIVE_URL, value).apply()

    var isLanReachable: Boolean
        get() = prefs.getBoolean(KEY_LAN_REACHABLE, false)
        set(value) = prefs.edit().putBoolean(KEY_LAN_REACHABLE, value).apply()

    /** Whether the user has configured at least one gateway address */
    fun isConfigured(): Boolean = lanUrl.isNotBlank() || publicUrl.isNotBlank()

    /**
     * Determine the best gateway URL to use.
     * Priority: LAN if reachable > Public > LAN (try anyway)
     */
    suspend fun resolveBestUrl(): String = withContext(Dispatchers.IO) {
        val lan = lanUrl.trimEnd('/')
        val pub = publicUrl.trimEnd('/')

        // If only one is configured, use it
        if (lan.isBlank() && pub.isBlank()) return@withContext ""
        if (lan.isBlank()) return@withContext pub.also { activeUrl = it }
        if (pub.isBlank()) return@withContext lan.also { activeUrl = it }

        // Both configured: prefer LAN if reachable
        val lanOk = testConnectivity(lan)
        isLanReachable = lanOk

        if (lanOk) {
            Log.i(TAG, "LAN gateway reachable: $lan")
            lan.also { activeUrl = it }
        } else {
            Log.i(TAG, "LAN unreachable, using public: $pub")
            pub.also { activeUrl = it }
        }
    }

    /**
     * Test if a URL is reachable by making a lightweight HTTP request.
     */
    suspend fun testConnectivity(url: String): Boolean = withContext(Dispatchers.IO) {
        if (url.isBlank()) return@withContext false
        try {
            val testUrl = url.trimEnd('/') + "/api/v1/dashboard/summary"
            val connection = URL(testUrl).openConnection() as HttpURLConnection
            connection.connectTimeout = CONNECT_TIMEOUT_MS
            connection.readTimeout = READ_TIMEOUT_MS
            connection.requestMethod = "GET"
            val code = connection.responseCode
            connection.disconnect()
            code in 200..399
        } catch (e: Exception) {
            Log.d(TAG, "Connectivity test failed for $url: ${e.message}")
            false
        }
    }

    /**
     * Check if currently on WiFi (more likely to be on LAN).
     */
    fun isOnWifi(): Boolean {
        val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val network = cm.activeNetwork ?: return false
        val caps = cm.getNetworkCapabilities(network) ?: return false
        return caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)
    }

    /**
     * Get the local IP address of the device.
     */
    fun getLocalIpAddress(): String? {
        try {
            val interfaces = NetworkInterface.getNetworkInterfaces() ?: return null
            for (intf in interfaces) {
                val addrs = intf.inetAddresses
                for (addr in addrs) {
                    if (addr is Inet4Address && !addr.isLoopbackAddress) {
                        return addr.hostAddress
                    }
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to get local IP", e)
        }
        return null
    }

    /**
     * Periodically re-check LAN connectivity and switch if needed.
     * Returns the current best URL.
     */
    suspend fun refreshConnectivity(): String {
        return resolveBestUrl()
    }

    fun clearConfig() {
        prefs.edit().clear().apply()
    }
}
