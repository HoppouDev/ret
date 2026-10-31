# ret for developers


## Getting started

### Requirements

You need the [Android SDK] (_Android Studio_) and JDK 17 or newer.

You also need the [Rust] environment to build the relay server:

```bash
wget https://sh.rustup.rs -O rustup-init
sh rustup-init
```

[Android SDK]: https://developer.android.com/studio/index.html
[Rust]: https://www.rust-lang.org/


### Build

#### Everything

If `gradle` is installed on your computer:

    gradle build

Otherwise, you can call the [gradle wrapper]:

    ./gradlew build

This will build the Android application and the relay server, both in debug
and release versions.

[gradle wrapper]: https://docs.gradle.org/current/userguide/gradle_wrapper.html


#### Specific parts

Several _gradle_ tasks are exposed in the root project. For instance:

 - `debugAll` and `releaseAll` build the Android application and the relay
   server
 - `releaseWindows` builds them, with the relay server cross-compiled to
   Windows (see below)
 - `checkAll` runs their checks and tests

Even if the relay server build tasks are exposed through `gradle` (which wraps
calls to `cargo`), it is often more convenient to use `cargo` directly.

For instance, to build a release version of the relay server:

    cargo build --release

It will generate the binary in `target/release/ret`.


#### Cross-compile the relay server from Linux to Windows

To build `ret.exe` from Linux, install the cross-compile toolchain (on
Debian):

    sudo apt install gcc-mingw-w64-x86-64
    rustup target add x86_64-pc-windows-gnu

Add the following lines to `~/.cargo/config`:

    [target.x86_64-pc-windows-gnu]
    linker = "x86_64-w64-mingw32-gcc"
    ar = "x86_64-w64-mingw32-gcc-ar"

Then build:

    cargo build --release --target=x86_64-pc-windows-gnu

It will generate `target/x86_64-pc-windows-gnu/release/ret.exe`.


### Android Studio

To import the project in _Android Studio_: File → Import…

From there, you can develop on the Android application. You can also execute
any _gradle_ tasks, and run the tests with visual results.


### Git hooks

Commits are checked by Git hooks managed by [prek]:
 - the relay server must be formatted (`cargo fmt`) and free of [clippy]
   warnings
 - commit messages must follow [Conventional Commits], checked by
   [cocogitto]

Install `prek` and `cocogitto` (for example `scoop install prek cocogitto` on
Windows, or `cargo install prek cocogitto`), then enable the hooks once in your
clone:

    prek install

To run the format and lint checks on the whole repository:

    prek run --all-files

The current version and the next one can be printed with `cog get-version` and
`cog bump --auto --dry-run`.

On every push, GitHub Actions runs the relay server tests (`cargo test`), and
the unit tests and lint of the Android application (`./gradlew :app:check`).

### Releases

Use `cog bump --auto` (or a specific `--major`, `--minor` or `--patch`) to
prepare the next version from Conventional Commits. Review the bump commit
before pushing it and its tag, for example `git push origin master v0.4.0`.
The bump hook updates the workspace, Android, Arch and Alpine versions and the
Cargo lockfile. It needs a POSIX shell with `sed`.

Release tags must be exactly `v<major>.<minor>.<patch>` and match all four
version declarations and the lockfile. Prerelease tags are not supported by
the native package recipes. Minor and patch must each be below 100 because
Android uses `10000 * major + 100 * minor + patch` as its `versionCode`.
`ret` reinstalls the Android application whenever its installed `versionCode`
differs from its own.

Run `python3 packaging/check-release.py` with Python 3.11 or newer to check the
versions and release icons before tagging. Git LFS must be installed and its
files downloaded. The Packaging workflow runs these checks on pushes and pull
requests.

Pushing a `v*` tag runs the Release workflow. Running it manually with
`workflow_dispatch` builds and validates the same assets without publishing.
Both modes need the Android signing secrets listed below. Build-only runs
leave a `release-assets` Actions artifact for inspection.

The Android release job locates `sdkmanager` and `apksigner` under
`ANDROID_HOME`, without requiring either command on `PATH`.

