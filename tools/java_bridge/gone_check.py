#!/usr/bin/env python3
"""Two horizon-ctl `entities` lists, before and after: prints "there" when an entity of TYPE
was in the first, "gone" when none is in the second (killed), and "kept KEEP" when one of
type KEEP is still in the second. Usage: gone_check.py TYPE BEFORE AFTER [KEEP]"""
import json
import sys


def of(path, kind):
    return [e for e in json.load(open(path)).get("entities", []) if e.get("type") == kind]


kind, before, after = sys.argv[1:4]
if of(before, kind):
    print("there")
if not of(after, kind):
    print("gone")
if len(sys.argv) > 4 and of(after, sys.argv[4]):
    print("kept", sys.argv[4])
