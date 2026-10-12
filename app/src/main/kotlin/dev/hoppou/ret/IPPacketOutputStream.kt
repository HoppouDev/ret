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

import android.util.Log
import java.io.IOException
import java.io.OutputStream
import java.nio.ByteBuffer

/**
 * Wrapper for writing one IP packet at a time to an [OutputStream].
 */
class IPPacketOutputStream(private val target: OutputStream) : OutputStream() {

    // must always accept 1 full packet + any partial packet
    private val buffer: ByteBuffer = ByteBuffer.allocate(2 * MAX_IP_PACKET_LENGTH)

    @Throws(IOException::class)
    override fun close() {
        target.close()
    }

    @Throws(IOException::class)
    override fun flush() {
        target.flush()
    }

    @Throws(IOException::class)
    override fun write(b: ByteArray, off: Int, len: Int) {
        if (len > MAX_IP_PACKET_LENGTH) {
            throw IOException("IPPacketOutputStream does not support writing more than one packet at a time")
        }
        // by design, the buffer must always have enough space for one packet
        if (BuildConfig.DEBUG && len > buffer.remaining()) {
            Log.e(TAG, "$len must be <= than ${buffer.remaining()}")
            Log.e(TAG, buffer.toString())
            throw AssertionError("Buffer is unexpectedly full")
        }
        buffer.put(b, off, len)
        buffer.flip()
        sink()
        buffer.compact()
    }

    @Throws(IOException::class)
    override fun write(b: Int) {
        if (!buffer.hasRemaining()) {
            throw IOException("IPPacketOutputStream buffer is full")
        }
        buffer.put(b.toByte())
        buffer.flip()
        sink()
        buffer.compact()
    }

    @Throws(IOException::class)
    private fun sink() {
        // sink all packets
        while (sinkPacket()) {
            // continue
        }
    }

    @Throws(IOException::class)
    private fun sinkPacket(): Boolean {
        val version = readPacketVersion(buffer)
        if (version == -1) {
            // no packet at all
            return false
        }
        if (version != 4) {
            Log.e(TAG, "Unsupported packet received, IP version is:$version")
            discardBuffer()
            return false
        }
        val packetLength = readPacketLength(buffer)
        if (packetLength == -1) {
            // the length field is not received yet
            return false
        }
        if (packetLength < MIN_IP_PACKET_LENGTH) {
            // a length of 0 would never consume anything, and loop forever
            Log.e(TAG, "Invalid packet received, length is:$packetLength")
            discardBuffer()
            return false
        }
        if (packetLength > buffer.remaining()) {
            // the packet is not complete yet
            return false
        }

        target.write(buffer.array(), buffer.arrayOffset() + buffer.position(), packetLength)
        buffer.position(buffer.position() + packetLength)
        return true
    }

    private fun discardBuffer() {
        Log.d(TAG, "Clearing buffer")
        // the buffer is in read mode: consume everything, so that compact() leaves it empty
        buffer.position(buffer.limit())
    }

    companion object {

        private const val TAG = "IPPacketOutputStream"

        private const val MAX_IP_PACKET_LENGTH = 1 shl 16 // packet length is stored on 16 bits

        private const val MIN_IP_PACKET_LENGTH = 20 // an IPv4 header without options

        /**
         * Read the packet IP version, assuming that an IP packet is stored at the buffer position.
         *
         * @param buffer the buffer
         * @return the IP version, or `-1` if not available
         */
        fun readPacketVersion(buffer: ByteBuffer): Int {
            if (!buffer.hasRemaining()) {
                // buffer is empty
                return -1
            }
            // version is stored in the 4 first bits
            val versionAndIHL = buffer.get(buffer.position()).toInt()
            return (versionAndIHL and 0xf0) shr 4
        }

        /**
         * Read the packet length, assuming that an IP packet is stored at the buffer position.
         *
         * @param buffer the buffer
         * @return the packet length, or `-1` if not available
         */
        fun readPacketLength(buffer: ByteBuffer): Int {
            if (buffer.limit() < buffer.position() + 4) {
                // buffer does not even contain the length field
                return -1
            }
            // packet length is 16 bits starting at offset 2
            return Binary.unsigned(buffer.getShort(buffer.position() + 2))
        }
    }
}
