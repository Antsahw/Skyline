#!/usr/bin/env python3
"""Whether two screenshots differ in their middle (an animation moved between them):
prints "changed N" (N pixels that differ) or "same". A regress case's step (gecko).

    python3 tools/java_bridge/changed.py a.png b.png
"""
import sys

from PIL import Image


def main():
    a, b = (Image.open(p).convert("RGB") for p in sys.argv[1:3])
    w, h = a.size
    # The middle half: the scene's mobs, not the sky's clouds or the HUD.
    box = (w // 4, h // 4, w * 3 // 4, h * 3 // 4)
    pa, pb = a.crop(box).tobytes(), b.crop(box).tobytes()
    n = sum(1 for i in range(0, len(pa), 3) if abs(pa[i] - pb[i]) + abs(pa[i + 1] - pb[i + 1]) + abs(pa[i + 2] - pb[i + 2]) > 30)
    print(f"changed {n}" if n > 200 else f"same {n}")


if __name__ == "__main__":
    main()
