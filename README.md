# ret

This project provides **reverse tethering** over `adb` for Android: it
allows devices to use the internet connection of the computer they are plugged
on. It does not require any _root_ access (neither on the device nor on the
computer). It works on _GNU/Linux_ and _Windows_.

ret is a fork of [Gnirehtet] by Genymobile.

[Gnirehtet]: https://github.com/Genymobile/gnirehtet

Currently, it relays [TCP] and [UDP] over [IPv4] traffic, but it does not
support [IPv6] (yet?).

[TCP]: https://en.wikipedia.org/wiki/Transmission_Control_Protocol
[UDP]: https://fr.wikipedia.org/wiki/User_Datagram_Protocol
[IPv4]: https://en.wikipedia.org/wiki/IPv4
[IPv6]: https://en.wikipedia.org/wiki/IPv6


## Requirements

The Android application requires at least API 21 (Android 5.0).

### adb

You need a recent version of [adb] (where `adb reverse` is implemented, it
works with 1.0.36).

It is available in the [Android SDK platform tools][platform-tools].

On Debian-based distros, you can alternatively install the package
`android-tools-adb`.

On Windows, if you need `adb` only for this application, just download the
[platform-tools][platform-tools-windows] and extract the following files to the
_ret_ directory:
 - `adb.exe`
 - `AdbWinApi.dll`
 - `AdbWinUsbApi.dll`

Make sure you [enabled adb debugging][enable-adb] on your device(s).

[adb]: https://developer.android.com/studio/command-line/adb.html
[enable-adb]: https://developer.android.com/studio/command-line/adb.html#Enabling
[platform-tools]: https://developer.android.com/studio/releases/platform-tools.html
[platform-tools-windows]: https://dl.google.com/android/repository/platform-tools-latest-windows.zip


## Get the app

Download the [latest release][latest] and extract it.

[latest]: https://github.com/HoppouDev/ret/releases/latest

The Linux archive (`ret-linux64-<version>.zip`) contains:
 - `ret.apk`
 - `ret`
 - `ret-ui`

The Windows archive (`ret-win64-<version>.zip`) contains:
 - `ret.apk`
 - `ret.exe`
 - `ret-ui.exe`
 - `ret-run.cmd`

Each release also lists the SHA-256 of both archives in `SHA256SUMS.txt`.

### Coming from Gnirehtet

The ret app is a different Android app from the Gnirehtet one, so `ret` does
not upgrade or control it. Uninstall the old app once:

    adb uninstall com.genymobile.gnirehtet

The first `ret run` installs the new app and asks for the VPN permission again.


## Run (simple)

_Note: On Windows, replace `./ret` by `ret` in the following commands._

The application has no UI, and is intended to be controlled from the computer
only.

If you want to activate reverse tethering for exactly one device, just execute:

    ./ret run

Reverse tethering remains active until you press _Ctrl+C_.

On Windows, for convenience, you can double-click on `ret-run.cmd` instead (it
just executes `ret run`, without requiring to open a
terminal).

The very first start should open a popup to request permission:

![request](assets/request.jpg)

A "key" logo appears in the status bar whenever _ret_ is active:

![key](assets/key.png)

Alternatively, you can enable reverse tethering for all connected devices
(present and future) by calling:

    ./ret autorun


## Run

You can execute the actions separately (it may be useful if you want to reverse
tether several devices simultaneously).

Start the relay server and keep it open:

    ./ret relay

Install the `apk` on your Android device:

    ./ret install [serial]

In another terminal, for each client, execute:

    ./ret start [serial]

To stop a client:

    ./ret stop [serial]

To reset the tunnel (useful to get the connection back when a device is
unplugged and plugged back while ret is active):

    ./ret tunnel [serial]

The _serial_ parameter is required only if `adb devices` outputs more than one
device.

For advanced options, run `./ret --help`, or `./ret help <command>` for the
details of one command (for example `./ret help start`).


## Graphical interface

`ret-ui` is a window that does the same as the commands above: start and stop
the relay server, set the port, DNS servers and routes, and start, stop,
reinstall or reset the tunnel of each connected device. It can also start every
device automatically as soon as it is connected, and shows the logs.

It is in the release archives next to `ret`. Start it with `./ret-ui` (on
Windows, double-click on `ret-ui.exe`), with `ret.apk` in the same directory,
like for `ret`.

It reads the same [environment variables](#environment-variables) as `ret`.


## Run manually

The `ret` program exposes a simple command-line interface that executes
lower-level commands. You can call them manually instead.

To start the relay server:

    ./ret relay

To install the apk:

    adb install -r ret.apk

To start a client:

    adb reverse localabstract:ret tcp:31416
    adb shell am start -a dev.hoppou.ret.START \
        -n dev.hoppou.ret/.RetActivity

To stop a client:

    adb shell am start -a dev.hoppou.ret.STOP \
        -n dev.hoppou.ret/.RetActivity


## Environment variables

`ADB` defines a custom path to the `adb` executable:

```bash
ADB=/path/to/my/adb ./ret run
```

`RET_APK` defines a custom path to `ret.apk`:

```bash
RET_APK=/usr/share/ret/ret.apk ./ret run
```

`RUST_LOG` sets which logs are printed (`info` by default), for example to
print debug logs:

```bash
RUST_LOG=debug ./ret run
```

It also accepts per-component levels, such as
`RUST_LOG=info,TcpConnection=debug`.


## Why _ret_?

It is short for _reverse tethering_. The upstream name, _gnirehtet_, is
_tethering_ reversed:

    rev <<< tethering

(in _Bash_)


## Developers

Read the [developers page].

[developers page]: DEVELOP.md


## Licence

    Copyright (C) 2017 Genymobile

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

        http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.


## Articles about Gnirehtet

- [Introducing “gnirehtet”, a reverse tethering tool for Android][medium-1] ([French version][blog-1])
- [Gnirehtet 2: our reverse tethering tool for Android now available in Rust][medium-2]
- [Gnirehtet rewritten in Rust][blog-2-en] ([French version][blog-2-fr])

[medium-1]: https://medium.com/@rom1v/gnirehtet-reverse-tethering-android-2afacdbdaec7
[blog-1]: https://blog.rom1v.com/2017/03/gnirehtet/
[medium-2]: https://medium.com/genymobile/gnirehtet-2-our-reverse-tethering-tool-for-android-now-available-in-rust-999960483d5a
[blog-2-en]: https://blog.rom1v.com/2017/09/gnirehtet-rewritten-in-rust/
[blog-2-fr]: https://blog.rom1v.com/2017/09/gnirehtet-reecrit-en-rust/
