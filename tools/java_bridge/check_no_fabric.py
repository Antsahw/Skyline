#!/usr/bin/env python3
"""Nothing of Fabric's in the repository or a release's files (docs/java-bridge.md, "Ownership").

Fabric Loader, Mixin, Sat4j, ASM and Fabric API are downloaded on the player's machine from
Fabric's servers and run unchanged: Horizon never ships, vendors or copies them. This looks for
their classes and jars:

  python3 tools/java_bridge/check_no_fabric.py --tree                  # the repository (git ls-files)
  python3 tools/java_bridge/check_no_fabric.py dist/*.tar.gz dist/*.zip  # a release's archives

In a file (a program, an archive's member, a jar inside either) it finds class files of their
packages by their names, which zip archives keep uncompressed, and the jars they ship as
(`fabric-loader-*.jar`, ...). In the tree it also finds Java sources declaring their packages.
Exit 1 with one line per finding.
"""

import argparse
import io
import re
import subprocess
import sys
import tarfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
# Fabric Loader and what it brings (fabric-libs.tsv), Fabric API, Mixin's other forks.
PACKAGES = ("net/fabricmc/", "org/spongepowered/", "org/sat4j/", "org/objectweb/asm/",
            "com/llamalad7/mixinextras/")
CLASS = re.compile(rb"(?:" + rb"|".join(re.escape(p.encode()) for p in PACKAGES) + rb")[\w/$\-]+\.class")
JAR = re.compile(r"(^|/)(fabric-loader|fabric-api|sponge-mixin|mixinextras|sat4j|asm(-[a-z]+)?)-\d[^/]*\.jar$",
                 re.I)
SOURCE = re.compile(rb"^\s*package\s+(net\.fabricmc|org\.spongepowered|org\.sat4j|org\.objectweb\.asm)\b", re.M)


def scan_bytes(label, data, found):
    """Class files of Fabric's packages named in `data` (a program, a jar, any file)."""
    hit = CLASS.search(data)
    if hit:
        found.append(f"{label}: {hit.group().decode(errors='replace')}")


def scan_member(label, name, data, found):
    if JAR.search(name):
        found.append(f"{label}: ships {name}")
    scan_bytes(label, data, found)
    if name.lower().endswith((".jar", ".zip")):
        try:
            with zipfile.ZipFile(io.BytesIO(data)) as z:
                for inner in z.namelist():
                    if inner.endswith("/"):
                        continue
                    scan_member(f"{label}!{inner}", inner, z.read(inner), found)
        except zipfile.BadZipFile:
            pass


def scan_file(path, found):
    name = path.name.lower()
    if name.endswith((".tar.gz", ".tgz", ".tar")):
        with tarfile.open(path) as t:
            for m in t.getmembers():
                if m.isfile():
                    scan_member(f"{path.name}:{m.name}", m.name, t.extractfile(m).read(), found)
    elif name.endswith((".zip", ".jar")):
        with zipfile.ZipFile(path) as z:
            for inner in z.namelist():
                if not inner.endswith("/"):
                    scan_member(f"{path.name}:{inner}", inner, z.read(inner), found)
    else:
        scan_member(path.name, path.name, path.read_bytes(), found)


def scan_tree(found):
    files = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, capture_output=True, check=True).stdout
    for rel in filter(None, files.decode().split("\0")):
        path = ROOT / rel
        if not path.is_file():
            continue
        if rel.lower().endswith((".jar", ".class")):
            found.append(f"{rel}: a {Path(rel).suffix} in the repository")
            continue
        if rel.endswith(".java") and SOURCE.search(path.read_bytes()):
            found.append(f"{rel}: declares a package of Fabric's, Mixin's, Sat4j's or ASM's")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("files", nargs="*", type=Path, help="programs or archives to look through")
    ap.add_argument("--tree", action="store_true", help="look through the repository's tracked files")
    a = ap.parse_args()
    if not a.files and not a.tree:
        ap.error("give files, --tree, or both")
    found = []
    if a.tree:
        scan_tree(found)
    for f in a.files:
        scan_file(f, found)
    for line in found:
        print(f"FAIL {line}")
    what = ", ".join([*(["the repository"] if a.tree else []), *(f.name for f in a.files)])
    print(f"{'no' if not found else len(found)} Fabric, Mixin, Sat4j or ASM files in {what}")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
