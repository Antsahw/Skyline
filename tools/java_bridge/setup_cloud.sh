#!/bin/bash
# One step to make a fresh cloud container ready for bridge work (what a boss or worker chat
# otherwise does by hand: ~10 tool calls). Run from the repo root; then build and test as usual.
#   tools/java_bridge/setup_cloud.sh [jar-slug ...]
# Does: JDK 25 + build/headless packages (apt), Mojang's 26.3 files into ~/mc (a launcher-style
# folder, never in the repo), lab.py prepare, the game build in the background (log:
# /tmp/build.log), horizon-prepare --complete, and Modrinth jars by slug into ~/jars.
# Default jars: the ones the boss's standard cases need.
set -euo pipefail
ROOT=$(git rev-parse --show-toplevel)
cd "$ROOT"
export JAVA_HOME=/usr/lib/jvm/java-25-openjdk-amd64 HORIZON_JAVA_HOME=/usr/lib/jvm/java-25-openjdk-amd64

if [ ! -x "$JAVA_HOME/bin/java" ]; then
  sudo apt-get update -qq
  sudo apt-get install -y -qq openjdk-25-jdk-headless libasound2-dev libudev-dev mesa-vulkan-drivers xvfb >/tmp/apt.log
fi

# Start the long build at once; everything else runs beside it.
cargo build --profile fast -p horizon-client -p horizon-prepare \
  --features horizon-client/java-mods >/tmp/build.log 2>&1 &
BUILD=$!

python3 - "$HOME/mc" <<'EOF'
import json, os, sys, urllib.request, concurrent.futures as cf
root, ver = sys.argv[1], "26.3"
def get(url):
    with urllib.request.urlopen(url, timeout=120) as r: return r.read()
def save(url, path):
    if os.path.exists(path): return
    os.makedirs(os.path.dirname(path), exist_ok=True)
    data = get(url); open(path + ".tmp", "wb").write(data); os.replace(path + ".tmp", path)
man = json.loads(get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"))
v = next(x for x in man["versions"] if x["id"] == ver)
vj = get(v["url"]); j = json.loads(vj)
vd = f"{root}/versions/{ver}"; os.makedirs(vd, exist_ok=True)
open(f"{vd}/{ver}.json", "wb").write(vj)
jobs = [(j["downloads"]["client"]["url"], f"{vd}/{ver}.jar"),
        (j["downloads"]["server"]["url"], f"{root}/server-{ver}.jar")]
for lib in j["libraries"]:
    a = lib.get("downloads", {}).get("artifact")
    if a: jobs.append((a["url"], f"{root}/libraries/{a['path']}"))
ai = j["assetIndex"]; idx = get(ai["url"])
p = f"{root}/assets/indexes/{ai['id']}.json"; os.makedirs(os.path.dirname(p), exist_ok=True); open(p, "wb").write(idx)
for o in json.loads(idx)["objects"].values():
    h = o["hash"]; jobs.append((f"https://resources.download.minecraft.net/{h[:2]}/{h}", f"{root}/assets/objects/{h[:2]}/{h}"))
with cf.ThreadPoolExecutor(16) as ex: list(ex.map(lambda a: save(*a), jobs))
print("Mojang files:", len(jobs))
EOF
python3 tools/lab.py prepare --minecraft-dir "$HOME/mc" --server-jar "$HOME/mc/server-26.3.jar" | tail -1

SLUGS=("$@")
[ ${#SLUGS[@]} -eq 0 ] && SLUGS=(fabric-api farmers-delight-refabricated jade appleskin xaeros-minimap rei architectury-api cloth-config jei geckolib mielons-the-sift)
python3 - "$HOME/jars" "${SLUGS[@]}" <<'EOF'
import json, os, sys, urllib.request, urllib.parse
out = sys.argv[1]; os.makedirs(out, exist_ok=True)
q = urllib.parse.urlencode({"game_versions": '["26.3"]', "loaders": '["fabric"]'})
ua = {"User-Agent": "horizon-bridge-test"}
for slug in sys.argv[2:]:
    vs = json.load(urllib.request.urlopen(urllib.request.Request(f"https://api.modrinth.com/v2/project/{slug}/version?{q}", headers=ua)))
    if not vs: print("no 26.3 Fabric version:", slug); continue
    f = next((f for f in vs[0]["files"] if f["primary"]), vs[0]["files"][0])
    path = os.path.join(out, f["filename"])
    if not os.path.exists(path):
        open(path, "wb").write(urllib.request.urlopen(urllib.request.Request(f["url"], headers=ua)).read())
    print("jar:", f["filename"])
EOF

echo "Waiting for the build (first time ~20 min on 2 cores; /tmp/build.log)"
wait $BUILD || { tail -20 /tmp/build.log; exit 1; }
target/fast/horizon-prepare --root "$ROOT" --prepare --complete \
  --minecraft-dir "$HOME/mc" --server-jar "$HOME/mc/server-26.3.jar" | tail -1
echo "Ready. JAVA_HOME=$JAVA_HOME; jars in ~/jars:"
echo "  python3 tools/java_bridge/regress.py --jars ~/jars <cases>"
