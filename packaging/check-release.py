#!/usr/bin/env python3
"""Check release versions and optionally export materialized release sources."""

import argparse
import gzip
import re
import subprocess
import tarfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag", nargs="?", help="release tag, such as v0.3.0")
    parser.add_argument("--source-archive", type=Path)
    args = parser.parse_args()

    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())
    version = workspace["workspace"]["package"]["version"]
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        parser.error(f"release version must be major.minor.patch: {version}")
    major, minor, patch = map(int, version.split("."))
    if minor >= 100 or patch >= 100 or not 0 < major * 10000 + minor * 100 + patch <= 2100000000:
        parser.error(f"version cannot be represented by the Android versionCode: {version}")
    if args.tag is not None and args.tag != f"v{version}":
        parser.error(f"tag {args.tag!r} does not match v{version}")

    patterns = {
        "app/build.gradle.kts": r'^\s*versionName = "([^"\n]+)"$',
        "packaging/linux/arch/PKGBUILD": r"^pkgver=(\S+)$",
        "packaging/linux/alpine/APKBUILD": r"^pkgver=(\S+)$",
    }
    for filename, pattern in patterns.items():
        matches = re.findall(pattern, (ROOT / filename).read_text(), re.MULTILINE)
        if matches != [version]:
            parser.error(f"{filename} version differs from {version}: {matches}")

    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    for package in lock["package"]:
        if package["name"].startswith("ret-") and "source" not in package:
            if package["version"] != version:
                parser.error(f"Cargo.lock has stale version for {package['name']}")

    for filename, magic in {"assets/ret.png": b"\x89PNG\r\n\x1a\n", "assets/ret.ico": b"\0\0\1\0"}.items():
        if not (ROOT / filename).read_bytes().startswith(magic):
            parser.error(f"{filename} is not materialized, run git lfs pull")

    if args.source_archive:
        epoch = int(subprocess.check_output(["git", "log", "-1", "--format=%ct"], cwd=ROOT))
        files = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode().split("\0")
        with args.source_archive.open("xb") as output:
            with gzip.GzipFile(filename="", mode="wb", fileobj=output, mtime=epoch) as compressed:
                with tarfile.open(fileobj=compressed, mode="w|") as archive:
                    for filename in sorted(filter(None, files)):
                        path = ROOT / filename
                        if path.is_file() and path.read_bytes().startswith(b"version https://git-lfs.github.com/spec/v1"):
                            parser.error(f"{filename} is an unresolved Git LFS pointer")
                        info = archive.gettarinfo(str(path), f"ret-{version}/{filename}")
                        info.uid = info.gid = 0
                        info.uname = info.gname = "root"
                        info.mtime = epoch
                        if info.isfile():
                            with path.open("rb") as source:
                                archive.addfile(info, source)
                        else:
                            archive.addfile(info)
    print(version)


if __name__ == "__main__":
    main()
