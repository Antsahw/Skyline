#!/usr/bin/env python3
"""Compares Skyline's Java hooks (`java_*` / `java120_*` calls) in `crates/horizon-client/src`
between a git ref (default `multiversion`) and the working tree.

After a merge of Horizon's main a hook that upstream's restructuring moved shows up as a count
that did not change; one that was dropped shows up as a count that went down. A hunk that landed
in the wrong function still compiles, so the build alone does not say. Read the differences:
a lower number is a hook to find again, a higher one is usually upstream's own `java` function
(number parsing, string hashes). A function upstream has too (`upstream/main`, fetched by
tools/sync_upstream.sh) is not Skyline's hook: its count moves with upstream's refactors, and only
a note is printed. Exit status 1 when one of Skyline's own hooks has fewer calls.
"""
import collections
import os
import re
import subprocess
import sys

ROOT = "crates/horizon-client/src"
CALL = re.compile(r"\b(java\w*_\w+|java120_\w+|\w+_java\w*)\(")


def count(text, into):
    for m in CALL.finditer(text):
        into[m.group(1)] += 1


def of_ref(ref):
    found = collections.Counter()
    files = subprocess.check_output(["git", "ls-tree", "-r", "--name-only", ref, ROOT], text=True)
    for name in files.split():
        if name.endswith(".rs"):
            text = subprocess.check_output(["git", "show", f"{ref}:{name}"], text=True, errors="ignore")
            count(text, found)
    return found


def of_tree():
    found = collections.Counter()
    for folder, _, names in os.walk(ROOT):
        for name in names:
            if name.endswith(".rs"):
                with open(os.path.join(folder, name), errors="ignore") as f:
                    count(f.read(), found)
    return found


def upstream_has(name, upstream):
    found = subprocess.run(["git", "grep", "-q", name + "(", upstream, "--", ROOT])
    return found.returncode == 0


def main():
    ref = sys.argv[1] if len(sys.argv) > 1 else "multiversion"
    upstream = sys.argv[2] if len(sys.argv) > 2 else "upstream/main"
    before, now = of_ref(ref), of_tree()
    lost = False
    for name in sorted(set(before) | set(now)):
        if before[name] != now[name]:
            fewer = now[name] < before[name]
            upstreams = fewer and upstream_has(name, upstream)
            print(f"{name}: {before[name]} -> {now[name]}" + ("  (upstream's own)" if upstreams else ""))
            lost = lost or (fewer and not upstreams)
    print("a hook went missing" if lost else "no hook fewer than in " + ref)
    return 1 if lost else 0


if __name__ == "__main__":
    sys.exit(main())
