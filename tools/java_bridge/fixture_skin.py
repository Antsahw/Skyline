#!/usr/bin/env python3
"""A test skin (64x64 PNG, slim arms) for the offline-skin cases (HORIZON_OFFLINE_SKIN): plain
dark base layers and bright, patterned outer layers, so 3D skin layer mods' voxels show which
skin built them. Drawn here, never taken from Mojang's art.

    python3 tools/java_bridge/fixture_skin.py OUT.png
"""
import struct
import sys
import zlib

W = H = 64

# Base layers (dark) and outer layers (bright): x, y, width, height, RGBA.
BASE = [
    ((0, 0, 32, 16), (60, 40, 30, 255)),      # head
    ((0, 16, 16, 16), (30, 30, 70, 255)),     # right leg
    ((16, 16, 24, 16), (40, 40, 40, 255)),    # body
    ((40, 16, 14, 16), (70, 50, 40, 255)),    # right arm (slim: 3 wide faces)
    ((16, 48, 16, 16), (30, 30, 70, 255)),    # left leg
    ((32, 48, 14, 16), (70, 50, 40, 255)),    # left arm
]
OUTER = [
    ((32, 0, 32, 16), (255, 230, 0, 255)),    # hat: yellow
    ((0, 32, 16, 16), (255, 120, 0, 255)),    # right trousers: orange
    ((16, 32, 24, 16), (0, 220, 60, 255)),    # jacket: green
    ((40, 32, 14, 16), (255, 0, 200, 255)),   # right sleeve: magenta
    ((0, 48, 16, 16), (255, 120, 0, 255)),    # left trousers: orange
    ((48, 48, 14, 16), (0, 220, 255, 255)),   # left sleeve: cyan
]


def pixels():
    rgba = bytearray(W * H * 4)

    def fill(rect, colour, pattern):
        x0, y0, w, h = rect
        for y in range(y0, y0 + h):
            for x in range(x0, x0 + w):
                # Outer layers: holes in a checker of 2x2 cells, so the layer stands out in 3D.
                if pattern and ((x // 2) + (y // 2)) % 3 == 0:
                    continue
                i = (y * W + x) * 4
                rgba[i:i + 4] = bytes(colour)

    for rect, colour in BASE:
        fill(rect, colour, False)
    for rect, colour in OUTER:
        fill(rect, colour, True)
    return bytes(rgba)


def png(rgba):
    rows = b"".join(b"\0" + rgba[y * W * 4:(y + 1) * W * 4] for y in range(H))

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    with open(sys.argv[1], "wb") as f:
        f.write(png(pixels()))


if __name__ == "__main__":
    main()
