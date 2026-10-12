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

import android.net.VpnService
import java.io.IOException
import java.util.concurrent.TimeUnit
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

/**
 * Provide a valid [RelayTunnel], creating a new one if necessary.
 */
class RelayTunnelProvider(
    private val vpnService: VpnService,
    private val listener: RelayTunnelListener?,
) {

    private val getCurrentTunnelLock = Any() // protects getCurrentTunnel()
    private val stateLock = ReentrantLock() // protects the fields shared with invalidateTunnel()
    private val retryDelayElapsed = stateLock.newCondition()

    private var tunnel: RelayTunnel? = null // protected both by "stateLock" and "getCurrentTunnelLock"
    private var first = true // protected by "getCurrentTunnelLock"
    private var lastFailureTimestamp = 0L // protected by "stateLock"

    @Throws(IOException::class, InterruptedException::class)
    fun getCurrentTunnel(): RelayTunnel {
        /*
         * To make sure that both the sending and receiving threads use the same tunnel, we must
         * guarantee that this method may not be called several times concurrently.
         *
         * However, since it executes potentially long-running blocking calls, we still want to be
         * able to call invalidateTunnel() concurrently, which requires to protect some fields.
         *
         * Therefore, use one mutex ("getCurrentTunnelLock") to avoid concurrent calls to
         * getCurrentTunnel(), and another one ("stateLock") to protect fields shared with
         * invalidateTunnel().
         */
        synchronized(getCurrentTunnelLock) {
            val newTunnel = stateLock.withLock {
                tunnel?.let { return it }

                waitUntilNextAttemptSlot()

                // "tunnel" has not changed during waiting (only getCurrentTunnel() may write it)
                RelayTunnel.open(vpnService).also { tunnel = it }
            }

            // the first connection must either notify "connected" or "disconnected"
            val notifyDisconnectedOnError = first
            first = false
            connectTunnel(newTunnel, notifyDisconnectedOnError)
            return newTunnel
        }
    }

    @Throws(IOException::class)
    private fun connectTunnel(tunnel: RelayTunnel, notifyDisconnectedOnError: Boolean) {
        try {
            tunnel.connect()
            listener?.notifyRelayTunnelConnected()
        } catch (e: IOException) {
            touchFailure()
            if (notifyDisconnectedOnError) {
                listener?.notifyRelayTunnelDisconnected()
            }
            throw e
        }
    }

    /**
     * Close the current tunnel, only if it is `tunnelToInvalidate` (or unconditionally if it is
     * `null`).
     *
     * @param tunnelToInvalidate the tunnel to invalidate
     */
    fun invalidateTunnel(tunnelToInvalidate: Tunnel? = null) {
        stateLock.withLock {
            val current = tunnel ?: return
            if (tunnelToInvalidate != null && tunnelToInvalidate !== current) {
                return
            }
            touchFailure()
            current.close()
            tunnel = null
            listener?.notifyRelayTunnelDisconnected()
        }
    }

    private fun touchFailure() {
        stateLock.withLock {
            lastFailureTimestamp = System.currentTimeMillis()
        }
    }

    /** Must be called with `stateLock` held, which is released while waiting. */
    @Throws(InterruptedException::class)
    private fun waitUntilNextAttemptSlot() {
        if (first) {
            // do not wait on first attempt
            return
        }
        var delay = lastFailureTimestamp + DELAY_BETWEEN_ATTEMPTS_MS - System.currentTimeMillis()
        while (delay > 0) {
            retryDelayElapsed.await(delay, TimeUnit.MILLISECONDS)
            delay = lastFailureTimestamp + DELAY_BETWEEN_ATTEMPTS_MS - System.currentTimeMillis()
        }
    }

    private companion object {
        const val DELAY_BETWEEN_ATTEMPTS_MS = 5000L
    }
}
