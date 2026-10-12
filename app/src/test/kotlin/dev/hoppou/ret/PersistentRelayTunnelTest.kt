package dev.hoppou.ret

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.IOException
import java.io.InterruptedIOException

class PersistentRelayTunnelTest {

    private fun persistentTunnel(factory: TunnelFactory) =
        PersistentRelayTunnel(RelayTunnelProvider(null, retryDelayMs = 0, openTunnel = factory))

    @Test
    fun sendThroughANewTunnelAfterAFailure() {
        val broken = FakeTunnel(sendError = IOException("broken pipe"))
        val working = FakeTunnel()
        val tunnel = persistentTunnel(TunnelFactory(broken, working))

        tunnel.send(byteArrayOf(1, 2, 3, 4), 3)

        assertTrue("The failing tunnel must be closed", broken.closed)
        assertEquals(1, working.sent.size)
        assertArrayEquals(byteArrayOf(1, 2, 3), working.sent[0])
    }

    @Test
    fun receiveFromANewTunnelAfterEndOfStream() {
        val ended = FakeTunnel(receives = listOf(null))
        val working = FakeTunnel(receives = listOf(byteArrayOf(4, 5)))
        val tunnel = persistentTunnel(TunnelFactory(ended, working))

        val buffer = ByteArray(16)
        assertEquals(2, tunnel.receive(buffer))
        assertArrayEquals(byteArrayOf(4, 5), buffer.copyOf(2))
        assertTrue(ended.closed)
    }

    @Test(expected = InterruptedIOException::class)
    fun failToSendOnceClosed() {
        val factory = TunnelFactory(FakeTunnel())
        val tunnel = persistentTunnel(factory)
        tunnel.close()
        try {
            tunnel.send(byteArrayOf(1), 1)
        } finally {
            assertEquals("A closed tunnel must not reconnect", 0, factory.opened)
        }
    }

    @Test
    fun closeUnblocksAPendingReceive() {
        val connected = FakeTunnel()
        val tunnel = persistentTunnel(TunnelFactory(connected))
        var result: Throwable? = null
        val receiving = Thread {
            result = runCatching { tunnel.receive(ByteArray(16)) }.exceptionOrNull()
        }
        receiving.start()
        Thread.sleep(100)

        tunnel.close()
        receiving.join(2000)

        assertTrue("receive() is still blocked", !receiving.isAlive)
        assertTrue("Unexpected $result", result is InterruptedIOException)
    }
}
