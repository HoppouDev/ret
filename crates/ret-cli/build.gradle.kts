tasks.register<Exec>("debug") {
    commandLine("cargo", "build", "--workspace", "--locked")
}

tasks.register<Exec>("release") {
    commandLine("cargo", "build", "--workspace", "--release", "--locked")
}

tasks.register<Exec>("test") {
    commandLine("cargo", "test", "--workspace", "--locked")
}

tasks.register<Exec>("install") {
    commandLine("cargo", "install", "--path", ".")
}

tasks.register<Exec>("clean") {
    commandLine("cargo", "clean")
}

tasks.register<Exec>("checkstyle") {
    commandLine("cargo", "fmt", "--all", "--", "--check")
}

tasks.register("check") {
    dependsOn("checkstyle", "test")
}

tasks.register("build") {
    dependsOn("check", "debug", "release")
}

// Requirements:
//
// Install the cross-compile toolchain (on Debian):
//   sudo apt install gcc-mingw-w64-x86-64
//   rustup target add x86_64-pc-windows-gnu
//
// Add the following lines to ~/.cargo/config:
//   [target.x86_64-pc-windows-gnu]
//   linker = "x86_64-w64-mingw32-gcc"
//   ar = "x86_64-w64-mingw32-gcc-ar"
tasks.register<Exec>("releaseCrossToWindows") {
    commandLine("cargo", "build", "--workspace", "--release", "--locked", "--target=x86_64-pc-windows-gnu")
}
