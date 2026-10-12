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

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.nio.ByteBuffer

class IPPacketOutputStreamTest {

    private fun createMockPacket(): ByteBuffer =
        ByteBuffer.allocate(32).apply {
            writeMockPacketTo(this)
            flip()
        }

    private fun writeMockPacketTo(buffer: ByteBuffer) {
        buffer.put(((4 shl 4) or 5).toByte()) // versionAndIHL
        buffer.put(0.toByte()) // ToS
        buffer.putShort(32) // total length 20 + 8 + 4
        buffer.putInt(0) // IdFlagsFragmentOffset
        buffer.put(0.toByte()) // TTL
        buffer.put(17.toByte()) // protocol (UDP)
        buffer.putShort(0) // checksum
        buffer.putInt(0x12345678) // source address
        buffer.putInt(0x42424242) // destination address

        buffer.putShort(1234) // source port
        buffer.putShort(5678) // destination port
        buffer.putShort(12) // length
        buffer.putShort(0) // checksum

        buffer.putInt(0x11223344) // payload
    }

    @Test
    fun simplePacket() {
        val bos = ByteArrayOutputStream()
        val pos = IPPacketOutputStream(bos)

        val rawPacket = createMockPacket().array()

        pos.write(rawPacket, 0, 14)
        assertEquals("Partial packet should not be written", 0, bos.size())

        pos.write(rawPacket, 14, 14)
        assertEquals("Partial packet should not be written", 0, bos.size())

        pos.write(rawPacket, 28, 4)
        assertEquals("Complete packet should be written", 32, bos.size())

        assertArrayEquals("Resulting array must be identical", rawPacket, bos.toByteArray())
    }

    @Test
    fun severalPacketsAtOnce() {
        class CapturingOutputStream : ByteArrayOutputStream() {
            var packetCount = 0

            override fun write(b: ByteArray, off: Int, len: Int) {
                super.write(b, off, len)
                ++packetCount
            }
        }

        val cos = CapturingOutputStream()
        val pos = IPPacketOutputStream(cos)

        val buffer = ByteBuffer.allocate(3 * 32)
        repeat(3) { writeMockPacketTo(buffer) }
        val rawPackets = buffer.array()

        pos.write(rawPackets, 0, 70) // 2 packets + 6 bytes
        assertEquals("Exactly 2 packets should have been written", 64, cos.size())
        assertEquals("Packets should be written individually to the target", 2, cos.packetCount)

        pos.write(rawPackets, 70, 26)
        assertEquals("Exactly 3 packets should have been written", 96, cos.size())
        assertEquals("Packets should be written individually to the target", 3, cos.packetCount)
    }

    @Test
    fun writeOneByteAtATime() {
        val bos = ByteArrayOutputStream()
        val pos = IPPacketOutputStream(bos)
        val rawPacket = createMockPacket().array()

        for ((i, byte) in rawPacket.withIndex()) {
            pos.write(byte.toInt())
            val expected = if (i == rawPacket.lastIndex) 32 else 0
            assertEquals("After ${i + 1} bytes", expected, bos.size())
        }
        assertArrayEquals(rawPacket, bos.toByteArray())
    }

    @Test(expected = IOException::class)
    fun rejectMoreThanOnePacketAtOnce() {
        IPPacketOutputStream(ByteArrayOutputStream()).write(ByteArray(0x10001), 0, 0x10001)
    }

    @Test
    fun readHeaderFields() {
        val packet = createMockPacket()
        assertEquals(4, IPPacketOutputStream.readPacketVersion(packet))
        assertEquals(32, IPPacketOutputStream.readPacketLength(packet))

        // the length is unsigned, a packet of 0xFFFF bytes must not read as negative
        val big = ByteBuffer.allocate(4).put(0x45).put(0).putShort(0xFFFF.toShort()).apply { flip() }
        assertEquals(0xFFFF, IPPacketOutputStream.readPacketLength(big))
    }

    @Test
    fun headerFieldsUnavailableInAPartialPacket() {
        assertEquals(-1, IPPacketOutputStream.readPacketVersion(ByteBuffer.allocate(0)))
        val partial = ByteBuffer.allocate(3).put(0x45).put(0).put(0).apply { flip() }
        assertEquals(4, IPPacketOutputStream.readPacketVersion(partial))
        assertEquals(-1, IPPacketOutputStream.readPacketLength(partial))
    }
}
