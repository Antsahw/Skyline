#!/usr/bin/env python3
"""Riding a mod's mob, from three horizon-ctl states and the floor's height: prints one line
per finding ("seated", "drove N", "off") for regress.py's `mount` case."""
import json
import sys


def at(path):
    s = json.load(open(path))
    p = s.get("player", s)
    pos = p.get("position") or p.get("pos")
    return [float(v) for v in pos]


floor = float(open(sys.argv[1]).read())
on, drove, off = (at(p) for p in sys.argv[2:5])
# The seat on the 1-block-tall mount is 0.4 over the floor (Player's vehicle attachment 0.6); standing, on it.
if on[1] - floor > 0.3:
    print("seated", round(on[1] - floor, 2))
moved = ((drove[0] - on[0]) ** 2 + (drove[2] - on[2]) ** 2) ** 0.5
if drove[1] - floor > 0.3 and moved > 1.5:
    print("drove", round(moved, 2))
if off[1] - floor < 0.3:
    print("off", round(off[1] - floor, 2))
# Where each was, for a failure's report.
print("at", [round(v, 2) for v in on], [round(v, 2) for v in drove], [round(v, 2) for v in off], "floor", floor)