The workflow:
 - exports a versioned source archive with materialized Git LFS files
 - builds and verifies the signed Android application
 - builds and tests native x86_64 and ARM64 Windows programs, compiles each
   installer, then installs, checks and uninstalls it on the matching runner
 - builds and tests the glibc programs on native x86_64 and ARM64 runners in
   a pinned Rust 1.98.0 Bullseye image, then packages tarballs, debs, RPMs
   and AppImages for each architecture
 - builds and tests the Arch x86_64 and Alpine x86_64/ARM64 packages as
   unprivileged users in separate containers
 - installs the deb, Arch and Alpine packages and checks their CLI versions
 - builds and tests macOS Intel and Apple Silicon programs, creates ad-hoc
   signed app bundles and disk images, and exercises the packaged CLIs
 - checks the complete asset list, native recipe checksums and portable
   archives' APKs and executable architectures, writes `SHA256SUMS.txt`,
   uploads everything to a draft release, then publishes it

Only the final job has repository write permission. Release builds do not
restore build caches from other workflows. The Linux baseline is glibc 2.31,
with Bullseye dependencies obtained from Debian's archive. The AppImage tool
is version-pinned and SHA-256 verified before execution. The AppImage reuses
that tool's embedded runtime rather than downloading an unpinned runtime.

The workflow never deletes or replaces an existing release. A failed upload
can leave an unpublished draft. Inspect and remove that draft manually before
retrying. Do not delete a published release to retry a build. Publish a new
version instead.

#### Linux packages

The deb uses automatic linked-library dependencies plus explicit dependencies
for the graphical libraries loaded at runtime. The RPMs also record detected
ABI versions, including glibc, and explicitly require the graphical library
sonames. They are generated in the same container as their binaries. Fedora
and openSUSE share one RPM, with a separate `.el9` release for RHEL 9 and its
clones.

Release filenames include `fedora-opensuse`, `rhel9`, `archlinux` or `alpine`
to distinguish packages that share an extension with other distributions or
with ordinary archives. Their native extensions and internal package names
remain unchanged. The `.deb` extension already identifies the Debian family.

Linux archive and AppImage names include `linux` and either `x86_64` or
`aarch64`. RPM and Alpine packages use those architecture names too, while
debs use `amd64` and `arm64`. Arch Linux publishes only x86_64 packages
because its official repositories do not support ARM64.

The AppImage contains both programs and its Android APK. Its launcher sets
`RET_APK` to the bundled APK unless the user already set that variable. It
does not bundle adb, the display server or graphics drivers, and still needs
the host's X11 or Wayland, libxkbcommon and EGL/OpenGL libraries.

The Arch and Alpine builds refresh checksums against the exact local source
and signed APK before building. The release publishes the resulting
`ret-<version>.PKGBUILD`, `ret-<version>.SRCINFO` and
`ret-<version>.APKBUILD`. Use those release recipes for standalone builds,
renaming them to `PKGBUILD` or `APKBUILD`. They download the published
`ret-<version>.tar.gz` and `ret-v<version>.apk`, not GitHub's automatically
generated source archives, which may contain Git LFS pointers.

The checked-in recipes are build inputs. Their versions are bumped by
cocogitto, but their historical checksums are replaced only in the copied
release recipes. Do not use `SKIP` checksums. Each Alpine release and
architecture has its own signing key. The public key filename is
`ret-<version>-alpine-<arch>.rsa.pub` and must match the name embedded in the
package signature. Install that exact filename under `/etc/apk/keys/`.
Alpine installs `android-tools` as a dependency. Run it with
`ADB=/usr/bin/adb` because Google's Linux archive needs x86_64 and glibc.
Linux ARM64 installations also need a native ADB selected through `ADB`.
`vulkan-loader` is optional.

To package an existing Linux build locally, put the APK at
`target/release/ret.apk`, then run `cargo deb -p ret-cli --no-build` and
`cargo generate-rpm` from `crates/ret-cli`. Run these tools on the distribution
used to build the programs so automatic ABI detection sees matching libraries.
The release workflow uses cargo-deb 3.8.0 and cargo-generate-rpm 0.21.1.

#### Windows installer

