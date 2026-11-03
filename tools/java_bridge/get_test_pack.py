#!/usr/bin/env python3
"""Downloads a test pack for the Java bridge: Fabric mods for Minecraft 26.3 from Modrinth
(the ones earlier bridge chats tested) plus their required dependencies.
Usage: python3 get_test_pack.py [folder]   (default ~/hz-test/mods-java)"""
import json, os, sys, urllib.parse, urllib.request, hashlib
GAME, LOADER = "26.3", "fabric"
OUT = os.path.expanduser(sys.argv[1] if len(sys.argv) > 1 else "~/hz-test/mods-java")
MODS = """fabric-api farmers-delight-refabricated toms-storage jade appleskin rei architectury-api
cloth-config biomes-o-plenty terrablender glitchcore naturalist friends-and-foes mutant-monsters
ecologics xaeros-minimap xaeros-world-map journeymap trinkets-updated geckolib veinminer
compressed-blocks moving-elevators simple-copper-pipes effortless-building inventory-sorter
easy-shulker-boxes spell-engine wizards runes jewelry archers mythic-upgrades forgiving-void
elevatorid mob-lassos hang-glider fallingtree towns-and-towers repurposed-structures
open-parties-and-claims better-combat travelers-backpack energized-power dungeons-and-taverns
serene-seasons carry-on clumps rightclickharvest legendary-tooltips skills cc-tweaked
advanced-loot-info voxelmap-updated minihud xaeroplus lithium""".split()
H = {"User-Agent": "horizon-java-bridge-test-pack"}
def api(p):
    return json.load(urllib.request.urlopen(urllib.request.Request("https://api.modrinth.com/v2" + p, headers=H), timeout=60))
def latest(pid):
    q = urllib.parse.urlencode({"game_versions": json.dumps([GAME]), "loaders": json.dumps([LOADER])})
    try: vs = api(f"/project/{pid}/version?{q}")
    except Exception: return None
    return vs[0] if vs else None
os.makedirs(OUT, exist_ok=True)
seen, todo, got, missing = set(), list(MODS), [], []
while todo:
    pid = todo.pop(0)
    if pid in seen: continue
    seen.add(pid)
    v = latest(pid)
    if not v: missing.append(pid); continue
    f = next((x for x in v["files"] if x["primary"]), v["files"][0])
    path = os.path.join(OUT, f["filename"])
    if not os.path.exists(path):
        data = urllib.request.urlopen(urllib.request.Request(f["url"], headers=H), timeout=300).read()
        if hashlib.sha1(data).hexdigest() != f["hashes"]["sha1"]: sys.exit("checksum mismatch: " + f["filename"])
        open(path, "wb").write(data)
    got.append(f["filename"])
    for d in v.get("dependencies", []):
        if d.get("dependency_type") == "required" and d.get("project_id"): todo.append(d["project_id"])
print(f"{len(got)} jars in {OUT}")
if missing: print("no 26.3 Fabric version (skipped):", " ".join(m for m in missing if not m.isalnum() or not len(m) == 8))
