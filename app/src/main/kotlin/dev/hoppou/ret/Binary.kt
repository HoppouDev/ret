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

object Binary {

    private const val MAX_STRING_PACKET_SIZE = 20

    fun unsigned(value: Short): Int = value.toInt() and 0xffff

    fun unsigned(value: Int): Long = value.toLong() and 0xffffffffL

    fun buildPacketString(data: ByteArray, len: Int): String = buildString {
        val limit = minOf(MAX_STRING_PACKET_SIZE, len)
        append('[').append(len).append(" bytes] ")
        for (i in 0 until limit) {
            if (i != 0) {
                append(if (i % 4 == 0) "  " else " ")
            }
            append("%02X".format(data[i].toInt() and 0xff))
        }
        if (limit < len) {
            append(" ... +").append(len - limit).append(" bytes")
        }
    }
}
