#!/usr/bin/env python3
"""Runs run_one.sh for each mod of plan.json (its jars + Fabric API, + Mod Menu when it has a
config screen). Usage: survey.py [slug ...] (default all, skipping ones already done)."""
import json, os, shutil, subprocess, sys
HERE = os.path.dirname(os.path.abspath(__file__))
SV = os.environ.get("SURVEY_DIR", "/tmp/hz-survey")
plan = json.load(open(f"{SV}/plan.json"))
by = {m["slug"]: m for m in plan}
EXTRA = {  # keys that open the mod's own screens, commands
    "xaeros-world-map": "key:m",
    "xaeros-minimap": "key:y",
    "jei": "cmd:recipes_minecraft:chest",
    "simple-voice-chat": "key:v",
    "inventory-profiles-next": "",
    "veinminer": "cmd:veinminer",
    "zoomify": "",
    "fancymenu": "",
}
want = sys.argv[1:] or [m["slug"] for m in plan]
for slug in want:
    m = by[slug]
    out = f"{SV}/runs/{slug}" + os.environ.get("SUFFIX", "")
    if os.path.exists(f"{out}/run.txt") and not sys.argv[1:]:
        continue
    d = f"{SV}/mods/{slug}"
    shutil.rmtree(d, ignore_errors=True); os.makedirs(d)
    files = [f["file"] for f in m["files"]]
    menu = m["info"]["modmenu"]
    extra = [by["fabric-api"]]
    if menu and slug != "modmenu":
        extra.append(by["modmenu"])
    for x in os.environ.get("ADD", "").split():
        extra.append(by[x])
    for e in extra:
        files += [f["file"] for f in e["files"]]
    for f in dict.fromkeys(files):
        os.symlink(f"{SV}/jars/{f}", f"{d}/{f}")
    shutil.rmtree(out, ignore_errors=True)
    env = dict(os.environ, MODDIR=d, OUTDIR=out, MODID=m["info"]["id"] or slug,
               MENU="1" if menu else "", EXTRA=EXTRA.get(slug, ""))
    print(f"== {slug} ({len(set(files))} jars)", flush=True)
    subprocess.run(["bash", f"{HERE}/run_one.sh"], env=env)
    print(open(f"{out}/run.txt").read(), flush=True)
