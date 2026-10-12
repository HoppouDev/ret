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

import android.os.Parcel
import android.os.Parcelable
import java.net.InetAddress
import java.net.UnknownHostException

class VpnConfiguration(
    val dnsServers: List<InetAddress> = emptyList(),
    val routes: List<CIDR> = emptyList(),
) : Parcelable {

    override fun writeToParcel(dest: Parcel, flags: Int) {
        dest.writeInt(dnsServers.size)
        for (address in dnsServers) {
            dest.writeByteArray(address.address)
        }
        dest.writeTypedList(routes)
    }

    override fun describeContents(): Int = 0

    companion object {
        @JvmField
        val CREATOR = object : Parcelable.Creator<VpnConfiguration> {
            override fun createFromParcel(source: Parcel): VpnConfiguration {
                val dnsServers = try {
                    List(source.readInt()) { InetAddress.getByAddress(source.createByteArray()) }
                } catch (e: UnknownHostException) {
                    throw AssertionError("Invalid address", e)
                }
                val routes = source.createTypedArrayList(CIDR.CREATOR).orEmpty()
                return VpnConfiguration(dnsServers, routes)
            }

            override fun newArray(size: Int): Array<VpnConfiguration?> = arrayOfNulls(size)
        }
    }
}
