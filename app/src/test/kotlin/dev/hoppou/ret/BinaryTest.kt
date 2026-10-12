package dev.hoppou.ret

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

class BinaryTest {

    @Test
    fun unsignedValues() {
        assertEquals(0xFFFF, Binary.unsigned((-1).toShort()))
        assertEquals(0x8000, Binary.unsigned(Short.MIN_VALUE))
        assertEquals(0xFFFFFFFFL, Binary.unsigned(-1))
        assertEquals(0x80000000L, Binary.unsigned(Int.MIN_VALUE))
    }

    @Test
    fun groupBytesByFour() {
        val data = byteArrayOf(0x45, 0, 0, 0x20, 0x12, 0x34, 0xAB.toByte(), 0xFF.toByte(), 1)
        assertEquals(
            "[9 bytes] 45 00 00 20  12 34 AB FF  01",
            Binary.buildPacketString(data, data.size),
        )
    }

    @Test
    fun printOnlyTheGivenLength() {
        assertEquals("[2 bytes] 01 02", Binary.buildPacketString(byteArrayOf(1, 2, 3), 2))
    }

    @Test
    fun truncateLongPackets() {
        val data = ByteArray(25) { it.toByte() }
        assertEquals(
            "[25 bytes] 00 01 02 03  04 05 06 07  08 09 0A 0B  0C 0D 0E 0F  10 11 12 13 ... +5 bytes",
            Binary.buildPacketString(data, data.size),
        )
        // exactly at the limit, nothing is omitted
        assertFalse(Binary.buildPacketString(data, 20).contains("..."))
    }
}
