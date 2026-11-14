#!/usr/bin/env python3
"""Builds Skyline with the native mods in `native-mods/`, as the launcher's native build does.

The launcher can't build the 1.20.1 bridge, so this does its steps by hand:

1. installs horizon-mixins at the commit each mod names (`horizon-native.json`'s `mixins`) into
   `.local/native/tools/<commit>/`, once;
2. copies this checkout's source (tracked and new files, not ignored ones) to
   `.local/native/source/`, pinning the mods to this checkout's commit: they were made for a
   newer Horizon, and `horizon-mixins` refuses them otherwise;
3. applies the mods (`horizon-mixins apply`) and copies only what changed into
   `.local/native/stage/`, so Cargo's next build recompiles only what the mods touch;
4. builds the game there into this checkout's `target/` (the dependencies are shared).

    python3 tools/skyline_native.py                  # fast profile, both Java bridges
    python3 tools/skyline_native.py --release
    python3 tools/skyline_native.py --features java-mods-120

The game is then `target/<profile>/horizon-client`, as from a plain build. Native mods are native
code with your rights: build only mods whose source you trust.
"""

import argparse
import filecmp
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODS = ROOT / "native-mods"
WORK = ROOT / ".local" / "native"
MIXINS_GIT = "https://github.com/horizon-rust/horizon-mixins"
# horizon-mixins' commit for a mod that names none (the launcher's MIXINS_REV).
DEFAULT_MIXINS = "e8cd0aa38cffef49cb2e088ef716baa107148c02"
EXE = ".exe" if os.name == "nt" else ""


def run(cmd, **kw):
    print("+", " ".join(str(c) for c in cmd), flush=True)
    subprocess.run([str(c) for c in cmd], check=True, **kw)


def bundles():
    """The mods' bundles: folders of `native-mods/` with a `horizon-mixin.toml`."""
    if not MODS.is_dir():
        return []
    return sorted(p for p in MODS.iterdir() if (p / "horizon-mixin.toml").is_file())


def mixins_commit(mods):
    commits = set()
    for mod in mods:
        meta = mod / "horizon-native.json"
        if meta.is_file():
            commits.add(json.loads(meta.read_text(encoding="utf-8")).get("mixins") or DEFAULT_MIXINS)
        else:
            commits.add(DEFAULT_MIXINS)
    if len(commits) > 1:
        sys.exit(f"the mods need different horizon-mixins commits: {sorted(commits)}")
    return commits.pop()


def tool(commit):
    """horizon-mixins at `commit`, built once."""
    root = WORK / "tools" / commit
    exe = root / "bin" / f"horizon-mixins{EXE}"
    if not exe.is_file():
        run(["cargo", "install", "--locked", "--git", MIXINS_GIT, "--rev", commit,
             "--root", root, "horizon-mixins-cli"])
    return exe


def source_files():
    out = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT, check=True, capture_output=True).stdout
    for name in out.decode().split("\0"):
        path = ROOT / name
        # horizon-mixins refuses links; mods' own folder isn't game source.
        if name and not name.startswith("native-mods/") and path.is_file() and not path.is_symlink():
            yield name


def copy_source(to, revision):
    if to.exists():
        shutil.rmtree(to)
    for name in source_files():
        dest = to / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / name, dest)
    (to / ".horizon-revision").write_text(revision + "\n", encoding="utf-8")


def copy_mods(mods, to, revision):
    if to.exists():
        shutil.rmtree(to)
    to.mkdir(parents=True)
    for mod in mods:
        shutil.copytree(mod, to / mod.name)
        manifest = to / mod.name / "horizon-mixin.toml"
        lines = manifest.read_text(encoding="utf-8").splitlines()
        lines = [f'horizon_revision = "{revision}"' if l.startswith("horizon_revision") else l
                 for l in lines]
        manifest.write_text("\n".join(lines) + "\n", encoding="utf-8")


def sync(src, dest):
    """`dest` made the same as `src`, touching only the files that differ."""
    dest.mkdir(parents=True, exist_ok=True)
    seen = set()
    for dirpath, _, files in os.walk(src):
        rel = Path(dirpath).relative_to(src)
        for f in files:
            seen.add(rel / f)
            a, b = src / rel / f, dest / rel / f
            if not b.is_file() or not filecmp.cmp(a, b, shallow=False):
                b.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(a, b)
    for dirpath, _, files in os.walk(dest):
        rel = Path(dirpath).relative_to(dest)
        for f in files:
            if rel / f not in seen:
                (dest / rel / f).unlink()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--release", action="store_true", help="the release profile instead of fast")
    ap.add_argument("--features", default="java-mods,java-mods-120")
    ap.add_argument("-j", "--jobs", help="Cargo's build jobs")
    args = ap.parse_args()

    mods = bundles()
    if not mods:
        sys.exit(f"no native mods in {MODS}")
    print("native mods:", ", ".join(m.name for m in mods))
    revision = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, check=True,
                              capture_output=True, text=True).stdout.strip()
    exe = tool(mixins_commit(mods))
    with tempfile.TemporaryDirectory(dir=WORK) as tmp:
        tmp = Path(tmp)
        copy_source(tmp / "source", revision)
        copy_mods(mods, tmp / "mods", revision)
        run([exe, "apply", "--workspace", tmp / "source", "--mods", tmp / "mods", "--out", tmp / "out"])
        sync(tmp / "out", WORK / "stage")
    profile = "release" if args.release else "fast"
    cmd = ["cargo", "build", "--locked", "--profile", profile, "-p", "horizon-client",
           "--bin", "horizon-client", "--target-dir", ROOT / "target"]
    if args.features:
        cmd += ["--features", args.features]
    if args.jobs:
        cmd += ["-j", args.jobs]
    run(cmd, cwd=WORK / "stage")
    print(f"built: {ROOT / 'target' / profile / ('horizon-client' + EXE)}")


if __name__ == "__main__":
    WORK.mkdir(parents=True, exist_ok=True)
    main()
