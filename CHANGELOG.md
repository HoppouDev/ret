# Changelog

All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

- - -

## [v0.4.0](https://github.com/HoppouDev/ret/compare/a81d9ce7cebf2a86615b1e4821423ec24181bd04..v0.4.0) - 2026-10-31

### Features

- (**adb**) always use the downloaded platform tools - ([581c273](https://github.com/HoppouDev/ret/commit/581c2738976639c17bfc792fbfac16d7dfb609ce)) - HoppouDev
- (**desktop**) show ADB tool download progress - ([0946917](https://github.com/HoppouDev/ret/commit/0946917036ca19cf016269b84d83d60ffe40aa10)) - HoppouDev
- (**packaging**) add native ARM64 and macOS release distributions - ([8ee2e40](https://github.com/HoppouDev/ret/commit/8ee2e407d02bb65ae752583617af574beb03d95a)) - HoppouDev
- (**ui**) title the window Ret and set its icon - ([2702943](https://github.com/HoppouDev/ret/commit/2702943f0475bed7f4c26410a2922001149be27b)) - HoppouDev

#### Bug Fixes

- (**adb**) start the adb server without flashing a terminal - ([4fab835](https://github.com/HoppouDev/ret/commit/4fab8356e170f54caecd16f859ce3df3873258ef)) - HoppouDev
- (**ui**) stop owned ADB server on window close - ([83d1c0a](https://github.com/HoppouDev/ret/commit/83d1c0ae284817f4f8f6291b047bb13aa29417e6)) - HoppouDev

#### Build system

- (**release**) skip the debug package of makepkg - ([a81d9ce](https://github.com/HoppouDev/ret/commit/a81d9ce7cebf2a86615b1e4821423ec24181bd04)) - HoppouDev

- - -

## [v0.3.0](https://github.com/HoppouDev/ret/compare/f153cd7c666d6c2d1260523892d36eb25f9e906e..v0.3.0) - 2026-10-29

### Features

- (**adb**) also look for the apk next to the program - ([867a00f](https://github.com/HoppouDev/ret/commit/867a00f0b9e3acc32aee469fa06f1e9283879d71)) - HoppouDev
- (**adb**) fall back to the apk installed by the system package - ([2562e38](https://github.com/HoppouDev/ret/commit/2562e38929400bb199fbf8b8e7cc6d5888dc529f)) - HoppouDev

#### Bug Fixes

- (**adb**) start the adb server when missing and unblock the install stream - ([681421a](https://github.com/HoppouDev/ret/commit/681421a03e88dcc21a2a625d10712fdd96d3c677)) - HoppouDev

#### Build system

- (**gradle**) add gradle spotless plugin fr this time - ([f153cd7](https://github.com/HoppouDev/ret/commit/f153cd7c666d6c2d1260523892d36eb25f9e906e)) - HoppouDev
- (**packaging**) add Debian, Fedora, RHEL, Alpine and AppImage packages - ([f018690](https://github.com/HoppouDev/ret/commit/f0186905affba23f8615a0f2129e6111653e7a9f)) - HoppouDev
- (**packaging**) add a Windows installer and an Arch Linux package - ([9467771](https://github.com/HoppouDev/ret/commit/9467771de5a0153d56e4c7859ce37f32c7495640)) - HoppouDev
- move the packaging scripts into packaging - ([b747142](https://github.com/HoppouDev/ret/commit/b74714221240087c7d4b33251a51d9c0395a83d5)) - HoppouDev

#### Refactoring

- (**adb**) run the device commands with adb_client instead of the adb executable - ([599085a](https://github.com/HoppouDev/ret/commit/599085a5297286ce101424a07d93a29aec5440a7)) - HoppouDev

#### Miscellaneous Chores

- (**assets**) add the logo and its affinity source - ([8d2f3bc](https://github.com/HoppouDev/ret/commit/8d2f3bc929068a1e17d1e3e74743d6992b47dc1c)) - HoppouDev

- - -

## [v0.2.0](https://github.com/HoppouDev/ret/compare/561c0030796191c1514b6049a4ffb41bebecec77..v0.2.0) - 2026-10-13

### Features

- (**adb**) get the name and kind of devices and whether their client runs - ([de51803](https://github.com/HoppouDev/ret/commit/de51803e82f3cc6687953d27f3ad36ec1069ffb1)) - HoppouDev
- (**cli**) run the relay of autorun only while a client needs it - ([df3f89d](https://github.com/HoppouDev/ret/commit/df3f89d0de159170712314088f03837a7a0c967d)) - HoppouDev
- (**relay**) show connections with an arrow in the logs - ([6ad182c](https://github.com/HoppouDev/ret/commit/6ad182c024f28991a9aace0c1534f477eef2d61b)) - HoppouDev
- (**ui**) add a settings page, device menus, saved settings and logs, and run the relay on demand - ([e83fe55](https://github.com/HoppouDev/ret/commit/e83fe558c63287b1b425a61ec9d7677bd20169f4)) - HoppouDev

#### Bug Fixes

- (**app**) drop packets shorter than an ipv4 header instead of looping forever - ([2265420](https://github.com/HoppouDev/ret/commit/22654207fb143366d1f3d1c5a911b49a10f029bf)) - HoppouDev
- (**app**) recover after a packet that is not ipv4 from the relay - ([5883b2b](https://github.com/HoppouDev/ret/commit/5883b2b4daf7e9d212b7ff260895a2605592d056)) - HoppouDev
- (**app**) do not crash when stopping a client that is not running - ([0b4e4a3](https://github.com/HoppouDev/ret/commit/0b4e4a37d27dac79ccde082a5ef240e17e128f3f)) - HoppouDev

#### Tests

- (**app**) cover the relay tunnel reconnection and the packet helpers - ([ddcf140](https://github.com/HoppouDev/ret/commit/ddcf14052b5c5449e1e895283edd989e70e0b131)) - HoppouDev

#### Build system

- (**gradle**) add the spotless plugin - ([9d3a5d6](https://github.com/HoppouDev/ret/commit/9d3a5d67e5b1f6f40fd35f0bcf613234e6a2aadd)) - HoppouDev
- optimize the release binaries for size - ([d3ee2fb](https://github.com/HoppouDev/ret/commit/d3ee2fb38749e20ded319486b673d57d1f63b412)) - HoppouDev
- enforce lf line endings and store media files with git lfs - ([0ba6ec5](https://github.com/HoppouDev/ret/commit/0ba6ec5df10c609efafcee1cd973d9e5d7465d2a)) - HoppouDev

#### Continuous Integration

- run the android unit tests and lint on every push - ([72ad3c0](https://github.com/HoppouDev/ret/commit/72ad3c0e137ba39ed6b46c60a97006e350298086)) - HoppouDev

#### Refactoring

- (**app**) port the android client to kotlin - ([561c003](https://github.com/HoppouDev/ret/commit/561c0030796191c1514b6049a4ffb41bebecec77)) - HoppouDev
- (**relay**) report the errors of the relay with their context - ([887eef4](https://github.com/HoppouDev/ret/commit/887eef41414f741f06c2fd5786cbaebb41b266d5)) - HoppouDev

#### Miscellaneous Chores

- ignore the lock files of affinity - ([3d8feca](https://github.com/HoppouDev/ret/commit/3d8fecabfb4132e7ded1c9b9273959ba991ca2c3)) - HoppouDev

- - -

## [v0.1.0](https://github.com/HoppouDev/ret/compare/326fdeb177cf40f8152292e40ff5e3beb2e33da5..v0.1.0) - 2026-10-12

### Features

- (**adb**) report every device known to adb with its state - ([e443ae6](https://github.com/HoppouDev/ret/commit/e443ae6f282d54cb85d7b50fe15f1da4b8e4c324)) - HoppouDev
- (**adb**) add ret-adb crate to control the client and devices through adb - ([44ff691](https://github.com/HoppouDev/ret/commit/44ff69102534bccdb355fa96dfdbc5760d4d35a4)) - HoppouDev
- (**cli**) color the help and error messages - ([3b38bec](https://github.com/HoppouDev/ret/commit/3b38bec4908be22704d9acad1fe6813463b2f1d2)) - HoppouDev
- (**relay**) stop the relay server on demand - ([4f44955](https://github.com/HoppouDev/ret/commit/4f4495561ff080e5b34fa51e191b60cb3e15a8df)) - HoppouDev
- (**relay**) configure logs with RUST_LOG using tracing - ([65dc50d](https://github.com/HoppouDev/ret/commit/65dc50d7ccedea2c16cb74ea2a1f62ec66d3d9c4)) - HoppouDev
- (**ui**) open no console window with release builds on windows - ([95cb8fe](https://github.com/HoppouDev/ret/commit/95cb8fe4da069892556c90cc1caa4aa3ec856122)) - HoppouDev
- (**ui**) add a graphical interface built with iced - ([440d351](https://github.com/HoppouDev/ret/commit/440d35177bd763134a520994e3da0fb3b0e0b0ad)) - HoppouDev
- derive the app versionCode from the release version, bumped by cog - ([46912b4](https://github.com/HoppouDev/ret/commit/46912b462b5b818b6e1471ebcada0ed54b05dab1)) - HoppouDev

#### Bug Fixes

- (**relay**) reset the client connection when the destination cannot be reached - ([28704c7](https://github.com/HoppouDev/ret/commit/28704c7171736f294107de7ee5992e47d674c45d)) - HoppouDev
- (**relay**) add color back to the logger - ([59327ea](https://github.com/HoppouDev/ret/commit/59327ea09234305e23ccd1d45a017c1cd8c05ee3)) - HoppouDev
- (**relay**) build empty TCP packets apart from pending data packets - ([cec3689](https://github.com/HoppouDev/ret/commit/cec368925318c28b19aceaa8a7ba702c96fab318)) - HoppouDev
- (**relay**) acknowledge unacceptable TCP segments such as keepalive probes - ([dce3dc4](https://github.com/HoppouDev/ret/commit/dce3dc4a3c365d9e8310ea8363a7893625319991)) - HoppouDev
- (**relay**) ignore spurious writable events on TCP connections - ([71400b2](https://github.com/HoppouDev/ret/commit/71400b228060b4aeb93d4d4d7d73eae12790e1b4)) - HoppouDev
- (**relay**) keep client connection open when its socket write would block - ([32d9322](https://github.com/HoppouDev/ret/commit/32d9322a17a94e09e134ff9b2aedf6cb78c60822)) - HoppouDev

#### Documentation

- remove the changelog - ([8b4a5c4](https://github.com/HoppouDev/ret/commit/8b4a5c4fec41d1a37e4fd1f8dee9eeeaf8e34bcb)) - HoppouDev
- describe the tokio-based rust relay - ([56841a6](https://github.com/HoppouDev/ret/commit/56841a60599bbc1d549d7bdb656e23925e32a238)) - HoppouDev

#### Tests

- (**relay**) cover tcp, udp, router and client relaying - ([8977700](https://github.com/HoppouDev/ret/commit/89777007192e95d9d6cc97d51aad6298fb39f1e0)) - HoppouDev

#### Build system

- (**cog**) hide the version bump commits from the changelog - ([a0eb182](https://github.com/HoppouDev/ret/commit/a0eb182f094e8320ba299dc89c0e96ce2a8b4124)) - HoppouDev
- (**gradle**) build every crate from the cargo tasks - ([b2bc06f](https://github.com/HoppouDev/ret/commit/b2bc06faa7f48c0a6a735cf556f4a70135220907)) - HoppouDev
- (**gradle**) migrate the build scripts to the kotlin dsl - ([ea6dec6](https://github.com/HoppouDev/ret/commit/ea6dec62f1595deeb2c30c7b93411c7b4c5099b1)) - HoppouDev
- (**relay**) upgrade dependencies - ([7bc02e9](https://github.com/HoppouDev/ret/commit/7bc02e962fbccdf058b6c191791a1090fdf77fe3)) - HoppouDev
- (**relay**) migrate to rust 2024 edition - ([82596a5](https://github.com/HoppouDev/ret/commit/82596a57b40f2d29a7a606369a6d768cf1fdb685)) - HoppouDev
- share one version across the crates, set by cog bump - ([783b611](https://github.com/HoppouDev/ret/commit/783b6115fcc1727e37e90937f1da0637bf6c6959)) - HoppouDev
- add prek hooks and cocogitto config - ([9466d4f](https://github.com/HoppouDev/ret/commit/9466d4fe0a0bd8fcc779b9cd6517a2aa861ae966)) - HoppouDev

#### Continuous Integration

- (**release**) ship ret-ui in the release archives - ([f2521b0](https://github.com/HoppouDev/ret/commit/f2521b09519c2821535ba4ba8fd6bf3a74c6dfe6)) - HoppouDev
- add rust test and release workflows - ([69e7353](https://github.com/HoppouDev/ret/commit/69e73531654ba4a61eaa72bf898951a6d0831b32)) - HoppouDev

#### Refactoring

- ![BREAKING](https://img.shields.io/badge/BREAKING-red) (**cli**) rewrite the cli with clap in ret-cli and make ret-relay a library - ([ff5d0e7](https://github.com/HoppouDev/ret/commit/ff5d0e7c8620b3f99d0ea726b0d3f26e96cd3b7c)) - HoppouDev
- (**relay**) organize the relay into modules - ([f0a7b31](https://github.com/HoppouDev/ret/commit/f0a7b310d6d5699c5e6b8253b3e13ec3053791c9)) - HoppouDev
- (**relay**) replace mio event loop with tokio tasks - ([0caf775](https://github.com/HoppouDev/ret/commit/0caf7754def65ee365f7b681389be85e83d7c5bc)) - HoppouDev
- (**relay**) port event loop to mio 1 - ([6fd9f2d](https://github.com/HoppouDev/ret/commit/6fd9f2d7261ba38f9cde6663df1cb5de948353e7)) - HoppouDev
- ![BREAKING](https://img.shields.io/badge/BREAKING-red) rename the project to ret - ([10c4bae](https://github.com/HoppouDev/ret/commit/10c4bae1d648aee0112a54028e4ce3679c01b058)) - HoppouDev
- ![BREAKING](https://img.shields.io/badge/BREAKING-red) remove the java relay and move the rust relay to relay/ - ([6f0d1bf](https://github.com/HoppouDev/ret/commit/6f0d1bfe5678a4ae88d23b43dbdf9073fd6c9675)) - HoppouDev

#### Miscellaneous Chores

- import gnirehtet v2.5.1 - ([326fdeb](https://github.com/HoppouDev/ret/commit/326fdeb177cf40f8152292e40ff5e3beb2e33da5)) - HoppouDev

#### Style

- (**relay**) fix clippy warnings - ([5e971e1](https://github.com/HoppouDev/ret/commit/5e971e10121092932a58d700303a7af739b8e246)) - HoppouDev
- (**relay**) apply rust 2024 formatting - ([1c43639](https://github.com/HoppouDev/ret/commit/1c43639ca7438b616666b8524e6590661734fc3d)) - HoppouDev

- - -

Changelog generated by [cocogitto](https://github.com/cocogitto/cocogitto).
