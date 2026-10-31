#!/usr/bin/env python3
"""Verify the complete release asset set and write its SHA-256 manifest."""

import argparse
import hashlib
import re
import tarfile
import zipfile
from pathlib import Path


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def check_portable(parser, path, root, programs, apk_name, architecture):
    members = {root, f"{root}/LICENSE", *[f"{root}/{name}" for name in programs], f"{root}/{apk_name}"}
    for name in (*programs, apk_name):
        members.update(str(parent) for parent in (Path(root) / name).parents if str(parent) != ".")
    with tarfile.open(path) as archive:
        if set(archive.getnames()) != members:
            parser.error(f"portable archive has unexpected contents: {path.name}")
        for name in programs:
            member = archive.getmember(f"{root}/{name}")
            if not member.isfile() or member.mode & 0o111 != 0o111:
                parser.error(f"portable {name} is not executable: {path.name}")
            with archive.extractfile(member) as stream:
                header = stream.read(20)
            if architecture in ("x86_64", "aarch64"):
                machine = {"x86_64": b"\x3e\x00", "aarch64": b"\xb7\x00"}[architecture]
                valid = header[:7] == b"\x7fELF\x02\x01\x01" and header[18:20] == machine
            else:
                cpu = {"macos-x86_64": b"\x07\x00\x00\x01", "macos-arm64": b"\x0c\x00\x00\x01"}[architecture]
                valid = header[:8] == b"\xcf\xfa\xed\xfe" + cpu
            if not valid:
                parser.error(f"portable {name} has the wrong architecture: {path.name}")
        apk_member = archive.getmember(f"{root}/{apk_name}")
        if not apk_member.isfile():
            parser.error(f"portable APK is not a regular file: {path.name}")
        with archive.extractfile(apk_member) as stream:
            return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version")
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    version, directory = args.version, args.directory
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        parser.error("version must be major.minor.patch")
    expected = {
        "LICENSE": None,
        f"ret-{version}.tar.gz": b"\x1f\x8b",
        f"ret-v{version}.apk": b"PK\x03\x04",
        f"ret-{version}-1-archlinux-x86_64.pkg.tar.zst": b"\x28\xb5\x2f\xfd",
        f"ret-{version}.PKGBUILD": None,
        f"ret-{version}.SRCINFO": None,
        f"ret-{version}.APKBUILD": None,
    }
    for arch, deb_arch in (("x86_64", "amd64"), ("aarch64", "arm64")):
        expected.update({
            f"ret-{version}-linux-{arch}.tar.gz": b"\x1f\x8b",
            f"ret_{version}-1_{deb_arch}.deb": b"!<arch>\n",
            f"ret-{version}-1-fedora-opensuse.{arch}.rpm": b"\xed\xab\xee\xdb",
            f"ret-{version}-1.el9-rhel9.{arch}.rpm": b"\xed\xab\xee\xdb",
            f"ret-{version}-linux-{arch}.AppImage": b"\x7fELF",
            f"ret-{version}-r0-alpine-{arch}.apk": b"\x1f\x8b",
            f"ret-{version}-alpine-{arch}.rsa.pub": b"-----BEGIN PUBLIC KEY-----",
        })
    for arch in ("x86_64", "arm64"):
        expected.update({
            f"ret-setup-{version}-windows-{arch}.exe": b"MZ",
            f"ret-{version}-macos-{arch}.dmg": None,
            f"ret-{version}-macos-{arch}.tar.gz": b"\x1f\x8b",
        })
    manifest_path = directory / "SHA256SUMS.txt"
    if manifest_path.is_symlink() or (manifest_path.exists() and not manifest_path.is_file()):
        parser.error("SHA256SUMS.txt must be a regular file")
    found = {path.name for path in directory.iterdir()} - {"SHA256SUMS.txt"}
    if found != expected.keys():
        parser.error(f"asset mismatch, missing {sorted(expected.keys() - found)}, unexpected {sorted(found - expected.keys())}")
    for name, magic in expected.items():
        path = directory / name
        if not path.is_file() or path.is_symlink() or path.stat().st_size == 0:
            parser.error(f"asset is not a non-empty regular file: {name}")
        if magic is not None:
            with path.open("rb") as stream:
                if not stream.read(len(magic)) == magic:
                    parser.error(f"invalid asset format: {name}")

    apk = directory / f"ret-v{version}.apk"
    source = directory / f"ret-{version}.tar.gz"
    arch_recipe = (directory / f"ret-{version}.PKGBUILD").read_text()
    arch_hashes = re.search(r"^sha256sums=\(([^)]*)\)", arch_recipe, re.MULTILINE)
    if arch_hashes is None or re.findall(r"[0-9a-f]{64}", arch_hashes[1]) != [sha256(source), sha256(apk)]:
        parser.error("Arch recipe checksums do not match the release source and APK")
    alpine_recipe = (directory / f"ret-{version}.APKBUILD").read_text()
    alpine_hashes = re.search(r'^sha512sums="([^"]*)"', alpine_recipe, re.MULTILINE)
    expected_alpine_hashes = []
    for path, name in ((source, source.name), (apk, "ret.apk")):
        with path.open("rb") as stream:
            expected_alpine_hashes.append([hashlib.file_digest(stream, "sha512").hexdigest(), name])
    if alpine_hashes is None or [line.split() for line in alpine_hashes[1].splitlines() if line.strip()] != expected_alpine_hashes:
        parser.error("Alpine recipe checksums do not match the release source and APK")
    with zipfile.ZipFile(apk) as archive:
        if "AndroidManifest.xml" not in archive.namelist() or archive.testzip() is not None:
            parser.error("Android APK is corrupt or missing its manifest")
    for arch in ("x86_64", "aarch64"):
        path = directory / f"ret-{version}-linux-{arch}.tar.gz"
        digest = check_portable(parser, path, f"ret-linux-{arch}", ("ret", "ret-ui"), "ret.apk", arch)
        if digest != sha256(apk):
            parser.error(f"portable Linux archive contains a different APK: {path.name}")
    for arch in ("x86_64", "arm64"):
        path = directory / f"ret-{version}-macos-{arch}.tar.gz"
        digest = check_portable(parser, path, f"ret-macos-{arch}", ("bin/ret", "bin/ret-ui"), "share/ret/ret.apk", f"macos-{arch}")
        if digest != sha256(apk):
            parser.error(f"portable macOS archive contains a different APK: {path.name}")
        dmg = directory / f"ret-{version}-macos-{arch}.dmg"
        with dmg.open("rb") as stream:
            if dmg.stat().st_size < 512:
                parser.error(f"invalid disk image: {dmg.name}")
            stream.seek(-512, 2)
            if stream.read(4) != b"koly":
                parser.error(f"invalid disk image: {dmg.name}")

    manifest = "".join(f"{sha256(directory / name)}  {name}\n" for name in sorted(expected))
    manifest_path.write_text(manifest)
    print(f"Verified {len(expected)} release assets for {version}")


if __name__ == "__main__":
    main()
