#!/usr/bin/env python3
"""Reads runs/<slug>/ and prints one JSON line per mod: loaded?, left out, failures (log lines
not in the Fabric-API-only baseline), config screen opened, screens."""
import json, os, re, sys
SV = os.environ.get("SURVEY_DIR", "/tmp/hz-survey")
plan = json.load(open(f"{SV}/plan.json"))

def norm(l):
    l = re.sub(r"^\[\d\d:\d\d:\d\d\] ", "", l)
    l = re.sub(r"/tmp/hz-bridge\.\w+", "<S>", l)
    l = re.sub(r"\b\d+ ms\b", "N ms", l)
    return l.rstrip()

BAD = re.compile(r"ERROR|Exception|Error:|failed|could not|Could not|panicked|refused|Caused by|"
                 r"Mixin.*(fail|error|warn)|WARN.*[Mm]ixin|not loaded|Invalid|Unable|Missing", re.I)
def lines(slug):
    p = f"{SV}/runs/{slug}/latest.log"
    return [norm(l) for l in open(p, errors="replace")] if os.path.exists(p) else []

base = set(l for l in lines("fabric-api") if BAD.search(l))
def state(slug, n):
    try: return json.load(open(f"{SV}/runs/{slug}/{n}.json"))
    except Exception: return None

res = []
for m in plan:
    slug = m["slug"]; d = f"{SV}/runs/{slug}"
    if not os.path.exists(f"{d}/run.txt"): continue
    run = open(f"{d}/run.txt").read()
    L = lines(slug)
    loaded = next((l for l in L if "Java mods: Fabric loaded" in l), "")
    mid = m["info"]["id"]
    r = {"slug": slug, "id": mid, "joined": "joined=yes" in run, "alive": "alive=yes" in run,
         "loaded": bool(re.search(rf"[ ,]{re.escape(mid)} ", loaded + " ")) if mid else False,
         "left_out": [l.split("Java mods: ")[1] for l in L if "isn't needed in Horizon" in l and "[CHAT]" not in l],
         "bad": [], "screens": {}}
    seen = set()
    for i, l in enumerate(L):
        if BAD.search(l) and l not in base and l not in seen and "[CHAT]" not in l:
            seen.add(l); r["bad"].append(l[:300])
    for n in ("1-hud", "2-inventory", "3-config") + tuple(
            f[:-5] for f in sorted(os.listdir(d)) if re.match(r"[4-8]-.*\.json$", f)):
        s = state(slug, n)
        if s: r["screens"][n] = s.get("screen")
    chat = [l for l in L if "[CHAT]" in l and ("No settings screen" in l or "failed" in l)]
    r["chat"] = [c.split("[CHAT] ")[1][:200] for c in chat]
    res.append(r)
for r in res:
    print(json.dumps(r))
