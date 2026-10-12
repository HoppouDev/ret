/*
 * Copyright (C) 2018 Genymobile
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
import android.util.Log
import java.net.InetAddress
import java.net.UnknownHostException

data class CIDR(val address: InetAddress, val prefixLength: Int) : Parcelable {

    override fun toString(): String = "${address.hostAddress}/$prefixLength"

    override fun describeContents(): Int = 0

    override fun writeToParcel(dest: Parcel, flags: Int) {
        dest.writeByteArray(address.address)
        dest.writeInt(prefixLength)
    }

    companion object {

        @Throws(InvalidCIDRException::class)
        fun parse(cidr: String): CIDR {
            val slashIndex = cidr.indexOf('/')
            try {
                return if (slashIndex != -1) {
                    CIDR(
                        Net.toInetAddress(cidr.substring(0, slashIndex)),
                        cidr.substring(slashIndex + 1).toInt(),
                    )
                } else {
                    CIDR(Net.toInetAddress(cidr), 32)
                }
            } catch (e: IllegalArgumentException) {
                // includes NumberFormatException for an invalid prefix
                Log.e("Error", e.message, e)
                throw InvalidCIDRException(cidr, e)
            }
        }

        @JvmField
        val CREATOR = object : Parcelable.Creator<CIDR> {
            override fun createFromParcel(source: Parcel): CIDR {
                val address = try {
                    InetAddress.getByAddress(source.createByteArray())
                } catch (e: UnknownHostException) {
                    throw AssertionError("Invalid address", e)
                }
                return CIDR(address, source.readInt())
            }

            override fun newArray(size: Int): Array<CIDR?> = arrayOfNulls(size)
        }
    }
}
