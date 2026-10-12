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

import java.net.InetAddress
import java.net.UnknownHostException

object Net {

    fun toInetAddresses(addresses: Array<out String>): List<InetAddress> = addresses.map(::toInetAddress)

    fun toInetAddress(address: String): InetAddress =
        try {
            InetAddress.getByName(address)
        } catch (e: UnknownHostException) {
            throw IllegalArgumentException(e)
        }

    fun toInetAddress(raw: ByteArray): InetAddress =
        try {
            InetAddress.getByAddress(raw)
        } catch (e: UnknownHostException) {
            throw IllegalArgumentException(e)
        }

    fun toCIDR(cidr: String): CIDR =
        try {
            CIDR.parse(cidr)
        } catch (e: InvalidCIDRException) {
            throw IllegalArgumentException(e)
        }

    fun toCIDRs(cidrs: Array<out String>): List<CIDR> = cidrs.map(::toCIDR)
}
