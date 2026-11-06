#!/usr/bin/env python3
"""The launcher's "Fabric 26.3 mods (beta)" switch, headless, through its CLI (docs/java-bridge.md).

A scratch launcher home with one installed version whose programs are stand-ins that write down
how they were started: the plain one, and its Java mods build in the version's java/ folder.
Checks, for the default instance:
- off (the default): Play starts the plain program without --java-mods, even with jars in
  mods-java/, and `java-mods status` says off;
- on with no Java mods build to get (no such release): the switch stays on and Play refuses,
  saying why, rather than starting the plain game;
- on with a build (`java-mods use-build`): Play starts the version's java/ program with
  --java-mods, in the instance's folder;
- off again: the plain program again.
Launcher only (bridge-launcher-only): with no signed-in owner, nothing for the mods is downloaded
(no build in java/, no Java runtime in the launcher's runtime folder) and the switch and Play say
"Sign in"; `use-build` needs no sign-in. Play's checks past the gate need a signed-in owner: a
scratch launcher home has none, so they are skipped, and said so. The window's developer option
(settings.json `dev_offline`) lets a plain instance's Play through but never a Fabric one:
docs/java-bridge.md, "Launcher only", has that check (the window under Xvfb).

    python3 tools/java_bridge/launcher_switch_test.py --launcher launcher/target/debug/horizon-launcher
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TAG = "v2099.1.1"


def stand_in(path, name, log):
    # name|arguments|folder|the owner record's path for the game (HORIZON_JAVA_MODS_OWNER)
    path.write_text(f"#!/bin/sh\nprintf '%s|%s|%s|%s\\n' '{name}' \"$*\" \"$PWD\" \"$HORIZON_JAVA_MODS_OWNER\" >> '{log}'\n")
    path.chmod(0o755)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--launcher", required=True)
    parser.add_argument("--keep", action="store_true", help="leave the scratch folder")
    args = parser.parse_args()
    launcher = Path(args.launcher).resolve()
    work = Path(tempfile.mkdtemp(prefix="hz-switch-"))
    log = work / "ran.log"
    version = work / "lh/versions" / TAG
    lock = json.loads((ROOT / "research-lock.json").read_text())
    resources = version / ".local/horizon-client" / lock["minecraft"]["version"]
    resources.mkdir(parents=True)
    (resources / "horizon-resources.json").write_text("{}")
    shutil.copy(ROOT / "research-lock.json", version)
    stand_in(version / "horizon-client", "plain", log)
    (version / ".installed").write_text("")
    java_build = work / "built-java-client"
    stand_in(java_build, "java", log)
    home = work / "home"
    env = dict(os.environ, HORIZON_LAUNCHER_HOME=str(work / "lh"), HORIZON_HOME=str(home),
               XDG_DATA_HOME=str(work / "data"), XDG_CONFIG_HOME=str(work / "conf"),
               HORIZON_LAUNCHER_NO_SELF_UPDATE="1", HORIZON_NO_AUDIO="1", HORIZON_TELEMETRY_URL="off",
               HORIZON_LAUNCHER_TRASH=str(work / "trash"), HORIZON_LAUNCHER_CLIPBOARD=str(work / "clip"),
               HORIZON_LAUNCHER_DESKTOP=str(work / "desktop"))
    failures = []

    def run(*a):
        r = subprocess.run([str(launcher), *a], env=env, input="", capture_output=True, text=True, timeout=300)
        print(f"$ horizon-launcher {' '.join(a)}  -> {r.returncode}\n{(r.stdout + r.stderr).strip()}\n")
        return r

    def check(ok, what):
        print(("PASS " if ok else "FAIL ") + what)
        if not ok:
            failures.append(what)

    gated = []

    def gate(r):
        # No signed-in owner in a scratch home: Play stops at the owner gate, before any program.
        if r.returncode != 0 and "Sign in" in r.stderr:
            gated.append(r)
            return True
        return False

    def played():
        lines = log.read_text().splitlines() if log.exists() else []
        log.unlink(missing_ok=True)
        return [line.split("|") for line in lines]

    status = lambda: json.loads(run("java-mods", "status", "--json").stdout)

    check(status()["on"] is False, "off by default")
    # Jars in mods-java/ don't turn anything on.
    (home / "mods-java").mkdir(parents=True)
    (home / "mods-java/some-mod.jar").write_bytes(b"PK\x05\x06" + bytes(18))
    r = run("play")
    ran = played()
    if not gate(r):
        check(r.returncode == 0 and len(ran) == 1 and ran[0][0] == "plain" and "--java-mods" not in ran[0][1]
              and ran[0][3] == "", "off: the plain program, no --java-mods, with jars in mods-java/")

    runtime = work / "lh/minecraft/runtime"

    def downloaded():
        return (version / "java").exists() or (version / ".java.partial").exists() or runtime.exists()

    r = run("java-mods", "on")
    check(status()["on"] is True and not status()["java_build_installed"], "on: saved, no build")
    check("Sign in" in r.stdout + r.stderr and not downloaded(),
          "on with no signed-in owner: nothing downloaded (build, Java), says Sign in")
    r = run("play")
    ran = played()
    check(r.returncode != 0 and not ran and "Sign in" in r.stderr and not downloaded(),
          "on without a build or owner: Play refuses, says Sign in, downloads nothing, no game")

    r = run("java-mods", "use-build", str(java_build))
    check(r.returncode == 0 and status()["java_build_installed"], "use-build: the version's java/ build, no sign-in")
    check((version / "java/horizon-client").is_file(), "the build sits in the version's java/ folder")
    r = run("play", "--world", "W")
    ran = played()
    gate(r) or check(r.returncode == 0 and len(ran) == 1 and ran[0][0] == "java" and "--java-mods" in ran[0][1].split()
          and "--world W" in ran[0][1] and ran[0][2] == str(home) and "/java-mods-owner/session-" in ran[0][3],
          "on: the Java mods build with --java-mods and the owner record's path, the game's arguments and folder")
    listed = json.loads(run("instance", "list", "--json").stdout)
    check(any(i["id"] == "horizon" and i["java_mods"] for i in listed), "instance list says java_mods")

    r = run("java-mods", "off")
    check(r.returncode == 0 and status()["on"] is False, "off again: saved")
    r = run("play")
    ran = played()
    gate(r) or check(r.returncode == 0 and len(ran) == 1 and ran[0][0] == "plain"
                     and "--java-mods" not in ran[0][1], "off again: the plain program")

    if not args.keep:
        shutil.rmtree(work, ignore_errors=True)
    else:
        print(f"scratch: {work}")
    if gated:
        print(f"SKIP {len(gated)} Play checks: no signed-in owner here (the owner gate)")
    print("launcher switch:", "FAIL " + str(failures) if failures else "PASS")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
