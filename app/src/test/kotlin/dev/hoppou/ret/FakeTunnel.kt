package dev.hoppou.ret

import java.io.IOException

/**
 * A tunnel scripted by the test: whether connect() fails, what each send() and receive() does.
 */
class FakeTunnel(
    private val connectError: IOException? = null,
    private val sendError: IOException? = null,
    receives: List<ByteArray?> = emptyList(),
) : ConnectableTunnel {

    /** Each receive() returns the next bytes, or EOF (`-1`) for `null`, then blocks until closed */
    private val receives = ArrayDeque(receives)

    val sent = mutableListOf<ByteArray>()

    @Volatile
    var closed = false
        private set

    override fun connect() {
        connectError?.let { throw it }
    }

    override fun send(packet: ByteArray, len: Int) {
        sendError?.let { throw it }
        sent += packet.copyOf(len)
    }

    override fun receive(packet: ByteArray): Int {
        if (receives.isEmpty()) {
            // like a socket with nothing to read, until it is closed
            while (!closed) {
                Thread.sleep(10)
            }
            throw IOException("closed")
        }
        val next = receives.removeFirst() ?: return -1
        next.copyInto(packet)
        return next.size
    }

    override fun close() {
        closed = true
    }
}

/** Records the notifications of a [RelayTunnelProvider] */
class RecordingListener : RelayTunnelListener {

    val events = mutableListOf<String>()

    override fun onRelayTunnelConnected() {
        synchronized(events) { events += "connected" }
    }

    override fun onRelayTunnelDisconnected() {
        synchronized(events) { events += "disconnected" }
    }
}

/** A factory handing out the given tunnels in order, recording how many were opened */
class TunnelFactory(vararg tunnels: FakeTunnel) : () -> ConnectableTunnel {

    private val tunnels = ArrayDeque(tunnels.toList())

    var opened = 0
        private set

    override fun invoke(): ConnectableTunnel {
        opened++
        return tunnels.removeFirst()
    }
}
