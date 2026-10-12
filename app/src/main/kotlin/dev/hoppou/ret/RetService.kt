/*
 * Copyright (C) 2017 Genymobile
 * Copyright (C) 2026 HoppouDev
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package dev.hoppou.ret

import android.content.Context
import android.content.Intent
import android.net.ConnectivityManager
import android.net.Network
import android.net.VpnService
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.Message
import android.os.ParcelFileDescriptor
import android.util.Log
import java.io.IOException

class RetService : VpnService() {

    private val notifier = Notifier(this)
    private val handler: Handler = RelayTunnelConnectionStateHandler(this)

    private var vpnInterface: ParcelFileDescriptor? = null
    private var forwarder: Forwarder? = null

    private val isRunning: Boolean
        get() = vpnInterface != null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val action = intent?.action
        Log.d(TAG, "Received request $action")
        when (action) {
            ACTION_START_VPN -> if (isRunning) {
                Log.d(TAG, "VPN already running, ignore START request")
            } else {
                @Suppress("DEPRECATION")
                val config = intent.getParcelableExtra<VpnConfiguration>(EXTRA_VPN_CONFIGURATION)
                startVpn(config ?: VpnConfiguration())
            }
            ACTION_CLOSE_VPN -> close()
        }
        return START_NOT_STICKY
    }

    private fun startVpn(config: VpnConfiguration) {
        notifier.start()
        val vpnInterface = setupVpn(config) ?: return
        this.vpnInterface = vpnInterface
        setAsUnderlyingNetwork()
        forwarder = Forwarder(this, vpnInterface.fileDescriptor, RelayTunnelListener(handler)).also {
            it.forward()
        }
    }

    private fun setupVpn(config: VpnConfiguration): ParcelFileDescriptor? {
        val builder = Builder()
            .addAddress(VPN_ADDRESS, 32)
            .setSession(getString(R.string.app_name))

        if (config.routes.isEmpty()) {
            // no routes defined, redirect the whole network traffic
            builder.addRoute("0.0.0.0", 0)
        } else {
            for (route in config.routes) {
                builder.addRoute(route.address, route.prefixLength)
            }
        }

        if (config.dnsServers.isEmpty()) {
            // no DNS server defined, use Google DNS
            builder.addDnsServer("8.8.8.8")
        } else {
            for (dnsServer in config.dnsServers) {
                builder.addDnsServer(dnsServer)
            }
        }

        // non-blocking by default, but FileChannel is not selectable, that's stupid!
        // so switch to synchronous I/O to avoid polling
        builder.setBlocking(true)
        builder.setMtu(MTU)

        // establish() may return null if the application is not prepared or is revoked
        return builder.establish().also {
            if (it == null) {
                Log.w(TAG, "VPN starting failed, please retry")
            }
        }
    }

    private fun setAsUnderlyingNetwork() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP_MR1) {
            // so that applications knows that network is available
            findVpnNetwork()?.let { setUnderlyingNetworks(arrayOf(it)) }
        } else {
            Log.w(TAG, "Cannot set underlying network, API version ${Build.VERSION.SDK_INT} < 22")
        }
    }

    private fun findVpnNetwork(): Network? {
        val cm = getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        @Suppress("DEPRECATION")
        return cm.allNetworks.firstOrNull { network ->
            cm.getLinkProperties(network)?.linkAddresses.orEmpty().any { it.address == VPN_ADDRESS }
        }
    }

    private fun close() {
        val vpnInterface = vpnInterface ?: return // already closed

        notifier.stop()

        try {
            forwarder?.stop()
            forwarder = null
            vpnInterface.close()
            this.vpnInterface = null
        } catch (e: IOException) {
            Log.w(TAG, "Cannot close VPN file descriptor", e)
        }
    }

    /**
     * Receives the events of the relay tunnel on the main thread, where the service runs
     *
     * A nested class rather than an inner one, so that the reference to the service is explicit.
     */
    private class RelayTunnelConnectionStateHandler(
        private val service: RetService,
    ) : Handler(Looper.getMainLooper()) {
        override fun handleMessage(message: Message) {
            if (!service.isRunning) {
                // if the VPN is not running anymore, ignore obsolete events
                return
            }
            when (message.what) {
                RelayTunnelListener.MSG_RELAY_TUNNEL_CONNECTED -> {
                    Log.d(TAG, "Relay tunnel connected")
                    service.notifier.setFailure(false)
                }
                RelayTunnelListener.MSG_RELAY_TUNNEL_DISCONNECTED -> {
                    Log.d(TAG, "Relay tunnel disconnected")
                    service.notifier.setFailure(true)
                }
            }
        }
    }

    companion object {

        const val VERBOSE = false

        private const val ACTION_START_VPN = "dev.hoppou.ret.START_VPN"
        private const val ACTION_CLOSE_VPN = "dev.hoppou.ret.CLOSE_VPN"
        private const val EXTRA_VPN_CONFIGURATION = "vpnConfiguration"

        private const val TAG = "RetService"

        private val VPN_ADDRESS = Net.toInetAddress(byteArrayOf(10, 0, 0, 2))

        // magic value: higher (like 0x8000 or 0xffff) or lower (like 1500) values show poorer
        // performances
        private const val MTU = 0x4000

        fun start(context: Context, config: VpnConfiguration) {
            val intent = Intent(context, RetService::class.java)
                .setAction(ACTION_START_VPN)
                .putExtra(EXTRA_VPN_CONFIGURATION, config)
            startService(context, intent)
        }

        fun stop(context: Context) {
            startService(context, createStopIntent(context))
        }

        fun createStopIntent(context: Context): Intent =
            Intent(context, RetService::class.java).setAction(ACTION_CLOSE_VPN)

        private fun startService(context: Context, intent: Intent) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }
    }
}
