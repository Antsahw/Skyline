#!/usr/bin/env python3
"""Counts the chunks of a square saved in a world's region files, and how many say they are full.

    tools/java_bridge/region_check.py <dimension folder> <cx0> <cz0> <cx1> <cz1>
    -> "saved 81 full 81 of 81"

Reads the region files as vanilla's RegionFile does (header offsets, length, compression; zlib,
gzip, none and lz4 are not all handled: zlib and none, Horizon's). A dev tool for the regress cases.
"""
import os
import struct
import sys
import zlib


def main():
    folder, x0, z0, x1, z1 = sys.argv[1], *map(int, sys.argv[2:6])
    saved = full = 0
    total = (x1 - x0 + 1) * (z1 - z0 + 1)
    files = {}
    for cx in range(x0, x1 + 1):
        for cz in range(z0, z1 + 1):
            path = os.path.join(folder, "region", f"r.{cx >> 5}.{cz >> 5}.mca")
            if path not in files:
                files[path] = open(path, "rb").read() if os.path.exists(path) else b""
            data = files[path]
            if len(data) < 8192:
                continue
            i = 4 * ((cx & 31) + (cz & 31) * 32)
            location = struct.unpack(">I", data[i:i + 4])[0]
            if location == 0:
                continue
            saved += 1
            offset = (location >> 8) * 4096
            length = struct.unpack(">I", data[offset:offset + 4])[0]
            compression = data[offset + 4]
            body = data[offset + 5:offset + 4 + length]
            if compression == 2:
                body = zlib.decompress(body)
            elif compression != 3:
                continue
            if b"minecraft:full" in body:
                full += 1
    print(f"saved {saved} full {full} of {total}")


if __name__ == "__main__":
    main()
