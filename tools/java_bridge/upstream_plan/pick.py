#!/usr/bin/env python3
"""pick.py FILE HUNKS: the fork's diff of FILE (merge base -> java-bridge, -U3), only hunks HUNKS
(1-based, comma list), as a patch for `git apply`. Env MB, FB: the two commits."""
import os, subprocess, sys
path, want = sys.argv[1], {int(x) for x in sys.argv[2].split(",")}
text = subprocess.run(["git", "diff", "-U3", os.environ["MB"], os.environ["FB"], "--", path],
                      capture_output=True, text=True, check=True).stdout.splitlines(True)
head, hunks, cur = [], [], None
for l in text:
    if l.startswith("@@"):
        cur = [l]; hunks.append(cur)
    elif cur is None:
        head.append(l)
    else:
        cur.append(l)
sys.stdout.write("".join(head) + "".join("".join(h) for i, h in enumerate(hunks, 1) if i in want))
