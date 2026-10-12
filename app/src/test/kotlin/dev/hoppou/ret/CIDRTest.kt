package dev.hoppou.ret

import org.junit.Assert.assertEquals
import org.junit.Test
import java.net.InetAddress

class CIDRTest {

    @Test
    fun parseAddressAndPrefix() {
        val cidr = CIDR.parse("192.168.1.0/24")
        assertEquals(InetAddress.getByName("192.168.1.0"), cidr.address)
        assertEquals(24, cidr.prefixLength)
        assertEquals("192.168.1.0/24", cidr.toString())
    }

    @Test
    fun singleAddressWithoutPrefix() {
        assertEquals(CIDR(InetAddress.getByName("10.0.2.2"), 32), CIDR.parse("10.0.2.2"))
    }

    @Test(expected = InvalidCIDRException::class)
    fun rejectAnInvalidPrefix() {
        CIDR.parse("10.0.0.0/eight")
    }

    @Test
    fun parseTheRoutesOfTheCommandLine() {
        assertEquals(
            listOf(CIDR.parse("0.0.0.0/0"), CIDR.parse("10.0.0.0/8")),
            Net.toCIDRs(arrayOf("0.0.0.0/0", "10.0.0.0/8")),
        )
    }

    @Test(expected = IllegalArgumentException::class)
    fun rejectAnInvalidRoute() {
        Net.toCIDRs(arrayOf("10.0.0.0/8", "10.0.0.0/x"))
    }
}
