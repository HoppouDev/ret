package dev.hoppou.ret

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import java.io.IOException
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

class RelayTunnelProviderTest {

    private val listener = RecordingListener()

    private fun provider(factory: TunnelFactory, retryDelayMs: Long = RETRY_DELAY_MS) =
        RelayTunnelProvider(listener, retryDelayMs, factory)

    @Test
    fun reuseTheConnectedTunnel() {
        val tunnel = FakeTunnel()
        val factory = TunnelFactory(tunnel)
        val provider = provider(factory)

        assertSame(tunnel, provider.getCurrentTunnel())
        assertSame(tunnel, provider.getCurrentTunnel())
        assertEquals(1, factory.opened)
        assertEquals(listOf("connected"), listener.events)
    }

    @Test
    fun reportOnlyTheFirstConnectionFailure() {
        val factory = TunnelFactory(
            FakeTunnel(connectError = IOException("refused")),
            FakeTunnel(connectError = IOException("refused")),
        )
        val provider = provider(factory, retryDelayMs = 0)

        repeat(2) {
            try {
                provider.getCurrentTunnel()
                fail("The connection should fail")
            } catch (e: IOException) {
                // expected
            }
            // the failed tunnel stays current until invalidated
            provider.invalidateTunnel()
        }
        // "disconnected" from the first failure and from each invalidation, not from the second failure
        assertEquals(listOf("disconnected", "disconnected", "disconnected"), listener.events)
    }

    @Test
    fun invalidateOnlyTheGivenTunnel() {
        val tunnel = FakeTunnel()
        val provider = provider(TunnelFactory(tunnel))
        provider.getCurrentTunnel()

        provider.invalidateTunnel(FakeTunnel())
        assertFalse("Another tunnel must not close the current one", tunnel.closed)

        provider.invalidateTunnel(tunnel)
        assertTrue(tunnel.closed)
        assertEquals(listOf("connected", "disconnected"), listener.events)
    }

    @Test
    fun invalidateWithoutTunnelDoesNothing() {
        provider(TunnelFactory()).invalidateTunnel()
        assertTrue(listener.events.isEmpty())
    }

    @Test
    fun waitBeforeReconnecting() {
        val first = FakeTunnel()
        val second = FakeTunnel()
        val provider = provider(TunnelFactory(first, second))
        provider.getCurrentTunnel()

        provider.invalidateTunnel()
        val start = System.nanoTime()
        assertSame(second, provider.getCurrentTunnel())
        val elapsedMs = TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - start)
        assertTrue("Reconnected after $elapsedMs ms", elapsedMs >= RETRY_DELAY_MS - CLOCK_SLACK_MS)
    }

    @Test
    fun invalidateWhileWaitingToReconnect() {
        val provider = provider(TunnelFactory(FakeTunnel(), FakeTunnel()), retryDelayMs = 2000)
        provider.getCurrentTunnel()
        provider.invalidateTunnel()

        // a forwarding thread waits for the retry delay
        val waiting = CountDownLatch(1)
        val reconnecting = thread {
            waiting.countDown()
            provider.getCurrentTunnel()
        }
        waiting.await()
        Thread.sleep(100)

        // the other forwarding thread must not be blocked by the wait
        val start = System.nanoTime()
        provider.invalidateTunnel()
        val elapsedMs = TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - start)
        assertTrue("invalidateTunnel() blocked for $elapsedMs ms", elapsedMs < 1000)

        reconnecting.join()
    }

    private companion object {
        const val RETRY_DELAY_MS = 300L
        // System.currentTimeMillis() may be coarser than the measured interval
        const val CLOCK_SLACK_MS = 20L
    }
}
