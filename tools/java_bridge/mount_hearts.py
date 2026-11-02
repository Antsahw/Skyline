#!/usr/bin/env python3
"""Counts a mount's heart colour (vanilla's hud/heart/vehicle_full: orange, 218 102 44) right
of the screen's middle just above the hotbar, where a mount's hearts replace the food bar,
in each screenshot given: "orange <file> <count>"."""
import sys
from PIL import Image

for path in sys.argv[1:]:
    im = Image.open(path).convert("RGB")
    w, h = im.size
    px = im.load()
    n = 0
    for y in range(max(0, h - 140), h - 40):
        for x in range(w // 2 + 8, min(w, w // 2 + 280)):
            r, g, b = px[x, y]
            if abs(r - 218) < 14 and abs(g - 102) < 14 and abs(b - 44) < 14:
                n += 1
    print("orange", path.rsplit("/", 1)[-1], n)
