package dev.hoppou.ret

import android.app.Activity
import android.content.Intent
import android.net.VpnService
import android.os.Bundle
import android.util.Log

/**
 * This (invisible) activity receives the [ACTION_RET_START] and [ACTION_RET_STOP] actions from
 * the command line.
 *
 * Recent versions of Android refuse to directly start a [android.app.Service] or a
 * [android.content.BroadcastReceiver], so actions are always managed by this activity.
 */
class RetActivity : Activity() {

    private var requestedConfig: VpnConfiguration? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        handleIntent(intent)
    }

    private fun handleIntent(intent: Intent) {
        val action = intent.action
        Log.d(TAG, "Received request $action")
        val finish = when (action) {
            ACTION_RET_START -> startRet(createConfig(intent))
            ACTION_RET_STOP -> {
                RetService.stop(this)
                true
            }
            else -> true
        }
        if (finish) {
            finish()
        }
    }

    /** Start the VPN, return whether the activity may finish now */
    private fun startRet(config: VpnConfiguration): Boolean {
        val vpnIntent = VpnService.prepare(this)
        if (vpnIntent == null) {
            Log.d(TAG, "VPN was already authorized")
            // we got the permission, start the service now
            RetService.start(this, config)
            return true
        }

        Log.w(TAG, "VPN requires the authorization from the user, requesting...")
        requestedConfig = config
        startActivityForResult(vpnIntent, VPN_REQUEST_CODE)
        return false // do not finish now
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        val config = requestedConfig
        if (requestCode == VPN_REQUEST_CODE && resultCode == RESULT_OK && config != null) {
            RetService.start(this, config)
        }
        requestedConfig = null
        finish()
    }

    companion object {

        private const val TAG = "RetActivity"

        const val ACTION_RET_START = "dev.hoppou.ret.START"
        const val ACTION_RET_STOP = "dev.hoppou.ret.STOP"

        const val EXTRA_DNS_SERVERS = "dnsServers"
        const val EXTRA_ROUTES = "routes"

        private const val VPN_REQUEST_CODE = 0

        private fun createConfig(intent: Intent): VpnConfiguration {
            val dnsServers = intent.getStringArrayExtra(EXTRA_DNS_SERVERS).orEmpty()
            val routes = intent.getStringArrayExtra(EXTRA_ROUTES).orEmpty()
            return VpnConfiguration(Net.toInetAddresses(dnsServers), Net.toCIDRs(routes))
        }
    }
}
