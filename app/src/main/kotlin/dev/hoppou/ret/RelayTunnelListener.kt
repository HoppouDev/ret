package dev.hoppou.ret

/**
 * Receives the state changes of the connection to the relay server, from any thread.
 */
interface RelayTunnelListener {

    fun onRelayTunnelConnected()

    fun onRelayTunnelDisconnected()
}
