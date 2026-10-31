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

ret talks to your devices through the [adb server] of your machine, with the
[adb_client] library. The `adb` program is only used to start that server
when no server runs yet.

No `adb` has to be installed: ret downloads the [Android SDK platform
tools][platform-tools] into its data directory (`<AppData>\ret` on Windows,
`~/Library/Application Support/ret` on macOS, `~/.local/share/ret` on Linux)
and always starts the server from that copy. An `adb` installed on the system
is never used, and the download is kept, so it happens once. Only `ADB`
replaces the downloaded program, with one you choose.

Google's Linux download requires x86_64 and glibc. On Linux ARM64 or Alpine,
install your distribution's native `adb` and set `ADB` to its full path.
For example, use `ADB=/usr/bin/adb ret run` or `ADB=/usr/bin/adb ret-ui`.
The Alpine package already installs native `adb`.

While the tools download, `ret` shows a terminal progress bar with
the amount received and the percentage when the download size is known,
or a spinner when it is unknown, followed by the extraction status. `ret-ui`
shows the same progress in a banner, even when the logs are hidden, and shows
an error there if the download or installation fails.

`ret-ui` stops the ADB server it started when its window closes. If an ADB
server was already running, the window reuses it and leaves it running on
exit. CLI commands leave the server running so commands such as `ret start`
can keep the device connected after returning.

Make sure you [enabled adb debugging][enable-adb] on your device(s).

[adb server]: https://developer.android.com/studio/command-line/adb.html
[adb_client]: https://github.com/cocool97/adb_client
[enable-adb]: https://developer.android.com/studio/command-line/adb.html#Enabling
[platform-tools]: https://developer.android.com/tools/releases/platform-tools


## Get the app

Download the appropriate files from the [latest release][latest].

The Linux tarball (`ret-<version>-linux-<arch>.tar.gz`) contains:
 - `ret.apk`
 - `ret`
 - `ret-ui`
 - `LICENSE`

On Windows, run `ret-setup-<version>-windows-<arch>.exe`, choosing `x86_64`
for Intel/AMD or `arm64` for Windows on ARM. The installer puts `ret` and
`ret-ui` on the user `Path` and installs the same `ret.apk`.

On macOS 13 or newer, download `ret-<version>-macos-<arch>.dmg`, choosing
`x86_64` for Intel or `arm64` for Apple Silicon. Open the disk image and drag
`ret.app` into Applications. The app includes the Android APK. These builds
are ad-hoc signed, not Developer ID signed or notarized. If macOS blocks the
first launch, review the app in System Settings > Privacy & Security and
choose Open Anyway only if you trust the downloaded release.

The matching macOS `.tar.gz` contains `bin/ret`, `bin/ret-ui`,
`share/ret/ret.apk` and `LICENSE`. Keep that directory layout when using the
command-line programs. Windows and macOS download their platform tools
automatically on first use.

Every release also publishes packages for Debian, Fedora, openSUSE, RHEL 9,
Arch and Alpine, an AppImage, and the Android application
(`ret-v<version>.apk`) on its own. Linux tarballs, debs, RPMs, AppImages and
Alpine packages support x86_64 and ARM64. Arch Linux packages are x86_64 only.
The release also includes the Apache-2.0 `LICENSE`.

Install the native package with your distribution's package manager so its
library dependencies are installed too:

```bash
# Debian and Ubuntu
sudo apt install ./ret_<version>-1_amd64.deb
# Fedora
sudo dnf install ./ret-<version>-1-fedora-opensuse.x86_64.rpm
# RHEL 9 and its clones
sudo dnf install ./ret-<version>-1.el9-rhel9.x86_64.rpm
# openSUSE
sudo zypper install ./ret-<version>-1-fedora-opensuse.x86_64.rpm
# Arch Linux
sudo pacman -U ./ret-<version>-1-archlinux-x86_64.pkg.tar.zst
```

Replace `<version>` with the release version. The examples above use x86_64.
For ARM64, choose `arm64` in Debian package names and `aarch64` in RPM,
Alpine, Linux tarball and AppImage names.

Alpine packages use a signing key for each release and architecture.
Download the matching `ret-<version>-alpine-<arch>.rsa.pub` from the same
release, install it under `/etc/apk/keys/`, then install
`ret-<version>-r0-alpine-<arch>.apk` with `apk add`. This Alpine package is
distinct from the Android APK named `ret-v<version>.apk`.

The Linux tarball and AppImage require glibc 2.31 or newer. The graphical
interface also needs the host's X11 or Wayland libraries, libxkbcommon and
EGL/OpenGL support. They do not include adb or graphics drivers. Make the
AppImage executable with `chmod +x ret-<version>-linux-<arch>.AppImage` before
running it. It uses its bundled Android APK unless `RET_APK` is already set.

Download `SHA256SUMS.txt` alongside your chosen assets to check their hashes:

```bash
sha256sum --ignore-missing -c SHA256SUMS.txt
```

This checks the files you downloaded, not the authenticity of their publisher.

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

The relay server then runs only while a client runs: it starts with the first
client and stops when the last one stops, for example when its device is
unplugged or the client is stopped from the device.


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

`ret-ui` is a window that does the same as the commands above. Its main page
lists the connected devices and shows the logs. Each device has a button to
start or stop its client, and a menu (the `…` button) to reset its tunnel or
reinstall the client. Headsets are shown with a headset icon, phones with a
phone icon.

The relay server runs by itself: it starts as soon as a client is started or
found running, and stops once no client runs anymore. The header shows its
state.

The gear button opens the settings, grouped by category:
 - Connection: the port of the relay server, the DNS servers and the routes
 - Devices: auto-start, which starts the client on every device as soon as it
   is connected (on by default)
 - Appearance: the theme, light, dark or the same as the system

It is in the release archives next to `ret`. Start it with `./ret-ui` (on
Windows, double-click on `ret-ui.exe`), with `ret.apk` in the same directory,
like for `ret`.

It reads the same [environment variables](#environment-variables) as `ret`.

The window remembers its settings in `config.toml`, in a `ret` directory of the
user configuration directory:
 - Linux: `~/.config/ret/config.toml` (or `$XDG_CONFIG_HOME/ret/config.toml`)
 - Windows: `%APPDATA%\ret\config.toml`

The window creates it with the default settings when it starts, and saves it as
soon as a setting changes. You can also edit it by hand, every key is optional:

```toml
version = 1
port = 31416
dns_servers = ["1.1.1.1", "9.9.9.9"]
routes = ["10.0.0.0/8"]
autostart = true
theme = "system"  # or "light", "dark"
```

`version` is the layout of the file, which ret writes and upgrades itself. If
the file cannot be read, or was written by a newer version of ret, the window
starts with the default settings, logs the error and leaves the file untouched.

The window also saves its logs, one file per day, keeping the last 7, in a
`ret/logs` directory of the user data directory:
 - Linux: `~/.local/share/ret/logs` (or `$XDG_DATA_HOME/ret/logs`)
 - Windows: `%APPDATA%\ret\logs`
 - macOS: `~/Library/Application Support/ret/logs`


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

`ADB` defines a custom path to the `adb` executable, which ret uses to start
the adb server:

```bash
ADB=/path/to/my/adb ./ret run
```

`RET_APK` defines a custom path to `ret.apk`. Without it, ret looks for
`ret.apk` in the current directory, then for the copy installed by the Arch
package at `/usr/share/ret/ret.apk`:

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
