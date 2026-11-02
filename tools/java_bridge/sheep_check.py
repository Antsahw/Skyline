#!/usr/bin/env python3
"""regress.py's `neighbours` case: Horizon's sheep in two horizon-ctl `entities` lists, before
and after the hunter's bites. Prints "there <health>" when it stood whole at first, "hurt
<health or gone>" when it then had less health or was gone (killed)."""
import json
import sys


def sheep(path):
    found = [e for e in json.load(open(path)).get("entities", []) if "sheep" in str(e.get("type"))]
    return found[0].get("health") if found else None


before, after = sheep(sys.argv[1]), sheep(sys.argv[2])
if before is not None and before >= 8:
    print("there", before)
if before is not None and (after is None or after < before):
    print("hurt", "gone" if after is None else after)