`packaging/windows/ret.iss` requires
[Inno Setup](https://jrsoftware.org/isinfo.php) 6.3 or later and an explicit
`/DAppVersion=<version>` and `/DAppArch=x86_64` or `/DAppArch=arm64`.
`FilesDir` defaults to `dist/ret-windows-<AppArch>` and `OutputDir` to `dist`,
both relative to the repository root. The workflow uses the preinstalled
compiler on native Windows runners for each architecture. Each installer
accepts only its native OS architecture, without relying on emulation.

It installs `ret.exe`, `ret-ui.exe`, `ret-run.cmd`, `ret.apk` and `LICENSE`
per user into `%LOCALAPPDATA%\Programs\ret`. It can add that directory to the
user `Path` and sets `RET_APK` only if it is absent. Uninstall removes only
values recorded as added by the installer and retains custom values.

#### macOS distributions

Native Intel and Apple Silicon runners build with
`MACOSX_DEPLOYMENT_TARGET=13.0`. After building both programs, run
`bash packaging/build-macos <version> <x86_64|arm64> <apk-path>` on the
matching Mac. `OUTPUT_DIR` defaults to `dist`. `CARGO_TARGET_DIR` and
`CARGO_BUILD_TARGET` can select an existing native Cargo build.

The helper creates `ret-<version>-macos-<arch>.dmg` and `.tar.gz`. The disk
image contains `ret.app` and an Applications shortcut. The tarball contains
`ret-macos-<arch>/bin/{ret,ret-ui}`, `share/ret/ret.apk` and `LICENSE`. Keep
the prefix layout intact so the programs can find the APK from any working
directory. The programs download their platform tools on first use.

The app bundles and portable executables are ad-hoc signed, not Developer ID
signed or notarized. They can require approval in macOS Privacy & Security
after download. The helper checks architecture, minimum OS version, system
library dependencies and code signatures, mounts the resulting DMG, then
executes the CLIs from both distributions outside the source directory.

#### Local portable release

`packaging/release` builds portable archives on x86_64 Linux from a clean
checkout at an exact release tag. It needs Python 3.11+, Java, Rust with the
`x86_64-unknown-linux-gnu` and `x86_64-pc-windows-gnu` targets, MinGW GCC,
`zip`, `tar`, and Android signing properties configured as described below.
It uses the Gradle wrapper unless `GRADLE` names another executable.

Set `APKSIGNER` to the SDK's executable if it is not on `PATH` and cannot be
found through `ANDROID_HOME` or `ANDROID_SDK_ROOT`. The script verifies the APK
signature and writes:
 - `dist/ret-<version>-linux-x86_64.tar.gz`
 - `dist/ret-<version>-windows-x86_64.zip`
 - `dist/ret-v<version>.apk`
 - `dist/SHA256SUMS-<version>.txt`

Existing outputs cause a failure rather than replacement. Other `dist` files
are retained. This local command does not build native Linux packages or the
Windows installer. To build the installer separately, extract the Windows ZIP
under `dist` and run
`iscc /DAppVersion=<version> /DAppArch=x86_64 packaging/windows/ret.iss`.
Verify the local archives with `sha256sum -c SHA256SUMS-<version>.txt` from
the `dist` directory.

#### Android signing

The release workflow signs the APK with the keystore stored in these repository
secrets:
 - `RELEASE_KEYSTORE_BASE64`: the keystore file, base64-encoded
 - `RELEASE_STORE_PASSWORD`: the keystore password
 - `RELEASE_KEY_ALIAS`: the alias of the signing key
 - `RELEASE_KEY_PASSWORD`: the password of the signing key

Keep the same key for every release. Android refuses to upgrade an app signed
with a different key, so users would have to uninstall it first.

To sign a local release build, set `RELEASE_STORE_FILE`,
`RELEASE_STORE_PASSWORD`, `RELEASE_KEY_ALIAS` and `RELEASE_KEY_PASSWORD` in
`~/.gradle/gradle.properties`.

[prek]: https://prek.j178.dev
[clippy]: https://doc.rust-lang.org/clippy/
[Conventional Commits]: https://www.conventionalcommits.org
[cocogitto]: https://docs.cocogitto.io


## Overview

The client registers itself as a [VPN], in order to intercept the whole device
network traffic.

It exchanges raw [IPv4 packets] as `byte[]` with the device:
 - it receives packets from the Android applications or system;
 - it must forge response packets.

The client (executed on the Android device) just maintains a TCP connection to
the relay server, and sends the raw packets to it.

This TCP connection is established over _adb_, after we started a reverse
port redirection:

    adb reverse localabstract:ret tcp:31416

This means that every connection initiated to `localhost:31416` from the device
will be redirected to the port `31416` on the computer, on which the relay
server is listening.

The relay server does all the hard work. It receives the IP packets from every
connected client and opens [standard sockets][berkeley] (which, of course, don't
require _root_) accordingly, then relays data in both directions. This requires
to translate packets between level 3 (on the device side) and level 5 (on the
network side) in the [OSI model].

In a sense, the relay server behaves like a [NAT] (more precisely a
[port-restricted cone NAT][portNAT]), in that it opens connections on behalf of
private peers. However, it differs from a standard NAT in the way it
communicates with the clients (the private peers), by using a very specific
(though simple) protocol, over a TCP connection.

[VPN]: https://developer.android.com/reference/android/net/VpnService.html
[IPv4 packets]: https://en.wikipedia.org/wiki/IPv4#Packet_structure
[OSI model]: https://en.wikipedia.org/wiki/OSI_model
[berkeley]: https://en.wikipedia.org/wiki/Berkeley_sockets
[NAT]: https://en.wikipedia.org/wiki/Network_address_translation
[portNAT]: https://en.wikipedia.org/wiki/Network_address_translation#Methods_of_translation


## Client

The client is an _Android_ project written in _Kotlin_, located in
[`app/`](app/).

The [`VpnService`] is implemented by [`RetService`].

We control the application through intents to [`RetActivity`].

Some configuration options may be passed as extra parameters, converted to a
[`VpnConfiguration`] instance. Currently, the user can configure the DNS servers
to use.

The very first time, Android requests to the user the permission to enable the
VPN. In that case, the API requires to call
[`startActivityForResult`], so we need an [`Activity`]: [`RetActivity`] also
handles this request.

[`RelayTunnel`] manages one connection to the relay server.
[`PersistentRelayTunnel`] manages [`RelayTunnel`] instances to handle
reconnections, so that we can stop and start the relay while the client keeps
running.

To send response packets to the system, we must write one packet at a time to
the VPN interface. Since we receive packets from the relay server over a TCP
connection, we have to split writes at packet boundaries: this is the purpose
of [`IPPacketOutputStream`].

[`VpnService`]: https://developer.android.com/reference/android/net/VpnService.html
[`RetService`]: app/src/main/kotlin/dev/hoppou/ret/RetService.kt
[`RetActivity`]: app/src/main/kotlin/dev/hoppou/ret/RetActivity.kt
[`VpnConfiguration`]: app/src/main/kotlin/dev/hoppou/ret/VpnConfiguration.kt
[`startActivityForResult`]: https://developer.android.com/reference/android/app/Activity.html#startActivityForResult%28android.content.Intent,%20int%29
[`Activity`]: https://developer.android.com/reference/android/app/Activity.html
[`RelayTunnel`]: app/src/main/kotlin/dev/hoppou/ret/RelayTunnel.kt
[`PersistentRelayTunnel`]: app/src/main/kotlin/dev/hoppou/ret/PersistentRelayTunnel.kt
[`IPPacketOutputStream`]: app/src/main/kotlin/dev/hoppou/ret/IPPacketOutputStream.kt


## Relay server

The relay server is a _Rust_ library located in
[`crates/ret-relay/`](crates/ret-relay/). The control of the clients and of
the devices through `adb` lives in the
[`crates/ret-adb/`](crates/ret-adb/) library. The `ret` program, built from
[`crates/ret-cli/`](crates/ret-cli/), is the command-line interface on top of
both, and the `ret-ui` window, built from [`crates/ret-ui/`](crates/ret-ui/)
with [iced], is the graphical one.

[iced]: https://iced.rs/

The relay server uses [asynchronous I/O] (through [tokio]). As a consequence,
it is essentially monothreaded, so there is no need for synchronization to
handle packets.

[asynchronous I/O]: https://en.wikipedia.org/wiki/Asynchronous_I/O
[tokio]: https://docs.rs/tokio/1/tokio/


### Event loop

[`Relay`][rust/relay] creates a single-threaded _tokio_ runtime, on which each
socket is handled by its own asynchronous task:
 - the [_tunnel server_][rust/tunnel-server] accepts the clients on port 31416
   and spawns a task for every accepted [_client_][rust/client]
 - each _TCP connection_ and each _UDP connection_ to the network runs in its
   own task

Tasks exchange packets through bounded channels ([`mpsc`][tokio/mpsc]).

[rust/relay]: crates/ret-relay/src/server/mod.rs
[rust/tunnel-server]: crates/ret-relay/src/server/tunnel.rs
[rust/client]: crates/ret-relay/src/server/client.rs
[tokio/mpsc]: https://docs.rs/tokio/1/tokio/sync/mpsc/index.html

![archi](assets/archi.png)

### Client

Each _client_ manages a TCP socket, used to transmit raw IP packets from and to
the _ret_ Android client. Thus, these IP packets are encapsulated into TCP
(they are transmitted as the TCP payload).

When a client connects, the relay server assigns an integer id to it, which it
writes to the TCP socket. The client considers itself connected to the relay
server only once it has received this number. This allows to detect any
end-to-end connection issue immediately. For instance, a TCP _connect_ initiated
by a client succeeds whenever a port redirection is enabled (typically through
`adb reverse`), even if the relay server is not listening. In that case, the
first _read_ will fail.


### Packets

A type representing an _IPv4 packet_ ([`Ipv4Packet`][rust/ipv4-packet])
provides a structured view to read and write packet data, which is physically
stored in the buffers (the little squares on the schema). A packet is copied
into a new buffer when it is passed from one task to another (between the client
and a connection). The packets for the device are copied once more when the
client batches them into a single write. Otherwise, the packets just point to
the buffer where they are stored.

[rust/ipv4-packet]: crates/ret-relay/src/packet/ipv4\_packet.rs

Each packet contains an instance of _IPv4 headers_ and _transport headers_
(which might be _TCP_ or _UDP_ headers).

The borrowing rules of Rust prevent the headers from sharing a mutable slice of
the raw packet buffer. Therefore, _header data_ classes (`*HeaderData`) are used
to store the fields, and lifetime-bound views (`*Header<'a>` and
`*HeaderMut<'a>`) reference both the raw array and the _header data_:

 - [`packet::ipv4_header`][rust/ipv4-header]:
   - data: `Ipv4HeaderData`
   - view: `Ipv4Header<'a>`
   - mutable view: `Ipv4HeaderMut<'a>`
 - [`packet::transport::tcp`][rust/tcp-header]:
   - data: `TcpHeaderData`
   - view: `TcpHeader<'a>`
   - mutable view: `TcpHeaderMut<'a>`
 - [`packet::transport::udp`][rust/udp-header]:
   - data: `UdpHeaderData`
   - view: `UdpHeader<'a>`
   - mutable view: `UdpHeaderMut<'a>`

In addition, we use [enums][rust-enums] for _transport headers_ to statically
dispatch calls to _UDP_ and _TCP_ header classes:

 - [`packet::transport`][rust/transport-header]:
   - data: `TransportHeaderData`
   - view: `TransportHeader<'a>`
   - mutable view: `TransportHeaderMut<'a>`

[rust/ipv4-header]: crates/ret-relay/src/packet/ipv4\_header.rs
[rust/tcp-header]: crates/ret-relay/src/packet/transport/tcp.rs
[rust/udp-header]: crates/ret-relay/src/packet/transport/udp.rs
[rust/transport-header]: crates/ret-relay/src/packet/transport/mod.rs
[rust-enums]: https://doc.rust-lang.org/book/first-edition/enums.html


### Router

Each _client_ holds a _router_ ([`Router`][rust/router]), responsible for
sending the packets to the right _connection_, identified by these 5 properties
available in the IP and transport headers:

 - protocol
 - source address
 - source port
 - destination address
 - destination port

These identifiers are stored in a _connection id_
([`ConnectionId`][rust/connection]), used as a key to find or create the
associated _connection_.

The router keeps the sending side of the channel of each _connection_. When
this channel is full, the packet is dropped (see below why this is safe).

[rust/router]: crates/ret-relay/src/server/router.rs
[rust/connection]: crates/ret-relay/src/connection/id.rs


### Connections

A _connection_ is either a _TCP connection_
([`TcpConnection`][rust/tcp-connection]) or a _UDP connection_
([`UdpConnection`][rust/udp-connection]) to the requested destination. It runs
in its own task, which receives the packets of the device from the router and
sends its packets to the client channel, in order, through an
[_outbox_][rust/outbox]: the packets which do not fit in the client channel
wait there until it has room.

[rust/tcp-connection]: crates/ret-relay/src/connection/tcp.rs
[rust/udp-connection]: crates/ret-relay/src/connection/udp.rs
[rust/outbox]: crates/ret-relay/src/connection/outbox.rs

The connection is responsible for converting data from level 3 to level 5 for
device-to-network packets, and from level 5 to level 3 for network-to-device
packets. For _UDP connections_, it consists essentially in removing or
adding IP and transport headers. For _TCP connections_, however, it
requires to respond to the client according to the TCP protocol ([RFC 793]),
in such a way as to ensure a correct end-to-end communication.

[RFC 793]: https://tools.ietf.org/html/rfc793

A _packetizer_ ([`Packetizer`][rust/packetizer]) converts from level 5 to level
3 by appending correct IP and transport headers.

[rust/packetizer]: crates/ret-relay/src/packet/packetizer.rs


#### UDP connection

When the first packet for a specific UDP connection is received from the device,
a new `UdpConnection` is created. It keeps a copy of the IP and UDP headers
of this first packet, swapping the source and the destination, in order to use
them as headers for all response packets.

The relaying is simple for UDP: each packet received from one side must be sent
to the other side, without any splitting or merging (datagram boundaries must be
preserved for UDP).

Since UDP is not a connected protocol, a UDP connection is never "closed".
Therefore, each UDP connection task terminates itself after 2 minutes without
any packet.


#### TCP connection

`TcpConnection` also keeps, as a reference, a copy of the IP and TCP headers
of the first packet received.

However, contrary to UDP, TCP must provide reliable delivery. In particular,
lost packets have to be retransmitted. Nonetheless, we can take advantage of the
two TCP we are proxifying, so that we can provide reliability by delegating the
retransmission mechanism to them. In fact, it is sufficient to guarantee that
**we cannot lose packets from network to device**.

Indeed, any packet written to a TCP channel is safe, since it will be managed by
the TCP implementation from the system. Losing a raw IP packet received from the
device is also safe: the device TCP implementation will follow the TCP protocol
to retransmit it. Therefore, **dropping packets from device to network does not
break the connection**.

On the other hand, once we retrieved a packet from a TCP channel from the
network, we are responsible for it. Would it be dropped, there would be no way
to recover the connection.

As far as I know, there are only two possible causes of packet loss for which we
are responsible:

 1. When **our buffers are full**, we won't resize them indefinitely, so we have to
drop packets. Typically, this may happen if the data from the network is
received at a higher rate than that they can be sent to the device.

 2. When **a raw packet is considered invalid** by the device, it is rejected.
This may happen for example if the checksum is invalid or if the TCP sequence
number is [out-of-the-window][flow control].

[flow control]: https://en.wikipedia.org/wiki/Transmission_Control_Protocol#Flow_control

Therefore, by [contraposition], if we guarantee that we never retrieve a packet
that we won't be able to store, and that we provide a valid checksum and respect
the client TCP window, then **we won't lose any packet without implementing any
retransmission mechanism**.

[contraposition]: https://en.wikipedia.org/wiki/Contraposition

To prevent retrieving a packet while our buffers are full, a `TcpConnection`
reads from the network only when its _outbox_ is empty. When the client channel
is full, the packet just read waits in the _outbox_, and the connection stops
reading until the channel has room for it.


## Hack

For more details, go read the code!

If you find a bug, or have an awesome idea to implement, please discuss and
contribute ;-)
